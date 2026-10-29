//! Lazy materialization and indexed-mode pipeline policy for [`Image`].

use std::sync::Arc;

use image_slash_star::{Decoded, DecodedImage, ImageFormat, ImageInfo};

use super::codec_adapter::decoded_to_dynamic;

use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, Mode};

use super::{
    Image, ImageMode, LoadedData, PalettedData, PipelinePrefixCache, expand_palette,
    expand_palette_alpha, image_mode_name, materialization_cache, padded_palette,
    take_cached_materialization, validate_scalar_storage,
};

fn image_from_materialized(
    image: Arc<DynamicImage>,
    source_format: Option<ImageFormat>,
    info: Option<ImageInfo>,
    exif: Option<Vec<u8>>,
) -> Result<Image, PilError> {
    let decoded_mode = info
        .as_ref()
        .map_or_else(|| image.color().into(), |info| info.mode);
    if image.mode() == Mode::ImagePalette8 || decoded_mode == ImageMode::P8 {
        let palette = info
            .as_ref()
            .and_then(|info| info.palette.clone())
            .ok_or_else(|| {
                PilError::PaletteError("decoded P-mode image has no retained palette".to_owned())
            })?;
        return Ok(Image::Paletted(PalettedData {
            indices: DynamicImage::ImagePalette8(image.to_luma8()),
            palette: padded_palette(&palette.rgb),
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
        explicit_mode: None,
        decoded_mode,
        palette: None,
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
    /// Check whether an op can be applied directly to palette indices (P-mode),
    /// versus needing actual RGB color values. Ops that operate on single-channel
    /// pixel values (indices) are safe. Color-dependent ops (filters, enhance,
    /// convert) need RGB.
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
                other.has_palette_mode() || other.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
            }
            PipelineOp::PutPixel {
                palette_index: true,
                ..
            } => true,
            PipelineOp::Rotate { fill: None, .. } => true,
            PipelineOp::Resize { filter, .. } | PipelineOp::ResizeBoxed { filter, .. } => {
                matches!(filter, ResampleFilter::Nearest)
            }
            PipelineOp::Thumbnail { filter, .. } => {
                matches!(filter, ResampleFilter::Nearest)
            }
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
            // ImageChops.invert applies 255-index directly to P samples. Its
            // newly allocated core intentionally has no palette.
            | PipelineOp::InvertChops
            // Pillow's Chops.c arithmetic also operates on raw indexed
            // samples for P/PA images. The result remains indexed, but its
            // newly allocated core has no palette attached.
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
            // Pillow mutates palette indices directly for putdata, while
            // putalpha promotes each index to a PA (index, alpha) pair.
            | PipelineOp::PutData {
                mode: crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA,
                ..
            }
            | PipelineOp::PutAlpha {
                mode: crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA,
                ..
            }
            | PipelineOp::PutAlphaData {
                mode: crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA,
                ..
            }
            // Pillow keeps P/PA images indexed through ImageOps.pad. The
            // resize step uses nearest-neighbour samples for these modes,
            // and the pad fill is another raw index/alpha pair.
            | PipelineOp::Pad { .. }
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

    /// Whether an indexed ImageChops result intentionally has no palette.
    ///
    /// Pillow's `libImaging/Chops.c` allocates a fresh indexed core for these
    /// operations and does not copy `Image.palette`. Keep the policy beside
    /// pipeline construction so every result is handled through the invariant
    /// that [`Self::push_op`] always creates an [`Image::Pipeline`].
    pub(super) fn indexed_chops_result_without_palette(op: &PipelineOp) -> bool {
        matches!(
            op,
            PipelineOp::InvertChops
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
        )
    }

    /// Returns whether an operation can stay in the source's indexed sample
    /// layout. Pillow's `libImaging/Filter.c` `PA` path applies convolution to
    /// the raw index/alpha bands and preserves the `PA` mode; unlike `P`, `PA`
    /// is a valid input for the built-in convolution filters. Keep this
    /// source-dependent exception out of [`Self::is_palette_safe_op`] so a
    /// direct filter on `P` cannot bypass `validate_filter` and become
    /// accepted.
    pub(super) fn is_palette_safe_op_for_source(source: &Image, op: &PipelineOp) -> bool {
        Self::is_palette_safe_op(op)
            // PA.putpixel writes the native index/alpha sample pair. The
            // operation does not depend on the RGB palette, but a later
            // conversion does, so keep the palette attached to the lazy
            // pipeline while the raw PA samples are updated.
            || (source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
                && matches!(op, PipelineOp::PutPixel { .. }))
            // PA resize filters the raw index/alpha pair without expanding
            // the palette. The newly allocated Pillow image does not carry
            // the source palette object, so output metadata clears it below.
            || (source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
                && matches!(op, PipelineOp::Resize { .. } | PipelineOp::ResizeBoxed { .. }))
            || (source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
                && matches!(
                    op,
                    PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
                ))
            // Pillow keeps PA rotation in its native index/alpha sample
            // layout, including bilinear/bicubic resampling and a two-element
            // fillcolor. The generic palette-safe rule only accepts a
            // rotation without a fill because P and PA fill values have
            // different semantics; this source-specific case carries the
            // already-normalized PA index/alpha pair through the
            // native-channel rotate kernel.
            || (source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
                && matches!(op, PipelineOp::Rotate { .. }))
            // Pillow keeps P rotation in its native index layout, including a
            // fillcolor. The rotate backend forces nearest sampling for P and
            // interprets the first normalized fill component as the raw index.
            || (source.known_mode_identity() == Some(Mode::ImagePalette8)
                && matches!(op, PipelineOp::Rotate { .. }))
            // Pillow's ImagingTransform applies affine/projective sampling to
            // native P/PA samples and preserves the indexed mode. Keep this
            // channel-wise operation indexed so a retained RGB palette is not
            // expanded to RGBA before the transform runs.
            || (matches!(
                source.known_mode_identity(),
                Some(Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            )
                && matches!(op, PipelineOp::Transform { .. }))
            // Pillow's ImageOps.fit keeps P/PA images indexed through the
            // boxed resize. P uses nearest-neighbour samples regardless of
            // the requested kernel; PA resamples its raw index/alpha bands.
            || (matches!(
                source.known_mode_identity(),
                Some(Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            )
                && matches!(op, PipelineOp::Fit { .. }))
            // Pillow's ImageOps.contain/cover are resize wrappers. Their
            // indexed paths preserve P/PA samples and the source mode while
            // applying the requested kernel (P still forces nearest inside
            // the resize implementation).
            || (matches!(
                source.known_mode_identity(),
                Some(Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            )
                && matches!(op, PipelineOp::Contain { .. } | PipelineOp::Cover { .. }))
    }

    /// Whether this value currently represents palette indices rather than
    /// visible luma samples. Lazy inputs use cached header metadata so the
    /// first queued operation does not need to decode merely to preserve mode.
    pub(crate) fn has_palette_mode(&self) -> bool {
        self.known_mode_identity() == Some(Mode::ImagePalette8)
    }

    /// Whether samples are palette indices, optionally paired with per-pixel
    /// alpha in PA mode.
    ///
    /// Keep this separate from `has_palette_mode`: callers such as
    /// `apply_transparency`, P encoding, and palette-index `putpixel` implement
    /// Pillow behavior that applies only to single-band P images.
    pub(super) fn has_palette_samples(&self) -> bool {
        self.has_palette_mode()
            || self.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
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
        let mode = self.mode_from_materialized(&image);
        validate_scalar_storage(&image, &mode)?;
        Ok(image)
    }

    /// Materialize a lazy image once before creating independent branches.
    ///
    /// Pillow's eager image objects can be cropped repeatedly without
    /// replaying an earlier resize.  Keeping the shared materialized pixels in
    /// a concrete image preserves that behavior for callers that branch a
    /// lazy pipeline, while each derived branch can still remain lazy.
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
                    decoded_mode: image.color().into(),
                    known_zero_fill: false,
                    explicit_mode: None,
                    palette: self.palette(),
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
    pub(crate) fn materialized_shared(&self) -> Result<Arc<DynamicImage>, PilError> {
        match self {
            Image::Loaded(data) => Ok(Arc::clone(&data.image)),
            Image::Paletted(data) => data
                .materialized
                .get_or_init(|| Ok(Arc::new(data.indices.clone())))
                .clone(),
            Image::Bytes {
                source,
                materialized,
                ..
            } => materialized
                .get_or_init(|| {
                    decoded_to_dynamic(source.decode().map_err(map_deferred_decode_error)?)
                        .map(Arc::new)
                })
                .clone(),
            Image::Pipeline {
                source,
                ops,
                explicit_mode,
                backend,
                palette,
                palette_alpha,
                materialized,
                ..
            } => materialized
                .get_or_init(|| {
                    let _origin_scope = ops.telemetry_origin_scope();
                    Self::evaluate_pipeline(
                        source,
                        ops.as_slice(),
                        explicit_mode,
                        *backend,
                        palette,
                        palette_alpha,
                        ops.prefix_cache(),
                    )
                })
                .clone(),
        }
    }

    /// Consumes an image into an owned materialization, reusing a unique
    /// cached pixel buffer when no other handle shares it.
    pub(crate) fn materialize_owned(self) -> Result<DynamicImage, PilError> {
        let shared = self.materialized_shared_for_ops()?;
        drop(shared);
        let cached = match self {
            Image::Loaded(data) => Ok(data.image),
            Image::Paletted(data) => take_cached_materialization(data.materialized),
            Image::Bytes { materialized, .. } | Image::Pipeline { materialized, .. } => {
                take_cached_materialization(materialized)
            }
        }?;
        Ok(Arc::try_unwrap(cached).unwrap_or_else(|shared| shared.as_ref().clone()))
    }

    /// Evaluates a pipeline without publishing its result into the pipeline's
    /// ordinary materialization cache.
    fn evaluate_pipeline(
        source: &Image,
        ops: &[PipelineOp],
        explicit_mode: &Option<Mode>,
        backend: Option<crate::compute::Backend>,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
        prefix_cache: Option<&PipelinePrefixCache>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        if let Some(prefix_cache) =
            prefix_cache.filter(|cache| cache.ops_len > 0 && cache.ops_len < ops.len())
        {
            let prefix_ops = &ops[..prefix_cache.ops_len];
            let prefix_image = prefix_cache
                .materialized
                .get_or_init(|| {
                    let prepared = crate::compute::prepare_execution(prefix_ops, backend)?;
                    let source_image = source.materialized_shared()?;
                    Self::evaluate_pipeline_with_image(
                        source,
                        source_image.as_ref(),
                        &source_image,
                        prefix_ops,
                        explicit_mode,
                        &prepared,
                        palette,
                        palette_alpha,
                    )
                })
                .clone()?;
            let suffix_ops = &ops[prefix_cache.ops_len..];
            let prepared = crate::compute::prepare_execution(suffix_ops, backend)?;
            let resolved_palette = palette.clone().or_else(|| source.extract_palette());
            let resolved_alpha = palette_alpha
                .clone()
                .or_else(|| source.palette_alpha())
                .unwrap_or_default();
            let inputs = crate::compute::registry::support::PaletteInput {
                rgb: resolved_palette.as_deref(),
                alpha: &resolved_alpha,
            };
            return crate::compute::execute_prepared_shared_with_palette(
                &prepared,
                suffix_ops,
                &prefix_image,
                super::mode_boundary::pillow_mode_name_opt(*explicit_mode),
                inputs,
            );
        }
        let prepared = crate::compute::prepare_execution(ops, backend)?;
        let source_image = source.materialized_shared()?;
        Self::evaluate_pipeline_with_image(
            source,
            source_image.as_ref(),
            &source_image,
            ops,
            explicit_mode,
            &prepared,
            palette,
            palette_alpha,
        )
    }

    pub(super) fn evaluate_pipeline_uncached(
        source: &Image,
        ops: &[PipelineOp],
        explicit_mode: &Option<Mode>,
        backend: Option<crate::compute::Backend>,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        let prepared = crate::compute::prepare_execution(ops, backend)?;
        let source_image = Arc::new(source.materialize_uncached()?);
        Self::evaluate_pipeline_with_image(
            source,
            source_image.as_ref(),
            &source_image,
            ops,
            explicit_mode,
            &prepared,
            palette,
            palette_alpha,
        )
    }

    fn evaluate_palette_grayscale(
        source: &Image,
        img: &DynamicImage,
        ops: &[PipelineOp],
        prepared: &crate::compute::PreparedExecution,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Option<Arc<DynamicImage>>, PilError> {
        if !source.has_palette_samples()
            || ops.len() != 1
            || !matches!(ops.first(), Some(PipelineOp::Grayscale))
            || prepared.context().backend() != crate::compute::Backend::Cpu
        {
            return Ok(None);
        }

        let palette_rgb = palette.clone().or_else(|| source.extract_palette());
        let palette_alpha = palette_alpha
            .clone()
            .or_else(|| source.palette_alpha())
            .unwrap_or_default();
        let input = crate::compute::registry::support::OperationInputs {
            image: img,
            palette: crate::compute::registry::support::PaletteInput {
                rgb: palette_rgb.as_deref(),
                alpha: &palette_alpha,
            },
        };
        let result = crate::compute::execute_prepared_cpu_arch_operation(
            prepared,
            &ops[0],
            input,
        )?;
        Ok(Some(Arc::new(result)))
    }

    fn evaluate_palette_pipeline(
        source: &Image,
        img: &DynamicImage,
        ops: &[PipelineOp],
        explicit_mode: &Option<Mode>,
        prepared: &crate::compute::PreparedExecution,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
        explicit_mode_name: Option<&str>,
    ) -> Result<Option<Arc<DynamicImage>>, PilError> {
        if !source.has_palette_samples() {
            return Ok(None);
        }
        let all_safe = ops
            .iter()
            .all(|op| Self::is_palette_safe_op_for_source(source, op));
        if all_safe {
            let palette_mode = if *explicit_mode == Some(Mode::ImagePaletteAlpha8)
                || source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
            {
                "PA"
            } else {
                "P"
            };
            let resolved_palette = palette.clone().or_else(|| source.extract_palette());
            let resolved_alpha = palette_alpha
                .clone()
                .or_else(|| source.palette_alpha())
                .unwrap_or_default();
            let inputs = crate::compute::registry::support::PaletteInput {
                rgb: resolved_palette.as_deref(),
                alpha: &resolved_alpha,
            };
            let result = crate::compute::execute_prepared_with_palette(
                prepared,
                ops,
                img,
                Some(palette_mode),
                inputs,
            )?;
            return Ok(Some(Arc::new(result)));
        }

        let palette_mode = if source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8) {
            "PA"
        } else {
            "P"
        };

        // PA always expands through a palette before color-dependent
        // operations, even when no palette is attached. Pillow's
        // libImaging/Convert.c indexed path treats an empty PA palette
        // as all black; falling through to the raw LA bytes would
        // incorrectly expose the index as grayscale (for example, index
        // 7 would become RGB(7, 7, 7)).
        let palette = palette.clone().or_else(|| source.extract_palette());
        if palette_mode == "PA" {
            let expanded = expand_palette_alpha(
                &img.to_luma_alpha8(),
                palette.as_deref().unwrap_or_default(),
            );
            let result = crate::compute::execute_prepared(
                prepared,
                ops,
                &expanded,
                explicit_mode_name,
            )?;
            return Ok(Some(Arc::new(result)));
        }
        if let Some(palette) = palette {
            let palette_alpha = palette_alpha
                .clone()
                .or_else(|| source.palette_alpha())
                .unwrap_or_default();
            let expanded = expand_palette(&img.to_luma8(), &palette, &palette_alpha);
            let result = crate::compute::execute_prepared(
                prepared,
                ops,
                &expanded,
                explicit_mode_name,
            )?;
            return Ok(Some(Arc::new(result)));
        }
        Ok(None)
    }

    fn compatibility_execution_mode(
        source: &Image,
        img: &DynamicImage,
        ops: &[PipelineOp],
        explicit_mode: &Option<Mode>,
        explicit_mode_name: Option<&str>,
    ) -> Result<Option<String>, PilError> {
        // RemapPalette needs the source mode to distinguish L samples from P
        // indices; its pipeline result may stay P-tagged.
        let remap_source_mode = if ops.len() == 1
            && matches!(ops.first(), Some(PipelineOp::RemapPalette { .. }))
        {
            Some(source.mode()?)
        } else {
            None
        };
        // A one-step conversion also needs the source tag. RGBX and RGBa share
        // storage but have different fourth-byte semantics.
        let convert_source_mode = if ops.len() == 1
            && matches!(ops.first(), Some(PipelineOp::Convert { .. }))
        {
            Some(source.mode()?)
        } else {
            None
        };
        // Encoded I;16 input uses generic Luma16 storage but retains its exact
        // source identity in the lazy image header.
        let source_luma16_mode = if remap_source_mode.is_none()
            && convert_source_mode.is_none()
            && explicit_mode.is_none()
            && matches!(img, DynamicImage::ImageI16(_))
        {
            Some(source.mode()?)
        } else {
            None
        };
        // Flattened pipelines start from the root buffer; an explicit output
        // mode must not reinterpret setup operations such as PutPixel.
        let initial_source_mode = if remap_source_mode.is_none()
            && convert_source_mode.is_none()
            && source_luma16_mode.is_none()
        {
            Some(source.mode()?)
        } else {
            None
        };
        let mode = remap_source_mode
            .or(convert_source_mode)
            .or(source_luma16_mode)
            .or(initial_source_mode)
            .or_else(|| explicit_mode_name.map(str::to_owned));
        Ok(mode)
    }

    fn evaluate_pipeline_with_image(
        source: &Image,
        img: &DynamicImage,
        shared_img: &Arc<DynamicImage>,
        ops: &[PipelineOp],
        explicit_mode: &Option<Mode>,
        prepared: &crate::compute::PreparedExecution,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        if let Some(result) = Self::evaluate_palette_grayscale(
            source,
            img,
            ops,
            prepared,
            palette,
            palette_alpha,
        )? {
            return Ok(result);
        }
        // TODO(cpu-migration): keep prepared execution typed through the pool;
        // this projection is needed only by the legacy string-mode executor.
        let explicit_mode_name = super::mode_boundary::pillow_mode_name_opt(*explicit_mode);
        if let Some(result) = Self::evaluate_palette_pipeline(
            source,
            img,
            ops,
            explicit_mode,
            prepared,
            palette,
            palette_alpha,
            explicit_mode_name,
        )? {
            return Ok(result);
        }
        // RGBX carries padding rather than alpha; keep its source spelling at
        // the compatibility executor boundary instead of normalizing pixels.
        let execution_mode = Self::compatibility_execution_mode(
            source,
            img,
            ops,
            explicit_mode,
            explicit_mode_name,
        )?;
        let resolved_palette = if palette.is_none() {
            source.extract_palette()
        } else {
            None
        };
        let resolved_alpha = if palette_alpha.is_none() {
            source.palette_alpha()
        } else {
            None
        };
        let inputs = crate::compute::registry::support::PaletteInput {
            rgb: palette.as_deref().or(resolved_palette.as_deref()),
            alpha: palette_alpha
                .as_deref()
                .or(resolved_alpha.as_deref())
                .unwrap_or_default(),
        };
        crate::compute::execute_prepared_shared_with_palette(
            prepared,
            ops,
            shared_img,
            execution_mode.as_deref(),
            inputs,
        )
    }

    fn materialize_uncached(&self) -> Result<DynamicImage, PilError> {
        match self {
            Image::Loaded(data) => Ok(data.image.as_ref().clone()),
            Image::Paletted(data) => Ok(data.indices.clone()),
            Image::Bytes { source, .. } => decoded_to_dynamic(
                &image_slash_star::decode(source.bytes()).map_err(map_deferred_decode_error)?,
            ),
            Image::Pipeline {
                source,
                ops,
                explicit_mode,
                backend,
                palette,
                palette_alpha,
                ..
            } => {
                let _origin_scope = ops.telemetry_origin_scope();
                Self::evaluate_pipeline_uncached(
                    source,
                    ops,
                    explicit_mode,
                    *backend,
                    palette,
                    palette_alpha,
                )
                .and_then(|image| {
                    let image = image.as_ref().clone();
                    let Some(mode) = explicit_mode else {
                        return Ok(image);
                    };
                    let Some(name) = super::mode_boundary::pillow_mode_name(*mode) else {
                        return Ok(image);
                    };
                    image.with_mode(*mode).map_err(|_| {
                        PilError::InternalError(format!(
                            "pipeline output storage does not match Pillow mode {}",
                            name
                        ))
                    })
                })
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
