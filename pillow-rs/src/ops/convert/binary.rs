use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, GrayImage};

use super::mode::is_nonstandard_mode;

pub(super) fn convert_to_binary(
    image: &Image,
    source_mode: &str,
    dither: Option<DitherMethod>,
) -> Result<Image, PilError> {
    if source_mode == "CMYK" && matches!(dither, Some(DitherMethod::None)) {
        let source = image.materialized_shared()?;
        let output = crate::color::cmyk_to_binary_truncate(source.as_ref())?;
        return Ok(Image::from_dynamic(
            DynamicImage::ImageLuma8(output),
            Some("1".to_owned()),
        ));
    }
    let source = image.materialize()?;
    let gray = binary_luma(image, source_mode, &source)?;
    let (width, height) = gray.dimensions();
    let mut output = GrayImage::new(width, height);
    if matches!(dither, Some(DitherMethod::None)) || matches!(source_mode, "P" | "PA") {
        threshold_binary(&gray, &mut output);
    } else {
        diffuse_binary_error(&source, source_mode, &gray, &mut output);
    }
    Ok(Image::from_dynamic(
        DynamicImage::ImageLuma8(output),
        Some("1".to_owned()),
    ))
}

fn binary_luma(
    image: &Image,
    source_mode: &str,
    source: &DynamicImage,
) -> Result<GrayImage, PilError> {
    if source_mode == "CMYK" {
        crate::color::cmyk_to_grayscale_truncate(source)
    } else if is_nonstandard_mode(source_mode) {
        let palette = image.palette();
        let rgb = crate::color::convert_from_nonstandard(source_mode, source, palette.as_deref())
            .unwrap_or_else(|| source.to_rgb8().into());
        crate::color::pil_grayscale_truncate(&rgb)
    } else {
        crate::color::pil_grayscale_truncate(source)
    }
}

fn threshold_binary(gray: &GrayImage, output: &mut GrayImage) {
    for (out_pixel, gray_pixel) in output.pixels_mut().zip(gray.pixels()) {
        out_pixel[0] = if gray_pixel[0] >= 128 { 255 } else { 0 };
    }
}

fn diffuse_binary_error(
    source: &DynamicImage,
    source_mode: &str,
    gray: &GrayImage,
    output: &mut GrayImage,
) {
    // RGB uses Pillow's 299/587/114 luma here; precomputed BT.601 differs for
    // a few values. Non-standard RGB carriers use their already-converted luma.
    let is_rgb = matches!(source.color(), crate::raster::ColorType::Rgb8)
        && !is_nonstandard_mode(source_mode);
    if is_rgb {
        let rgb = source.to_rgb8();
        diffuse_rgb_rows(&rgb.into_raw(), output);
    } else {
        diffuse_gray_rows(gray, output);
    }
}

fn diffuse_rgb_rows(rgb: &[u8], output: &mut GrayImage) {
    let (width, height) = output.dimensions();
    let width_usize = width as usize;
    let mut errors = vec![0i32; (width + 1) as usize];
    for y in 0..height as usize {
        let mut carry = 0i32;
        let mut delayed_five = 0i32;
        let mut delayed_one = 0i32;
        let row_base = y * width_usize * 3;
        for x in 0..width_usize {
            let r = i32::from(rgb[row_base + x * 3]);
            let g = i32::from(rgb[row_base + x * 3 + 1]);
            let b = i32::from(rgb[row_base + x * 3 + 2]);
            let luminance = (299 * r + 587 * g + 114 * b) / 1000;
            let value = (luminance + (carry + errors[x + 1]) / 16).clamp(0, 255);
            let quantized = if value > 128 { 255 } else { 0 };
            output.get_pixel_mut(x as u32, y as u32)[0] = quantized as u8;
            carry = value - quantized;
            let original_error = carry;
            let doubled = carry + carry;
            carry += doubled;
            errors[x] = carry + delayed_five;
            carry += doubled;
            delayed_five = carry + delayed_one;
            delayed_one = original_error;
            carry += doubled;
        }
        errors[width_usize] = delayed_five;
    }
}

fn diffuse_gray_rows(gray: &GrayImage, output: &mut GrayImage) {
    let (width, height) = output.dimensions();
    let width_usize = width as usize;
    let source: Vec<i32> = gray.pixels().map(|pixel| i32::from(pixel[0])).collect();
    let mut errors = vec![0i32; (width + 1) as usize];
    for y in 0..height as usize {
        let mut carry = 0i32;
        let mut delayed_five = 0i32;
        let mut delayed_one = 0i32;
        for x in 0..width_usize {
            let index = y * width_usize + x;
            let value = (source[index] + (carry + errors[x + 1]) / 16).clamp(0, 255);
            let quantized = if value > 128 { 255 } else { 0 };
            output.get_pixel_mut(x as u32, y as u32)[0] = quantized as u8;
            carry = value - quantized;
            let original_error = carry;
            let doubled = carry + carry;
            carry += doubled;
            errors[x] = carry + delayed_five;
            carry += doubled;
            delayed_five = carry + delayed_one;
            delayed_one = original_error;
            carry += doubled;
        }
        errors[width_usize] = delayed_five;
    }
}
