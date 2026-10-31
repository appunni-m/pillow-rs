use crate::color;
use crate::error::PilError;
use crate::image::Image;
use crate::raster::{DynamicImage, Mode};

use super::mode::is_nonstandard_mode;
use super::palette::palette_alpha_for_convert;

pub(super) fn try_convert_between_nonstandard_modes(
    image: &Image,
    source_mode: Mode,
    target_mode: Mode,
    matrix: Option<Vec<f64>>,
    dither: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    if !is_nonstandard_mode(source_mode)
        || !matches!(
            target_mode,
            Mode::ImageHsv8
                | Mode::ImageYCbCr8
                | Mode::ImagePalette8
                | Mode::ImageI32
                | Mode::ImageF32
        )
        || target_mode == source_mode
        || (source_mode == Mode::ImageCmyk8
            && matches!(target_mode, Mode::ImageI32 | Mode::ImageF32))
    {
        return Ok(None);
    }
    let palette = image.palette();
    let source = image.materialize()?;
    let converted = if source_mode == Mode::ImagePaletteAlpha8 {
        crate::image::expand_palette_alpha(
            &source.to_luma_alpha8(),
            palette.as_deref().unwrap_or_default(),
        )
    } else {
        color::convert_from_nonstandard(source_mode, &source, palette.as_deref())
            .unwrap_or_else(|| source.to_rgb8().into())
    };
    let rgb_source = Image::from_dynamic(converted);
    Ok(Some(rgb_source.convert(
        target_mode,
        matrix,
        dither,
        None,
        colors,
    )?))
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

pub(super) fn try_convert_nonstandard_source(
    image: &Image,
    source_mode: Mode,
    target_mode: Mode,
) -> Result<Option<Image>, PilError> {
    // Binary conversion consumes the original source mode below. Avoid
    // materializing a temporary RGB image that would be discarded.
    if target_mode == Mode::One {
        return Ok(None);
    }
    // TODO(pillow-rs#3ptb): reconcile the remaining eager palette/YCbCr/CMYK/
    // HSV pairs with the archived matrix; add support only where a native
    // strategy preserves their source-specific luma, alpha, and palette rules.
    if !needs_nonstandard_expansion(source_mode, target_mode) {
        return Ok(None);
    }
    let palette = image.palette();
    let shared = image.materialized_shared_for_ops()?;
    let source = shared.as_ref();
    if source_mode == Mode::ImagePaletteAlpha8
        && target_mode == Mode::ImageRgba8
        && let DynamicImage::ImageLumaA8(indices_alpha) = source
    {
        let rgba = crate::image::expand_palette_alpha(
            indices_alpha,
            palette.as_deref().unwrap_or_default(),
        );
        return Ok(Some(Image::from_dynamic_with_mode(
            rgba,
            Some(target_mode),
        )?));
    }
    let converted = nonstandard_rgb_source(source, source_mode, palette.as_deref());
    let result = if matches!(target_mode, Mode::ImageLuma8 | Mode::ImageLumaA8) {
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
    Ok(Some(Image::from_dynamic_with_mode(
        result,
        Some(target_mode),
    )?))
}

fn needs_nonstandard_expansion(source_mode: Mode, target_mode: Mode) -> bool {
    let target_is_standard = !is_nonstandard_mode(target_mode);
    is_nonstandard_mode(source_mode)
        && (target_is_standard
            || target_mode == Mode::ImageCmyk8
            || (target_mode == Mode::ImagePaletteAlpha8 && source_mode == Mode::ImagePalette8))
        && !(source_mode == Mode::ImagePaletteAlpha8 && target_mode == Mode::ImageRgb8)
}

fn nonstandard_rgb_source(
    source: &DynamicImage,
    source_mode: Mode,
    palette: Option<&[u8]>,
) -> DynamicImage {
    if source_mode == Mode::ImagePaletteAlpha8 {
        crate::image::expand_palette_alpha(&source.to_luma_alpha8(), palette.unwrap_or_default())
    } else {
        color::convert_from_nonstandard(source_mode, source, palette)
            .unwrap_or_else(|| source.to_rgb8().into())
    }
}

fn convert_nonstandard_luma(
    image: &Image,
    source_mode: Mode,
    target_mode: Mode,
    source: &DynamicImage,
    converted: &DynamicImage,
) -> Result<DynamicImage, PilError> {
    if target_mode == Mode::ImageLuma8 && source_mode == Mode::ImageYCbCr8 {
        return Ok(DynamicImage::ImageLuma8(super::mode::ycbcr_luma8(source)));
    }
    if target_mode == Mode::ImageLumaA8 && source_mode == Mode::ImageYCbCr8 {
        let gray = super::mode::ycbcr_luma8(source);
        let (width, height) = gray.dimensions();
        let mut output = crate::raster::GrayAlphaImage::new(width, height);
        for (out_pixel, gray_pixel) in output.pixels_mut().zip(gray.pixels()) {
            out_pixel[0] = gray_pixel[0];
            out_pixel[1] = 255;
        }
        return Ok(DynamicImage::ImageLumaA8(output));
    }
    if target_mode == Mode::ImageLuma8 {
        return Ok(DynamicImage::ImageLuma8(color::pil_grayscale(converted)?));
    }
    let mut la = if source_mode == Mode::ImagePaletteAlpha8 {
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
    if source_mode == Mode::ImagePalette8 {
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
    source_mode: Mode,
    target_mode: Mode,
    source: &DynamicImage,
    palette: &[u8],
) -> Result<Option<Image>, PilError> {
    if target_mode != Mode::ImageRgba8 || source_mode != Mode::ImagePalette8 {
        return Ok(None);
    }
    let alpha = image.palette_alpha().unwrap_or_default();
    if alpha.is_empty() {
        return Ok(None);
    }
    let indices = source.to_luma8();
    let expanded = crate::image::expand_palette(&indices, palette, &alpha);
    Ok(Some(Image::from_dynamic_with_mode(
        expanded,
        Some(target_mode),
    )?))
}

fn convert_nonstandard_color(target_mode: Mode, converted: DynamicImage) -> DynamicImage {
    if target_mode == Mode::ImageCmyk8 {
        DynamicImage::ImageRgba8(crate::color::rgb_to_cmyk_inverse(&converted.to_rgb8()))
    } else if target_mode == Mode::ImageRgb8 {
        DynamicImage::ImageRgb8(converted.into_rgb8())
    } else if target_mode == Mode::ImageRgba8 {
        // PA expansion already owns RGBA bytes; other sources use normal widening.
        DynamicImage::ImageRgba8(converted.into_rgba8())
    } else {
        converted
    }
}

