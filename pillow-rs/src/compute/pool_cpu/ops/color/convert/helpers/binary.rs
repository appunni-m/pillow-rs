//! CPU conversion to Pillow's expanded-byte bilevel representation.

use crate::compute::{Architecture, registry::support::StrategySelection};
use crate::compute::registry::support::OperationInputs;
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, GrayImage, Mode};

/// Declares the target-`1` strategy in one exact source-mode owner.
macro_rules! declare_mode_strategy {
    ($mode:expr) => {
        pub(crate) fn execute_binary(
            inputs: $crate::compute::registry::support::OperationInputs<'_>,
            dither: Option<&$crate::pipeline::DitherMethod>,
            selection: $crate::compute::registry::support::StrategySelection,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::color::convert::helpers::binary::convert_for_mode(
                inputs, $mode, dither, selection,
            )
        }
    };
}

pub(crate) use declare_mode_strategy;

/// Executes target `1` after the exact source-mode owner has been selected.
pub(crate) fn convert_for_mode(
    inputs: OperationInputs<'_>,
    source_mode: Mode,
    dither: Option<&DitherMethod>,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError(
            "Convert selected an undeclared binary architecture".into(),
        ));
    }

    if inputs.mode() != source_mode {
        return Err(PilError::InternalError(
            "Convert binary strategy received a different source mode".into(),
        ));
    }
    let source = inputs.image;
    if source_mode == Mode::One {
        if let DynamicImage::One(source) = source {
            return Ok(DynamicImage::One(source.clone()));
        }
    }
    if source_mode == Mode::ImageCmyk8 && matches!(dither, Some(&DitherMethod::None)) {
        return crate::color::cmyk_to_binary_truncate(source).map(DynamicImage::One);
    }

    let no_dither = dither.is_some_and(|method| matches!(method, DitherMethod::None));
    match (source_mode, no_dither) {
        (Mode::ImagePalette8 | Mode::ImagePaletteAlpha8, _) | (_, true) => {
            let gray = binary_luma(source, source_mode, inputs.palette.rgb)?;
            let (width, height) = gray.dimensions();
            let mut output = GrayImage::new(width, height);
            threshold_binary(&gray, &mut output);
            Ok(DynamicImage::One(output))
        }
        (Mode::ImageRgb8 | Mode::ImageLab8, false) => {
            // The archived path uses RGB error diffusion for LAB's three-byte
            // carrier too; its nonstandard-mode predicate intentionally
            // excludes LAB.
            let (width, height) = (source.width(), source.height());
            let mut output = GrayImage::new(width, height);
            diffuse_rgb_rows(&source.to_rgb8().into_raw(), &mut output);
            Ok(DynamicImage::One(output))
        }
        (_, false) => {
            let gray = binary_luma(source, source_mode, inputs.palette.rgb)?;
            let (width, height) = gray.dimensions();
            let mut output = GrayImage::new(width, height);
            diffuse_gray_rows(&gray, &mut output);
            Ok(DynamicImage::One(output))
        }
    }
}

fn binary_luma(
    source: &DynamicImage,
    source_mode: Mode,
    palette: Option<&[u8]>,
) -> Result<GrayImage, PilError> {
    match source_mode {
        Mode::ImageCmyk8 => crate::color::cmyk_to_grayscale_truncate(source),
        Mode::ImageHsv8
        | Mode::ImageYCbCr8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8 => {
            let rgb = crate::color::convert_from_nonstandard(source_mode, source, palette)
                .unwrap_or_else(|| source.to_rgb8().into());
            crate::color::pil_grayscale_truncate(&rgb)
        }
        _ => crate::color::pil_grayscale_truncate(source),
    }
}

fn threshold_binary(gray: &GrayImage, output: &mut GrayImage) {
    for (out_pixel, gray_pixel) in output.pixels_mut().zip(gray.pixels()) {
        out_pixel[0] = if gray_pixel[0] >= 128 { 255 } else { 0 };
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
