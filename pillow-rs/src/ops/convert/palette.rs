#[cfg(test)]
use std::borrow::Cow;
use std::sync::Arc;

use crate::compute::Backend;
use crate::compute::registry::support::{OperationInputs, PaletteInput};
use crate::error::PilError;
use crate::image::{Image, LoadedData, PipelineOps};
use crate::raster::{DynamicImage, Mode};

/// Per-entry alpha table Pillow applies when converting a palette image to an
/// alpha band: a single transparent index becomes 0 with everything else 255,
/// a PNG `tRNS` table is used verbatim.
pub(super) fn palette_alpha_for_convert(img: &Image) -> Option<Vec<u8>> {
    if let Some(transparency) = img.pending_palette_transparency() {
        return match transparency {
            crate::image::PaletteTransparency::Index(index) => {
                let mut table = vec![255u8; 256];
                table[usize::from(index)] = 0;
                Some(table)
            }
            crate::image::PaletteTransparency::Table(alpha) => Some(alpha),
        };
    }
    // `putpalette(..., rawmode="RGBA")` stores attached alpha on the
    // committed palette rather than in pending `info["transparency"]`.
    // Pillow's P->PA conversion consumes both representations.
    img.palette_alpha().filter(|alpha| !alpha.is_empty())
}

#[cfg(test)]
pub(super) fn split_rgba_for_palette(
    source: &DynamicImage,
) -> Result<(crate::raster::RgbImage, Vec<u8>), PilError> {
    let rgba: Cow<'_, crate::raster::RgbaImage> = match source {
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba),
        other => Cow::Owned(other.to_rgba8()),
    };
    let width = rgba.width();
    let height = rgba.height();
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    let rgb_len = pixel_count
        .checked_mul(3)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    let rgba_len = pixel_count
        .checked_mul(4)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    let input = rgba
        .as_raw()
        .get(..rgba_len)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    let mut rgb = Vec::with_capacity(rgb_len);
    let mut alpha = Vec::with_capacity(pixel_count);
    for pixel in input.chunks_exact(4) {
        rgb.extend_from_slice(&pixel[..3]);
        alpha.push(pixel[3]);
    }
    let rgb = crate::raster::RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
    Ok((rgb, alpha))
}

/// Attach a prepared P/PA raster and its wrapper-owned metadata to an `Image`.
///
/// Keep the generated palette and metadata policy outside the pixel kernel so
/// each exact source-mode strategy can return its native index raster.
pub(super) fn convert_prepared_palette(
    image: &Image,
    source_mode: Mode,
    target: Mode,
    dither: Option<&str>,
) -> Result<Image, PilError> {
    let is_scalar_source = matches!(source_mode, Mode::ImageI32 | Mode::ImageF32);
    let should_parse_dither =
        !(source_mode == Mode::ImagePaletteAlpha8 && target == Mode::ImagePalette8);
    let source_normalizes_before_palette = is_scalar_source
        || (target == Mode::ImagePalette8
            && matches!(
                source_mode,
                Mode::ImageCmyk8 | Mode::ImageHsv8 | Mode::ImageYCbCr8
            ));
    let parse_before_materialize = should_parse_dither
        && (target == Mode::ImagePaletteAlpha8 || !source_normalizes_before_palette);
    let mut prepared_dither = if parse_before_materialize {
        super::input::parse_dither(dither)?
    } else {
        None
    };
    let (source, mut operation_receipts) = image.materialized_shared_for_ops_with_receipts()?;
    if should_parse_dither && !parse_before_materialize {
        // Scalar I/F and nonstandard-to-P conversions normalized their source
        // before parsing dither; retain that validation order in the prepared route.
        prepared_dither = super::input::parse_dither(dither)?;
    }
    let source_palette = if matches!(source_mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        image.palette()
    } else {
        None
    };
    let palette_alpha = match source_mode {
        Mode::ImagePalette8 => palette_alpha_for_convert(image).unwrap_or_default(),
        Mode::ImagePaletteAlpha8 => image.palette_alpha().unwrap_or_default(),
        _ => Vec::new(),
    };
    let input = OperationInputs {
        image: source.as_ref(),
        palette: PaletteInput {
            rgb: source_palette.as_deref(),
            alpha: &palette_alpha,
        },
    };
    let operation = crate::pipeline::PipelineOp::Convert {
        mode: target,
        matrix: None,
        dither: prepared_dither,
    };
    let (selected_backend, context) = image.cpu_arch_execution_route()?;
    let fallback_reason = (selected_backend == Some(Backend::Gpu)).then(|| {
        format!(
            "Convert palette metadata is produced by the CPU wrapper route; used CPU {:?}",
            context.architecture()
        )
    });
    let mut palette_output = None;
    let raster = crate::compute::execute_eager_cpu_arch_image_operation_with_context_and_receipts(
        "Convert",
        source.as_ref(),
        context,
        selected_backend,
        fallback_reason,
        &mut operation_receipts,
        |context| {
            let prepared = crate::compute::registry::prepare_cpu(&operation, input, context)?;
            let selection = *prepared.selection();
            let output = prepared.execute_convert_output()?;
            palette_output = Some((
                output.palette,
                output.palette_alpha,
                output.wrapper_metadata,
            ));
            Ok((output.image, selection))
        },
    )?;
    let (palette, palette_alpha, wrapper_metadata) = palette_output.ok_or_else(|| {
        PilError::InternalError("prepared palette conversion omitted its result metadata".into())
    })?;
    let wrapper_metadata = wrapper_metadata.ok_or_else(|| {
        PilError::InternalError(
            "prepared palette conversion omitted wrapper metadata policy".into(),
        )
    })?;
    wrap_prepared_palette(
        image,
        target,
        raster,
        palette,
        palette_alpha,
        wrapper_metadata,
    )
}

fn wrap_prepared_palette(
    image: &Image,
    target: Mode,
    raster: DynamicImage,
    palette: Option<Vec<u8>>,
    palette_alpha: Option<Vec<u8>>,
    metadata: crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata,
) -> Result<Image, PilError> {
    let (source_format, info, exif) = match metadata {
        crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::PreserveSource
        | crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::PreserveSourceLoaded => {
            (image.source_format(), image.image_info(), image.exif_metadata())
        }
        crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::ExifOnly => {
            (None, None, image.exif_metadata())
        }
        crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::Drop => {
            (None, None, None)
        }
    };
    match target {
        Mode::ImagePalette8 => {
            if matches!(
                metadata,
                crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::PreserveSourceLoaded
            ) {
                return Ok(Image::Loaded(LoadedData {
                    image: Arc::new(raster),
                    known_zero_fill: false,
                    palette: image.palette(),
                    palette_alpha: image.palette_alpha(),
                    source_format,
                    info,
                    exif,
                }));
            }
            let palette = palette.ok_or_else(|| {
                PilError::InternalError("prepared P conversion omitted RGB entries".into())
            })?;
            let source = LoadedData {
                image: Arc::new(raster),
                known_zero_fill: false,
                palette: None,
                palette_alpha: None,
                source_format,
                info,
                exif,
            };
            Ok(Image::Pipeline {
                source: Arc::new(Image::Loaded(source)),
                ops: PipelineOps::empty(),
                format: None,
                backend: None,
                palette: Some(palette),
                palette_alpha,
                compatibility_icc_profile: image.compatibility_icc_profile().map(Arc::from),
                materialized: crate::image::materialization_cache(),
                shape: crate::image::pipeline_shape_cache(),
                mode: crate::image::pipeline_mode_cache_for(Some(Mode::ImagePalette8)),
            })
        }
        Mode::ImagePaletteAlpha8 => Ok(Image::Loaded(LoadedData {
            image: Arc::new(raster),
            known_zero_fill: false,
            palette,
            palette_alpha,
            source_format,
            info,
            exif,
        })),
        _ => Err(PilError::InternalError(
            "prepared palette wrapper received a non-palette target".into(),
        )),
    }
}

