//! Lazy materialization and indexed-mode pipeline policy for [`Image`].

use std::sync::Arc;

use image_slash_star::{Decoded, DecodedImage, ImageFormat, ImageInfo};

use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::DynamicImage;

use super::{
    Image, ImageMode, LoadedData, Mode, PalettedData, PipelinePrefixCache, expand_palette,
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
    if decoded_mode == ImageMode::P8 {
        let palette = info
            .as_ref()
            .and_then(|info| info.palette.clone())
            .ok_or_else(|| {
                PilError::PaletteError("decoded P-mode image has no retained palette".to_owned())
            })?;
        return Ok(Image::Paletted(PalettedData {
            indices: image.as_ref().clone().to_luma8(),
            palette: padded_palette(&palette.rgb),
            palette_alpha: palette.alpha,
            source_format,
            info,
            exif: exif.clone(),
            materialized: materialization_cache(),
        }));
    }
    let explicit_mode = match decoded_mode {
        ImageMode::L1 | ImageMode::Cmyk8 | ImageMode::F32 | ImageMode::I32 => {
            Some(image_mode_name(decoded_mode).to_owned())
        }
        _ => None,
    };
    let mode = super::Mode::for_image(&image, explicit_mode.as_deref());
    Ok(Image::Loaded(LoadedData {
        image,
        mode,
        known_zero_fill: false,
        explicit_mode,
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

fn decoded_to_dynamic(decoded: &Decoded<DecodedImage>) -> Result<DynamicImage, PilError> {
    let content = &decoded.content;
    match content.mode {
        ImageMode::P8 => crate::raster::GrayImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::DimensionError("invalid indexed buffer".to_owned())),
        ImageMode::L1 => {
            let row_bytes = content.width.div_ceil(8) as usize;
            let mut unpacked = Vec::with_capacity(content.width as usize * content.height as usize);
            for row in content.pixels.chunks_exact(row_bytes) {
                for x in 0..content.width as usize {
                    let bit = (row[x / 8] >> (7 - (x % 8))) & 1;
                    unpacked.push(if bit == 0 { 0 } else { 255 });
                }
            }
            crate::raster::GrayImage::from_raw(content.width, content.height, unpacked)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::DimensionError("invalid packed bilevel buffer".to_owned()))
        }
        ImageMode::Cmyk8 | ImageMode::F32 | ImageMode::I32 => crate::raster::RgbaImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| PilError::DimensionError("invalid four-byte buffer".to_owned())),
        mode => DynamicImage::from_decoded(content)
            .ok_or_else(|| PilError::ValueError(format!("unsupported decoded mode {mode:?}"))),
    }
}

fn retained_palette(
    source: &Image,
    palette: &Option<Vec<u8>>,
    palette_alpha: &Option<Vec<u8>>,
) -> (Vec<u8>, Vec<u8>) {
    (
        palette.clone().or_else(|| source.extract_palette()).unwrap_or_default(),
        palette_alpha
            .clone()
            .or_else(|| source.palette_alpha())
            .unwrap_or_default(),
    )
}

fn expanded_palette_mode(image: &DynamicImage) -> Result<&'static str, PilError> {
    match image {
        DynamicImage::ImageRgb8(_) => Ok("RGB"),
        DynamicImage::ImageRgba8(_) => Ok("RGBA"),
        _ => Err(PilError::InternalError(
            "palette expansion did not produce RGB or RGBA samples".to_owned(),
        )),
    }
}

fn palette_conversion_ignores_alpha(ops: &[PipelineOp]) -> bool {
    matches!(
        ops.first(),
        Some(PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. })
    )
}

fn palette_native_prefix_len(source: &Image, mode: Mode, ops: &[PipelineOp]) -> (usize, bool) {
    if mode != Mode::ImagePalette8 {
        return (0, true);
    }
    let mut prefix_len = 0;
    let mut current_mode = "P".to_owned();
    for op in ops {
        if !Image::is_palette_safe_op_for_source(source, op) {
            break;
        }
        let next_mode = super::known_pipeline_op_mode(op, &current_mode)
            .unwrap_or_else(|| current_mode.clone());
        if next_mode != "P" {
            return (prefix_len, false);
        }
        current_mode = next_mode;
        prefix_len += 1;
    }
    (prefix_len, true)
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
                other.has_palette_mode() || other.explicit_mode() == Some("PA")
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
            // NO_PILLOW_SUPPORT: Pillow ImageOps._lut rejects P; CPU
            // Autocontrast, Invert, Posterize, and Solarize operate on native
            // indices and keep the source palette attached.
            || (source.explicit_mode() == Some("P")
                && matches!(
                    op,
                    PipelineOp::Autocontrast { .. }
                        | PipelineOp::Invert
                        | PipelineOp::Posterize { .. }
                        | PipelineOp::Solarize { .. }
                ))
            // PA.putpixel writes the native index/alpha sample pair. The
            // operation does not depend on the RGB palette, but a later
            // conversion does, so keep the palette attached to the lazy
            // pipeline while the raw PA samples are updated.
            || (source.explicit_mode() == Some("PA")
                && matches!(op, PipelineOp::PutPixel { .. }))
            // PA resize filters the raw index/alpha pair without expanding
            // the palette. The newly allocated Pillow image does not carry
            // the source palette object, so output metadata clears it below.
            || (source.explicit_mode() == Some("PA")
                && matches!(op, PipelineOp::Resize { .. } | PipelineOp::ResizeBoxed { .. }))
            || (source.explicit_mode() == Some("PA")
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
            || (source.explicit_mode() == Some("PA")
                && matches!(op, PipelineOp::Rotate { .. }))
            // Pillow keeps P rotation in its native index layout, including a
            // fillcolor. The rotate backend forces nearest sampling for P and
            // interprets the first normalized fill component as the raw index.
            || (source.explicit_mode() == Some("P")
                && matches!(op, PipelineOp::Rotate { .. }))
            // Pillow's ImagingTransform applies affine/projective sampling to
            // native P/PA samples and preserves the indexed mode. Keep this
            // channel-wise operation indexed so a retained RGB palette is not
            // expanded to RGBA before the transform runs.
            || (matches!(source.explicit_mode(), Some("P") | Some("PA"))
                && matches!(op, PipelineOp::Transform { .. }))
            // Pillow's ImageOps.fit keeps P/PA images indexed through the
            // boxed resize. P uses nearest-neighbour samples regardless of
            // the requested kernel; PA resamples its raw index/alpha bands.
            || (matches!(source.explicit_mode(), Some("P") | Some("PA"))
                && matches!(op, PipelineOp::Fit { .. }))
            // Pillow's ImageOps.contain/cover are resize wrappers. Their
            // indexed paths preserve P/PA samples and the source mode while
            // applying the requested kernel (P still forces nearest inside
            // the resize implementation).
            || (matches!(source.explicit_mode(), Some("P") | Some("PA"))
                && matches!(op, PipelineOp::Contain { .. } | PipelineOp::Cover { .. }))
    }

    /// Whether this value currently represents palette indices rather than
    /// visible luma samples. Lazy inputs use cached header metadata so the
    /// first queued operation does not need to decode merely to preserve mode.
    pub(crate) fn has_palette_mode(&self) -> bool {
        match self {
            Image::Paletted(_) => true,
            Image::Bytes {
                info: Some(info), ..
            } => info.mode == ImageMode::P8,
            Image::Pipeline {
                explicit_mode: Some(mode),
                ..
            }
            | Image::Loaded(LoadedData {
                explicit_mode: Some(mode),
                ..
            }) => mode == "P",
            _ => false,
        }
    }

    /// Whether samples are palette indices, optionally paired with per-pixel
    /// alpha in PA mode.
    ///
    /// Keep this separate from `has_palette_mode`: callers such as
    /// `apply_transparency`, P encoding, and palette-index `putpixel` implement
    /// Pillow behavior that applies only to single-band P images.
    pub(super) fn has_palette_samples(&self) -> bool {
        self.has_palette_mode() || self.explicit_mode() == Some("PA")
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
                let explicit_mode = self.explicit_mode().map(str::to_owned);
                let mode = super::Mode::for_image(&image, explicit_mode.as_deref());
                Ok(Image::Loaded(LoadedData {
                    decoded_mode: image.color().into(),
                    mode,
                    known_zero_fill: false,
                    explicit_mode,
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
                .get_or_init(|| Ok(Arc::new(DynamicImage::ImageLuma8(data.indices.clone()))))
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
        explicit_mode: &Option<String>,
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
                        &None,
                        &None,
                    )
                })
                .clone()?;
            let suffix_ops = &ops[prefix_cache.ops_len..];
            let prepared = crate::compute::prepare_execution(suffix_ops, backend)?;
            return crate::compute::execute_prepared_shared(
                &prepared,
                suffix_ops,
                &prefix_image,
                explicit_mode.as_deref(),
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
        explicit_mode: &Option<String>,
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

    fn evaluate_pipeline_with_image(
        source: &Image,
        img: &DynamicImage,
        shared_img: &Arc<DynamicImage>,
        ops: &[PipelineOp],
        explicit_mode: &Option<String>,
        prepared: &crate::compute::PreparedExecution,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        if source.has_palette_samples() {
            return Self::evaluate_palette_pipeline(
                source,
                img,
                ops,
                prepared,
                palette,
                palette_alpha,
            );
        }

        // RemapPalette's kernel needs the source mode to distinguish L samples
        // from P indices. The pipeline remains P-tagged for the result, but
        // passing that destination tag into the kernel would skip the public
        // L-source path and reinterpret luma values as palette indices.
        let remap_source_mode =
            if ops.len() == 1 && matches!(ops.first(), Some(PipelineOp::RemapPalette { .. })) {
                Some(source.mode()?)
            } else {
                None
            };
        // A one-step conversion still needs the source-only mode tag at the
        // executor boundary. RGBX and RGBa share RGBA storage, but their fourth
        // byte has different Pillow semantics; the pipeline output tag is the
        // destination and cannot stand in for that source tag.
        let convert_source_mode =
            if ops.len() == 1 && matches!(ops.first(), Some(PipelineOp::Convert { .. })) {
                // The concrete buffer is still in the source layout when a
                // one-step conversion begins. In particular, RGB -> CMYK/HSV/
                // YCbCr carries a nonstandard destination tag on the lazy result,
                // but preflight and the adapter must inspect the source mode.
                Some(source.mode()?)
            } else {
                None
            };
        // Encoded I;16 inputs decode to the generic Luma16 buffer while their
        // source-only mode remains in the lazy Bytes header. Preserve that
        // mode at the executor boundary so a strict SIMD resize can select
        // the native two-byte sample kernel instead of rejecting the image as
        // an untagged layout.
        let source_luma16_mode = if remap_source_mode.is_none()
            && convert_source_mode.is_none()
            && explicit_mode.is_none()
            && matches!(img, DynamicImage::ImageLuma16(_))
        {
            Some(source.mode()?)
        } else {
            None
        };
        // A flattened pipeline still starts with the root source buffer.  A
        // terminal Convert/Colorize/etc. may set `explicit_mode` to the
        // destination mode, but that tag must not be used to interpret the
        // bytes written by earlier setup operations such as PutPixel.  Keep
        // the source layout as the initial GPU mode and let the batch planner
        // advance it after each representable mode-changing operation.
        let initial_source_mode = if remap_source_mode.is_none()
            && convert_source_mode.is_none()
            && source_luma16_mode.is_none()
        {
            Some(source.mode()?)
        } else {
            None
        };
        let execution_mode = remap_source_mode
            .as_deref()
            .or(convert_source_mode.as_deref())
            .or(source_luma16_mode.as_deref())
            .or(initial_source_mode.as_deref())
            .or(explicit_mode.as_deref());
        // RGBX uses RGBA storage but its fourth byte is padding, not an alpha
        // sample. The native SIMD Convert kernel carries that logical source
        // tag and supplies opaque alpha while gathering; do not normalize the
        // whole frame here just to rewrite the padding byte.
        crate::compute::execute_prepared_shared(prepared, ops, shared_img, execution_mode)
    }

    fn evaluate_palette_pipeline(
        source: &Image,
        img: &Arc<DynamicImage>,
        ops: &[PipelineOp],
        prepared: &crate::compute::PreparedExecution,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        let (palette_mode, palette_mode_name) = match source.mode_selector()? {
            Mode::ImagePalette8 => (Mode::ImagePalette8, "P"),
            Mode::ImagePaletteAlpha8 => (Mode::ImagePaletteAlpha8, "PA"),
            mode => {
                return Err(PilError::InternalError(format!(
                    "palette samples carry incompatible mode {mode:?}"
                )));
            }
        };
        if ops
            .iter()
            .all(|op| Self::is_palette_safe_op_for_source(source, op))
        {
            return crate::compute::execute_prepared_shared(
                prepared,
                ops,
                img,
                Some(palette_mode_name),
            );
        }

        let (prefix_len, prefix_keeps_p_mode) =
            palette_native_prefix_len(source, palette_mode, ops);
        if !prefix_keeps_p_mode {
            // TODO(pillow-rs#3ptb): Route manually constructed flattened
            // pipelines with P->PA/L transitions through an intermediate
            // typed mode node. Image::push_op already creates that boundary.
        } else if prefix_len > 0 && prefix_len < ops.len() {
            return Self::execute_palette_prefix_and_color_suffix(
                source,
                img,
                prepared,
                ops,
                prefix_len,
                palette,
                palette_alpha,
            );
        }

        Self::execute_palette_pipeline_after_expansion(
            source,
            img,
            ops,
            prepared,
            palette_mode,
            palette,
            palette_alpha,
        )
    }

    fn execute_palette_prefix_and_color_suffix(
        source: &Image,
        img: &Arc<DynamicImage>,
        prepared: &crate::compute::PreparedExecution,
        ops: &[PipelineOp],
        prefix_len: usize,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        // TODO(pillow-rs#3ptb): Aggregate telemetry across the indexed
        // prefix and the color suffix instead of retaining only the last
        // executor receipt.
        let (indexed_prefix, suffix_ops) = ops.split_at(prefix_len);
        let indexed = crate::compute::execute_prepared_shared(
            prepared,
            indexed_prefix,
            img,
            Some("P"),
        )?;
        let DynamicImage::ImageLuma8(indices) = indexed.as_ref() else {
            return Err(PilError::InternalError(
                "P-mode prefix did not produce native index samples".to_owned(),
            ));
        };
        let (palette, palette_alpha) = retained_palette(source, palette, palette_alpha);
        let expansion_alpha = if palette_conversion_ignores_alpha(suffix_ops) {
            &[]
        } else {
            &palette_alpha
        };
        let expanded = expand_palette(indices, &palette, expansion_alpha);
        let expanded_mode = expanded_palette_mode(&expanded)?;
        crate::compute::execute_prepared_shared(
            prepared,
            suffix_ops,
            &Arc::new(expanded),
            Some(expanded_mode),
        )
    }

    fn execute_palette_pipeline_after_expansion(
        source: &Image,
        img: &Arc<DynamicImage>,
        ops: &[PipelineOp],
        prepared: &crate::compute::PreparedExecution,
        palette_mode: Mode,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        let (palette, palette_alpha) = retained_palette(source, palette, palette_alpha);
        let expanded = match palette_mode {
            Mode::ImagePaletteAlpha8 => {
                let DynamicImage::ImageLumaA8(indices_alpha) = img.as_ref() else {
                    return Err(PilError::InternalError(
                        "PA image did not contain native index-alpha samples".to_owned(),
                    ));
                };
                expand_palette_alpha(indices_alpha, &palette)
            }
            Mode::ImagePalette8 => {
                let DynamicImage::ImageLuma8(indices) = img.as_ref() else {
                    return Err(PilError::InternalError(
                        "P image did not contain native index samples".to_owned(),
                    ));
                };
                let expansion_alpha = if palette_conversion_ignores_alpha(ops) {
                    &[]
                } else {
                    &palette_alpha
                };
                expand_palette(indices, &palette, expansion_alpha)
            }
            _ => {
                return Err(PilError::InternalError(
                    "palette materialization lost its indexed mode".to_owned(),
                ));
            }
        };
        let expanded_mode = expanded_palette_mode(&expanded)?;
        crate::compute::execute_prepared_shared(
            prepared,
            ops,
            &Arc::new(expanded),
            Some(expanded_mode),
        )
    }

    fn materialize_uncached(&self) -> Result<DynamicImage, PilError> {
        match self {
            Image::Loaded(data) => Ok(data.image.as_ref().clone()),
            Image::Paletted(data) => Ok(DynamicImage::ImageLuma8(data.indices.clone())),
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
                .map(|image| image.as_ref().clone())
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
