use crate::color;
use crate::error::PilError;
use crate::image::Image;
use crate::raster::DynamicImage;

use super::input::parse_dither;
use super::mode::is_nonstandard_mode;
use super::palette::{convert_to_palette_alpha, palette_alpha_for_convert};
use super::support::explicit_mode_for;

pub(super) fn try_convert_scalar_source(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    matrix: Option<Vec<f64>>,
    dither: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    if !matches!(source_mode, "I" | "F") {
        return Ok(None);
    }
    if target_mode == "PA" {
        return Ok(None);
    }
    with_eager_cpu_convert_receipt(
        || {
            if target_mode == "L" {
                let source = image.materialized_shared_for_ops()?;
                let luma = if source_mode == "I" {
                    color::i_to_l(source.as_ref())
                } else {
                    color::f_to_l(source.as_ref())
                };
                return Ok(Some(Image::from_dynamic(luma, None)));
            }
            let source = image.materialize()?;
            let converted = match (source_mode, target_mode) {
                ("I", "F") => color::i_to_f(&source),
                ("I", "RGB") => color::i_to_rgb(&source),
                ("F", "I") => color::f_to_i(&source),
                ("F", "RGB") => color::f_to_rgb(&source),
                _ => {
                    let luma = if source_mode == "I" {
                        color::i_to_l(&source)
                    } else {
                        color::f_to_l(&source)
                    };
                    return Ok(Some(Image::from_dynamic(luma, None).convert(
                        target_mode,
                        matrix,
                        dither,
                        None,
                        colors,
                    )?));
                }
            };
            Ok(Some(Image::from_dynamic(
                converted,
                explicit_mode_for(target_mode),
            )))
        },
        |result| result.as_ref().map_or(0, eager_convert_image_bytes),
    )
}

pub(super) fn try_convert_between_nonstandard_modes(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    matrix: Option<Vec<f64>>,
    dither: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    if !is_nonstandard_mode(source_mode)
        || !matches!(target_mode, "HSV" | "YCbCr" | "P" | "I" | "F")
        || target_mode == source_mode
        || (source_mode == "CMYK" && matches!(target_mode, "I" | "F"))
    {
        return Ok(None);
    }
    let palette = image.palette();
    let source = image.materialize()?;
    let converted = if source_mode == "PA" {
        crate::image::expand_palette_alpha(
            &source.to_luma_alpha8(),
            palette.as_deref().unwrap_or_default(),
        )
    } else {
        color::convert_from_nonstandard(source_mode, &source, palette.as_deref())
            .unwrap_or_else(|| source.to_rgb8().into())
    };
    let rgb_source = Image::from_dynamic(converted, None);
    Ok(Some(rgb_source.convert(
        target_mode,
        matrix,
        dither,
        None,
        colors,
    )?))
}

pub(super) fn try_convert_to_palette_alpha(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    dither: Option<&str>,
) -> Result<Option<Image>, PilError> {
    if target_mode != "PA" {
        return Ok(None);
    }
    let dither = parse_dither(dither)?;
    if matches!(source_mode, "I" | "F") {
        return with_eager_cpu_convert_receipt(
            || Ok(Some(convert_to_palette_alpha(image, source_mode, dither)?)),
            |result| result.as_ref().map_or(0, eager_convert_image_bytes),
        );
    }
    Ok(Some(convert_to_palette_alpha(image, source_mode, dither)?))
}

/// Record scalar-mode conversions that execute directly on the host so a
/// strict SIMD/GPU parity run cannot count an eager CPU path as backend work.
/// The normal conversion path stays telemetry-free, and Parallel CPU keeps its
/// own execution profile.
pub(super) fn with_eager_cpu_convert_receipt<T>(
    execute: impl FnOnce() -> Result<T, PilError>,
    output_bytes: impl FnOnce(&T) -> usize,
) -> Result<T, PilError> {
    if cfg!(feature = "parallel") || !crate::compute::Backend::pipeline_telemetry_enabled() {
        return execute();
    }
    let requested_backend = crate::compute::preferred_active_backend()
        .ok()
        .flatten()
        .map(|(backend, _)| backend);
    let telemetry =
        crate::compute::begin_eager_cpu_operation_telemetry("Convert", requested_backend);
    match execute() {
        Ok(result) => {
            crate::compute::finish_eager_cpu_operation_telemetry(telemetry, output_bytes(&result));
            Ok(result)
        }
        Err(error) => {
            crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
            Err(error)
        }
    }
}

pub(super) fn eager_convert_image_bytes(image: &Image) -> usize {
    match image {
        Image::Loaded(data) => data.image.as_bytes().len(),
        Image::Paletted(data) => data.indices.as_bytes().len(),
        Image::Bytes { .. } | Image::Pipeline { .. } => 0,
    }
}

pub(super) fn try_convert_luma16_to_cmyk(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
) -> Result<Option<Image>, PilError> {
    if target_mode != "CMYK" || !matches!(source_mode, "I;16" | "I;16L" | "I;16B" | "I;16N") {
        return Ok(None);
    }
    let gray = image.materialize()?.to_luma8();
    let (width, height) = gray.dimensions();
    let mut result = crate::raster::RgbaImage::new(width, height);
    for (output, input) in result.pixels_mut().zip(gray.pixels()) {
        *output = crate::raster::Rgba([0, 0, 0, 255u8.saturating_sub(input[0])]);
    }
    Ok(Some(Image::from_dynamic(
        DynamicImage::ImageRgba8(result),
        explicit_mode_for(target_mode),
    )))
}

pub(super) fn try_convert_nonstandard_source(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
) -> Result<Option<Image>, PilError> {
    if !needs_nonstandard_expansion(source_mode, target_mode) {
        return Ok(None);
    }
    let palette = image.palette();
    let shared = image.materialized_shared_for_ops()?;
    let source = shared.as_ref();
    if source_mode == "PA"
        && target_mode == "RGBA"
        && let DynamicImage::ImageLumaA8(indices_alpha) = source
    {
        let rgba = crate::image::expand_palette_alpha(
            indices_alpha,
            palette.as_deref().unwrap_or_default(),
        );
        return Ok(Some(Image::from_dynamic(
            rgba,
            explicit_mode_for(target_mode),
        )));
    }
    let converted = nonstandard_rgb_source(source, source_mode, palette.as_deref());
    let result = if matches!(target_mode, "L" | "LA") {
        convert_nonstandard_luma(image, source_mode, target_mode, source, &converted)?
    } else if let Some(result) = convert_nonstandard_rgba_palette_alpha(
        image,
        source_mode,
        target_mode,
        source,
        palette.as_deref().unwrap_or_default(),
    )? {
        return Ok(Some(result));
    } else {
        convert_nonstandard_color(target_mode, converted)
    };
    if target_mode == "1" {
        // Mode 1 re-derives truncated luma in the shared threshold/dither path.
        return Ok(None);
    }
    Ok(Some(Image::from_dynamic(
        result,
        explicit_mode_for(target_mode),
    )))
}

fn needs_nonstandard_expansion(source_mode: &str, target_mode: &str) -> bool {
    let target_is_standard = !is_nonstandard_mode(target_mode);
    is_nonstandard_mode(source_mode)
        && (target_is_standard
            || target_mode == "CMYK"
            || (target_mode == "PA" && source_mode == "P"))
        && !(source_mode == "PA" && target_mode == "RGB")
}

fn nonstandard_rgb_source(
    source: &DynamicImage,
    source_mode: &str,
    palette: Option<&[u8]>,
) -> DynamicImage {
    if source_mode == "PA" {
        crate::image::expand_palette_alpha(&source.to_luma_alpha8(), palette.unwrap_or_default())
    } else {
        color::convert_from_nonstandard(source_mode, source, palette)
            .unwrap_or_else(|| source.to_rgb8().into())
    }
}

fn convert_nonstandard_luma(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    source: &DynamicImage,
    converted: &DynamicImage,
) -> Result<DynamicImage, PilError> {
    if target_mode == "L" && source_mode == "YCbCr" {
        return Ok(DynamicImage::ImageLuma8(super::mode::ycbcr_luma8(source)));
    }
    if target_mode == "LA" && source_mode == "YCbCr" {
        let gray = super::mode::ycbcr_luma8(source);
        let (width, height) = gray.dimensions();
        let mut output = crate::raster::GrayAlphaImage::new(width, height);
        for (out_pixel, gray_pixel) in output.pixels_mut().zip(gray.pixels()) {
            out_pixel[0] = gray_pixel[0];
            out_pixel[1] = 255;
        }
        return Ok(DynamicImage::ImageLumaA8(output));
    }
    if target_mode == "L" {
        return Ok(DynamicImage::ImageLuma8(color::pil_grayscale(converted)?));
    }
    let mut la = if source_mode == "PA" {
        let gray = color::pil_grayscale(converted)?;
        let rgba = converted.to_rgba8();
        let (width, height) = gray.dimensions();
        let mut output = crate::raster::GrayAlphaImage::new(width, height);
        for ((out_pixel, gray_pixel), rgba_pixel) in
            output.pixels_mut().zip(gray.pixels()).zip(rgba.pixels())
        {
            out_pixel[0] = gray_pixel[0];
            out_pixel[1] = rgba_pixel[3];
        }
        output
    } else {
        color::pil_grayscale_alpha(converted)?
    };
    if source_mode == "P" {
        let indices = source.to_luma8();
        if let Some(alpha) = palette_alpha_for_convert(image) {
            for (out_pixel, input) in la.pixels_mut().zip(indices.pixels()) {
                out_pixel[1] = alpha.get(usize::from(input[0])).copied().unwrap_or(255);
            }
        }
    }
    Ok(DynamicImage::ImageLumaA8(la))
}

fn convert_nonstandard_rgba_palette_alpha(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    source: &DynamicImage,
    palette: &[u8],
) -> Result<Option<Image>, PilError> {
    if target_mode != "RGBA" || source_mode != "P" {
        return Ok(None);
    }
    let alpha = image.palette_alpha().unwrap_or_default();
    if alpha.is_empty() {
        return Ok(None);
    }
    let indices = source.to_luma8();
    let expanded = crate::image::expand_palette(&indices, palette, &alpha);
    Ok(Some(Image::from_dynamic(
        expanded,
        explicit_mode_for(target_mode),
    )))
}

fn convert_nonstandard_color(target_mode: &str, converted: DynamicImage) -> DynamicImage {
    if target_mode == "CMYK" {
        DynamicImage::ImageRgba8(crate::color::rgb_to_cmyk_inverse(&converted.to_rgb8()))
    } else if target_mode == "RGB" {
        DynamicImage::ImageRgb8(converted.into_rgb8())
    } else if target_mode == "RGBA" {
        // PA expansion already owns RGBA bytes; other sources use normal widening.
        DynamicImage::ImageRgba8(converted.into_rgba8())
    } else {
        converted
    }
}

pub(super) fn try_convert_cmyk_to_scalar(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
) -> Result<Option<Image>, PilError> {
    if source_mode != "CMYK" || !matches!(target_mode, "I" | "F") {
        return Ok(None);
    }
    let source = image.materialize()?;
    let rgb = color::cmyk_to_rgb(&source).to_rgb8();
    let (width, height) = rgb.dimensions();
    let mut output = crate::raster::RgbaImage::new(width, height);
    for (out_pixel, pixel) in output.pixels_mut().zip(rgb.pixels()) {
        let red = i32::from(pixel[0]);
        let green = i32::from(pixel[1]);
        let blue = i32::from(pixel[2]);
        *out_pixel = if target_mode == "I" {
            let value = (19595 * red + 38470 * green + 7471 * blue + 32768) >> 16;
            crate::raster::Rgba(value.to_le_bytes())
        } else {
            let value = (red * 299 + green * 587 + blue * 114) as f32 / 1000.0;
            crate::raster::Rgba(value.to_le_bytes())
        };
    }
    Ok(Some(Image::from_dynamic(
        DynamicImage::ImageRgba8(output),
        explicit_mode_for(target_mode),
    )))
}
