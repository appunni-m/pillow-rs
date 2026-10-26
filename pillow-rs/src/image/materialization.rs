//! Lazy materialization and indexed-mode pipeline policy for [`Image`].

use std::sync::Arc;

use image_slash_star::{Decoded, DecodedImage, ImageFormat, ImageInfo};

use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::DynamicImage;

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
    let mode = info
        .as_ref()
        .map_or_else(|| image.color().into(), |info| info.mode);
    if mode == ImageMode::P8 {
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
    let explicit_mode = match mode {
        ImageMode::L1 | ImageMode::Cmyk8 | ImageMode::F32 | ImageMode::I32 => {
            Some(image_mode_name(mode).to_owned())
        }
        _ => None,
    };
    Ok(Image::Loaded(LoadedData {
        image,
        known_zero_fill: false,
        explicit_mode,
        decoded_mode: mode,
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
            // PA.putpixel writes the native index/alpha sample pair. The
            // operation does not depend on the RGB palette, but a later
            // conversion does, so keep the palette attached to the lazy
            // pipeline while the raw PA samples are updated.
            || (source.explicit_mode() == Some("PA")
                && matches!(op, PipelineOp::PutPixel { .. }))
            // PA resize filters the raw index/alpha pair without expanding
            // the palette. Preserve that palette for later conversions.
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
                Ok(Image::Loaded(LoadedData {
                    decoded_mode: image.color().into(),
                    known_zero_fill: false,
                    explicit_mode: self.explicit_mode().map(str::to_owned),
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
            let all_safe = ops
                .iter()
                .all(|op| Self::is_palette_safe_op_for_source(source, op));
            if all_safe {
                let palette_mode = if explicit_mode.as_deref() == Some("PA")
                    || source.explicit_mode() == Some("PA")
                {
                    "PA"
                } else {
                    "P"
                };
                let result =
                    crate::compute::execute_prepared(prepared, ops, img, Some(palette_mode))?;
                return Ok(Arc::new(result));
            }

            let palette_mode = if source.explicit_mode() == Some("PA") {
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
                    explicit_mode.as_deref(),
                )?;
                return Ok(Arc::new(result));
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
                    explicit_mode.as_deref(),
                )?;
                return Ok(Arc::new(result));
            }
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

impl Image {
    /// Materializes the image for color-space image operations.
    ///
    /// `P` images are converted through their palette to RGB so operations such
    /// as paste, composite, and filters operate on visible colors. Use
    /// [`Image::materialize_indices`] when an operation needs palette indices.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy decoding or pipeline execution fails.
    pub fn materialize_for_ops(&self) -> Result<DynamicImage, PilError> {
        // Fast path: Paletted images have direct palette→RGB conversion
        if let Some(rgb) = self.paletted_to_rgb() {
            return Ok(rgb);
        }
        // Pipeline-based P-mode images need palette→RGB conversion
        if matches!(self, Image::Pipeline { .. }) && self.explicit_mode() == Some("P") {
            let img = self.materialize()?; // Luma8 indices
            if let Some(palette) = self.palette() {
                let indices = img.to_luma8();
                let palette_alpha = self.palette_alpha().unwrap_or_default();
                return Ok(expand_palette(&indices, &palette, &palette_alpha));
            }
        }
        self.materialize()
    }

    /// Convert Paletted image to RGB for rendering/saving. Returns None for non-Paletted.
    pub(crate) fn paletted_to_rgb(&self) -> Option<DynamicImage> {
        if let Image::Paletted(data) = self {
            Some(expand_palette(
                &data.indices,
                &data.palette,
                &data.palette_alpha,
            ))
        } else {
            None
        }
    }

    /// Forces lazy decoding and pipeline execution.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn load(&mut self) -> Result<(), PilError> {
        let loaded = match &*self {
            Image::Loaded(_) | Image::Paletted(_) => return Ok(()),
            Image::Bytes { format, info, .. } => image_from_materialized(
                self.materialized_shared()?,
                *format,
                info.clone(),
                self.exif_metadata(),
            )?,
            Image::Pipeline {
                source,
                explicit_mode,
                format,
                palette,
                palette_alpha,
                ..
            } => {
                if explicit_mode.as_deref() == Some("P") {
                    if let Some(palette) = palette {
                        let indices = self.materialized_shared()?.as_ref().clone().to_luma8();
                        Image::Paletted(PalettedData {
                            indices,
                            palette: palette.clone(),
                            palette_alpha: palette_alpha.clone().unwrap_or_default(),
                            source_format: *format,
                            info: source.image_info(),
                            exif: source.exif_metadata(),
                            materialized: materialization_cache(),
                        })
                    } else {
                        return Err(PilError::PaletteError(
                            "P-mode pipeline has no retained palette".to_owned(),
                        ));
                    }
                } else {
                    let image = self.materialized_shared()?;
                    let decoded_mode = image.color().into();
                    Image::Loaded(LoadedData {
                        image,
                        known_zero_fill: false,
                        explicit_mode: explicit_mode.clone(),
                        decoded_mode,
                        palette: palette.clone(),
                        palette_alpha: palette_alpha.clone(),
                        source_format: *format,
                        info: source.image_info(),
                        exif: source.exif_metadata(),
                    })
                }
            }
        };
        *self = loaded;
        Ok(())
    }

    /// Returns whether this handle can reuse successfully materialized pixels.
    pub fn is_materialized(&self) -> bool {
        match self {
            Image::Loaded(_) | Image::Paletted(_) => true,
            Image::Bytes { materialized, .. } | Image::Pipeline { materialized, .. } => {
                matches!(materialized.get(), Some(Ok(_)))
            }
        }
    }

    /// Fully validates an encoded or deferred image without changing its state.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when decoding or pipeline execution fails.
    pub fn verify(&self) -> Result<(), PilError> {
        match self {
            Image::Bytes { source, .. } => source
                .verify()
                .map_err(|error| map_codec_error(error, "memory"))?,
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
                )?;
            }
            Image::Loaded(_) | Image::Paletted(_) => {}
        }
        Ok(())
    }
}
