//! Lazy materialization and indexed-mode pipeline policy for [`Image`].

use std::sync::Arc;

use image_slash_star::{ImageFormat, ImageInfo};

use super::codec_adapter::decoded_to_dynamic;

use crate::compute::registry::support::{ModeSupport, OperationId, OutputModeRule};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, Mode};

mod derived;
mod evaluation;
mod palette_output;
mod receipts;

use super::{
    Image, LoadedData, PaletteData, PalettedData, PipelinePaletteOutput, declared_pipeline_mode,
    expand_palette, expand_palette_alpha, materialization_cache, padded_palette,
    validate_scalar_storage,
};

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn retag_declared_palette_mode(
    image: Arc<DynamicImage>,
    output_mode: Option<Mode>,
) -> Arc<DynamicImage> {
    let needs_luma_retag = matches!(
        (output_mode, image.as_ref()),
        (Some(Mode::ImagePalette8), DynamicImage::ImageLuma8(_))
            | (Some(Mode::ImagePaletteAlpha8), DynamicImage::ImageLumaA8(_))
    );
    if !needs_luma_retag {
        return image;
    }

    match Arc::try_unwrap(image) {
        Ok(DynamicImage::ImageLuma8(image)) => Arc::new(DynamicImage::ImagePalette8(image)),
        Ok(DynamicImage::ImageLumaA8(image)) => Arc::new(DynamicImage::ImagePaletteAlpha8(image)),
        Ok(image) => Arc::new(image),
        Err(image) => match (output_mode, image.as_ref()) {
            (Some(Mode::ImagePalette8), DynamicImage::ImageLuma8(image)) => {
                Arc::new(DynamicImage::ImagePalette8(image.clone()))
            }
            (Some(Mode::ImagePaletteAlpha8), DynamicImage::ImageLumaA8(image)) => {
                Arc::new(DynamicImage::ImagePaletteAlpha8(image.clone()))
            }
            _ => image,
        },
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn image_from_materialized(
    image: Arc<DynamicImage>,
    source_format: Option<ImageFormat>,
    info: Option<ImageInfo>,
    exif: Option<Vec<u8>>,
) -> Result<Image, PilError> {
    if image.mode() == Mode::ImagePalette8 {
        let palette = info
            .as_ref()
            .and_then(|info| info.palette.clone())
            .ok_or_else(|| {
                PilError::PaletteError("decoded P-mode image has no retained palette".to_owned())
            })?;
        return Ok(Image::Paletted(PalettedData {
            indices: DynamicImage::ImagePalette8(image.to_luma8()),
            palette_attached: true,
            palette: padded_palette(&palette.rgb),
            // image-slash-star keeps decoded palette transparency in codec
            // metadata. Pillow's P palette remains RGB until transparency is
            // explicitly committed to an RGBA palette.
            palette_data: PaletteData::Rgb,
            palette_alpha: palette.alpha,
            source_format,
            info,
            exif: exif.clone(),
            materialized: materialization_cache(),
        }));
    }
    Ok(Image::Loaded(LoadedData {
        image,
        known_zero_fill: false,
        palette: None,
        palette_data: None,
        palette_alpha: None,
        source_format,
        info,
        exif,
    }))
}

fn map_codec_error(error: image_slash_star::ImageError, source: &str) -> PilError {
    if matches!(error, image_slash_star::ImageError::UnknownFormat) {
        PilError::UnidentifiedImageError(source.to_owned())
    } else {
        PilError::ImageError(error)
    }
}

fn map_deferred_decode_error(error: image_slash_star::ImageError) -> PilError {
    match error {
        // Pillow's lazy Image.load path collapses malformed encoded payloads
        // to this stable OSError message, even though open() accepted the
        // format header. Keep codec-specific diagnostics out of the public
        // error contract for deferred loads.
        image_slash_star::ImageError::Malformed { .. } => {
            PilError::IOError("cannot load this image".to_owned())
        }
        other => map_codec_error(other, "memory"),
    }
}

impl Image {
    // ── Materialize ──

    /// Execute the pipeline chain and return a decoded DynamicImage.
    /// This is where all the lazy work gets done.
    /// Check whether an op can run on indexed P/PA samples without expanding
    /// them to RGB. The mode strategy owns P filter normalization; PA filters
    /// operate on their native index and alpha lanes.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn is_palette_safe_op(op: &PipelineOp) -> bool {
        if Self::palette_preserves_raw_samples(op) {
            return true;
        }
        match op {
            PipelineOp::CompositeModule { other, .. } => {
                // PIL.Image.composite starts from image2 and pastes image1
                // into it. P and PA operands are both blended as raw sample
                // bytes; PA is not included in has_palette_mode because that
                // helper intentionally means a single-byte palette index.
                other.has_palette_mode()
                    || other.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
            }
            PipelineOp::PutPixel {
                palette_index: true,
                ..
            } => true,
            PipelineOp::Rotate { fill: None, .. } => true,
            PipelineOp::Resize { .. }
            | PipelineOp::ResizeBoxed { .. }
            | PipelineOp::Thumbnail { .. } => true,
            PipelineOp::Transform {
                method: TransformMethod::Affine,
                filter: ResampleFilter::Nearest,
                palette_fill,
                ..
            } => palette_fill.is_some(),
            _ => false,
        }
    }

    #[inline(always)]
    fn palette_preserves_raw_samples(op: &PipelineOp) -> bool {
        match op {
            // ImageChops.duplicate keeps source samples and palette metadata,
            // but remains a pipeline operation so backend selection is
            // observable.
            PipelineOp::Duplicate
            | PipelineOp::Crop { .. }
            | PipelineOp::Reduce { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Offset { .. }
            | PipelineOp::Paste { .. }
            // Pillow 12.2.0 libImaging/Effects.c:117-159 scatters the
            // one-byte P indices, then duplicates the complete palette.
            | PipelineOp::EffectSpread { .. }
            | PipelineOp::DrawLine { .. }
            | PipelineOp::DrawRectangle { .. }
            | PipelineOp::DrawRoundedRect { .. }
            | PipelineOp::DrawEllipse { .. }
            | PipelineOp::DrawCircle { .. }
            | PipelineOp::DrawPolygon { .. }
            | PipelineOp::DrawArc { .. }
            | PipelineOp::DrawChord { .. }
            | PipelineOp::DrawPieslice { .. }
            | PipelineOp::DrawPoint { .. }
            // ImageChops.invert applies 255-index directly to P samples.
            // Pillow's Image._new copies the source palette onto the new
            // indexed wrapper, so the lazy Image retains that metadata too.
            | PipelineOp::InvertChops
            // Pillow's Chops.c arithmetic also operates on raw indexed
            // samples for P/PA images. Image._new preserves the primary
            // operand's palette when wrapping that new indexed core.
            | PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
            | PipelineOp::Multiply { .. }
            | PipelineOp::Screen { .. }
            | PipelineOp::Darker { .. }
            | PipelineOp::Lighter { .. }
            | PipelineOp::Difference { .. }
            | PipelineOp::Overlay { .. }
            | PipelineOp::HardLight { .. }
            | PipelineOp::SoftLight { .. }
            | PipelineOp::AddModulo { .. }
            | PipelineOp::SubtractModulo { .. }
            // ImageOps.expand allocates the destination in the source mode;
            // P and PA therefore carry raw index (and alpha) samples through
            // the border operation instead of being expanded to RGB(A).
                        | PipelineOp::Expand { .. }
                        | PipelineOp::ExpandTyped { .. }
            // Image.merge accepts a P image as the first single-byte band of
            // a multi-band result. Keep its raw index values; expanding the
            // palette here would turn an index such as 1 into its RGB entry.
            | PipelineOp::Merge { .. }
            // Image.remap_palette builds an inverse index LUT and separately
            // installs the reordered palette on the result.
            | PipelineOp::RemapPalette { .. }
            // Pillow's Image.point maps P indices directly, and Image._new
            // copies the source palette. Image.eval delegates to point.
            | PipelineOp::Eval { .. }
            | PipelineOp::PutAlpha {
                mode: Mode::ImagePalette8 | Mode::ImagePaletteAlpha8,
                ..
            }
            | PipelineOp::PutAlphaData {
                mode: Mode::ImagePalette8 | Mode::ImagePaletteAlpha8,
                ..
            }
            // Pillow keeps P/PA images indexed through ImageOps.pad. The
            // resize step uses nearest-neighbour samples for these modes,
            // and the pad fill is another raw index/alpha pair.
            | PipelineOp::Pad { .. }
            | PipelineOp::PadTyped { .. }
            | PipelineOp::PadPositioned { .. }
            // ImageOps.scale delegates to the resize path. Indexed P/PA
            // samples force nearest-neighbour scaling and remain indexed;
            // expanding them here would change the public result mode before
            // the SIMD/CPU resize adapter can preserve the raw samples.
            | PipelineOp::Scale { .. }
            // Pillow's PA bands are the raw index and alpha bytes. Extracting
            // either band must happen before palette expansion, just like the
            // corresponding ImagingCore band operation.
            | PipelineOp::ExtractBand { .. } => true,
            _ => false,
        }
    }

    /// Returns whether PutData's declared payload keeps an indexed source mode.
    ///
    /// TODO(pillow-rs#3ptb): freeze P/PA exact-width and cross-width payload
    /// output, palette metadata, and selected/fallback receipt parity.
    ///
    /// Reuse the operation-level payload/output contract so cross-width payloads
    /// cannot retain a P/PA wrapper when execution produces another mode.
    fn putdata_preserves_palette_output(mode: Mode, op: &PipelineOp) -> bool {
        let (
            source_mode @ (Mode::ImagePalette8 | Mode::ImagePaletteAlpha8),
            PipelineOp::PutData { payload_mode, .. },
        ) = (mode, op)
        else {
            return false;
        };
        let ModeSupport::Supported {
            output: OutputModeRule::ByPayload(outputs),
            ..
        } = OperationId::PutData.support().for_mode(source_mode)
        else {
            return false;
        };
        outputs
            .iter()
            .any(|output| output.payload == *payload_mode && output.output == source_mode)
    }

    /// Returns whether an operation can stay in this exact indexed sample
    /// layout. Pillow's `libImaging/Filter.c` `PA` path applies convolution to
    /// the raw index/alpha bands and preserves `PA`; unlike `P`, it is a valid
    /// input for built-in convolution filters. Keep the PA-specific exception
    /// out of the generic classifier so P still follows its declared rejection.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    pub(super) fn is_palette_safe_op_for_mode(mode: Mode, op: &PipelineOp) -> bool {
        Self::is_palette_safe_op(op) || Self::putdata_preserves_palette_output(mode, op)
            // Same-mode P/PA conversion is a wrapper copy; retain indexed
            // samples and their attached palette through internal pipelines.
            || (matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
                && matches!(
                    op,
                    PipelineOp::Convert {
                        mode: target_mode,
                        matrix: None,
                        ..
                    } if mode == *target_mode
                ))
            // Core-only P Posterize/Solarize strategies transform their
            // native index bytes and retain the source palette.
            || (mode == Mode::ImagePalette8
                && matches!(op, PipelineOp::Posterize { .. } | PipelineOp::Solarize { .. }))
            // Rust-only ImageOps Autocontrast transforms P index bytes and
            // keeps the source palette attached to the indexed output.
            || (mode == Mode::ImagePalette8
                && matches!(op, PipelineOp::Autocontrast { .. }))
            // The Rust ImageOps Invert strategies operate on native P/PA
            // samples and preserve their palette metadata. Python's Pillow
            // compatibility wrappers reject these modes before queuing Invert.
            || (matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
                && matches!(op, PipelineOp::Invert))
            // A single-band Eval LUT over P maps indices, not palette colors.
            || (mode == Mode::ImagePalette8
                && matches!(op, PipelineOp::Eval { lut } if lut.len() == 256))
            // PA.putpixel writes the native index/alpha sample pair. The
            // operation does not depend on the RGB palette, but a later
            // conversion does, so keep the palette attached to the lazy
            // pipeline while the raw PA samples are updated.
            || (mode == Mode::ImagePaletteAlpha8
                && matches!(op, PipelineOp::PutPixel { .. }))
            || (mode == Mode::ImagePaletteAlpha8
                && matches!(
                    op,
                    PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
                ))
            // The PA CPU Filter strategy declares native two-channel
            // implementations for box and rank filters. Keep their index / per-pixel
            // alpha samples in PA so prepared execution reaches those exact-mode
            // strategies instead of expanding the palette to RGBA. GaussianBlur
            // remains excluded because the PA strategy explicitly rejects it.
            || (mode == Mode::ImagePaletteAlpha8
                && matches!(
                    op,
                    PipelineOp::BoxBlur { .. }
                        | PipelineOp::BoxBlurXY { .. }
                        | PipelineOp::MedianFilter { .. }
                        | PipelineOp::MaxFilter { .. }
                        | PipelineOp::MinFilter { .. }
                        | PipelineOp::RankFilter { .. }
                ))
            // Pillow keeps PA rotation in its native index/alpha sample
            // layout, including bilinear/bicubic resampling and a two-element
            // fillcolor. The generic palette-safe rule only accepts a
            // rotation without a fill because P and PA fill values have
            // different semantics; this source-specific case carries the
            // already-normalized PA index/alpha pair through the
            // native-channel rotate kernel.
            || (mode == Mode::ImagePaletteAlpha8
                && matches!(op, PipelineOp::Rotate { .. }))
            // Pillow keeps P rotation in its native index layout, including a
            // fillcolor. The rotate backend forces nearest sampling for P and
            // interprets the first normalized fill component as the raw index.
            || (mode == Mode::ImagePalette8
                && matches!(op, PipelineOp::Rotate { .. }))
            // Pillow's ImagingTransform applies affine/projective sampling to
            // native P/PA samples and preserves both the indexed mode and the
            // wrapper palette. Keep this channel-wise operation indexed so a
            // retained RGB palette is not expanded to RGBA before transform.
            || (matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
                && matches!(op, PipelineOp::Transform { .. }))
            // Pillow's ImageOps.fit keeps P/PA images indexed through the
            // boxed resize. P uses nearest-neighbour samples regardless of
            // the requested kernel; PA resamples its raw index/alpha bands.
            || (matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
                && matches!(op, PipelineOp::Fit { .. }))
            // Pillow's ImageOps.contain/cover are resize wrappers. Their
            // indexed paths preserve P/PA samples and the source mode while
            // applying the requested kernel (P still forces nearest inside
            // the resize implementation).
            || (matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
                && matches!(op, PipelineOp::Contain { .. } | PipelineOp::Cover { .. }))
    }

    /// Whether this value currently represents palette indices rather than
    /// visible luma samples. Lazy inputs use cached header metadata so the
    /// first queued operation does not need to decode merely to preserve mode.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn has_palette_mode(&self) -> bool {
        self.known_mode_identity() == Some(Mode::ImagePalette8)
    }

    /// Whether samples are palette indices, optionally paired with per-pixel
    /// alpha in PA mode.
    ///
    /// Keep this separate from `has_palette_mode`: callers such as
    /// `apply_transparency`, P encoding, and palette-index `putpixel` implement
    /// Pillow behavior that applies only to single-band P images.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(super) fn has_palette_samples(&self) -> bool {
        self.has_palette_mode() || self.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
    }

    /// Decodes or executes this image into a concrete pixel buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy decoding, pipeline execution, or format
    /// conversion fails.
    pub fn materialize(&self) -> Result<DynamicImage, PilError> {
        let image = self.materialized_shared_for_ops()?;
        Ok(image.as_ref().clone())
    }

    /// Returns operation-ready shared pixels after validating the logical
    /// mode against their storage, without cloning the materialized buffer.
    pub(crate) fn materialized_shared_for_ops(&self) -> Result<Arc<DynamicImage>, PilError> {
        let image = self.materialized_shared()?;
        let mode = Self::mode_from_materialized(&image);
        validate_scalar_storage(&image, mode)?;
        Ok(image)
    }

    /// Materialize a lazy image once before creating independent branches.
    ///
    /// Pillow's eager image objects can be cropped repeatedly without
    /// replaying an earlier resize.  Keeping the shared materialized pixels in
    /// a concrete image preserves that behavior for callers that branch a
    /// lazy pipeline, while each derived branch can still remain lazy.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub(crate) fn materialized_branch(&self) -> Result<Image, PilError> {
        match self {
            Image::Bytes { format, info, .. } => {
                // Pillow loads an encoded image before constructing a crop;
                // a deferred decode failure therefore belongs to crop(), not
                // to a later property access on the returned image.
                let image = self.materialized_shared()?;
                image_from_materialized(image, *format, info.clone(), self.exif_metadata())
            }
            Image::Pipeline { .. } => {
                let image = self.materialized_shared()?;
                Ok(Image::Loaded(LoadedData {
                    known_zero_fill: false,
                    palette: self.palette(),
                    palette_data: self.palette_data().cloned(),
                    palette_alpha: self.palette_alpha(),
                    source_format: None,
                    // Loading a pipeline before crop clears the public file
                    // format, but Pillow retains the source's info mapping.
                    info: self.image_info(),
                    exif: self.exif_metadata(),
                    image,
                }))
            }
            _ => Ok(self.clone()),
        }
    }

    /// Returns shared operation-ready pixels, persistently initializing lazy
    /// source or pipeline state when necessary.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    pub(crate) fn materialized_shared(&self) -> Result<Arc<DynamicImage>, PilError> {
        match self {
            Image::Loaded(data) => Ok(Arc::clone(&data.image)),
            Image::Paletted(data) => data
                .materialized
                .get_or_init(|| Ok(Arc::new(data.indices.clone())))
                .clone(),
            Image::Bytes {
                source,
                header_mode,
                materialized,
                ..
            } => materialized
                .get_or_init(|| {
                    let decoded = source.decode().map_err(map_deferred_decode_error)?;
                    decoded_to_dynamic(decoded, source.info().mode, *header_mode).map(Arc::new)
                })
                .clone(),
            Image::Pipeline {
                source,
                ops,
                mode,
                backend,
                palette,
                palette_alpha,
                palette_data,
                materialized,
                ..
            } => materialized
                .get_or_init(|| {
                    let _origin_scope = ops.telemetry_origin_scope();
                    let output_mode = declared_pipeline_mode(mode);
                    let (image, palette_output) = Self::evaluate_pipeline(
                        source,
                        ops.as_slice(),
                        &output_mode,
                        *backend,
                        palette,
                        palette_alpha,
                        palette_data,
                        ops.preserves_palette(),
                        ops.prefix_cache(),
                        ops.execution_context(),
                    )?;
                    if let Some(output) = palette_output {
                        ops.cache_palette_output(output)?;
                    }
                    Ok(image)
                })
                .clone(),
        }
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn materialize_uncached(&self) -> Result<DynamicImage, PilError> {
        self.materialize_uncached_with_metadata()
            .map(|(image, _)| image)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn materialize_uncached_with_metadata(
        &self,
    ) -> Result<(DynamicImage, Option<PipelinePaletteOutput>), PilError> {
        match self {
            Image::Loaded(data) => Ok((data.image.as_ref().clone(), None)),
            Image::Paletted(data) => Ok((data.indices.clone(), None)),
            Image::Bytes {
                source,
                header_mode,
                ..
            } => decoded_to_dynamic(
                &image_slash_star::decode(source.bytes()).map_err(map_deferred_decode_error)?,
                source.info().mode,
                *header_mode,
            )
            .map(|image| (image, None)),
            Image::Pipeline {
                source,
                ops,
                mode,
                backend,
                palette,
                palette_alpha,
                palette_data,
                ..
            } => {
                let _origin_scope = ops.telemetry_origin_scope();
                let output_mode = declared_pipeline_mode(mode);
                Self::evaluate_pipeline_uncached(
                    source,
                    ops,
                    &output_mode,
                    *backend,
                    palette,
                    palette_alpha,
                    palette_data,
                    ops.preserves_palette(),
                    ops.execution_context(),
                )
                .map(|(image, palette_output)| (image.as_ref().clone(), palette_output))
            }
        }
    }

    /// Materializes a `P` image as palette index bytes.
    ///
    /// Paletted images return a one-byte-per-pixel `Luma8` buffer containing
    /// palette indices rather than RGB colors. Non-paletted images delegate to
    /// [`Image::materialize`].
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy decoding or pipeline execution fails.
    pub fn materialize_indices(&self) -> Result<DynamicImage, PilError> {
        self.materialize()
    }
}

mod lifecycle;
