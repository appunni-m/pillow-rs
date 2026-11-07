//! CPU conversion to Pillow's expanded-byte bilevel representation.

use crate::checked_dims::CheckedDims;
use crate::compute::{
    Architecture,
    registry::support::{OperationInputs, PaletteInput, StrategySelection},
};
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, GrayImage, ImageBuffer, Luma, Mode};

type Luma16Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

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
    if let Some(samples) = luma16_samples(source, source_mode) {
        return convert_luma16(samples, no_dither(dither));
    }
    if source_mode == Mode::One {
        if let DynamicImage::One(source) = source {
            return Ok(DynamicImage::One(source.clone()));
        }
    }
    if source_mode == Mode::ImageCmyk8 && matches!(dither, Some(&DitherMethod::None)) {
        return crate::color::cmyk_to_binary_truncate(source).map(DynamicImage::One);
    }

    let no_dither = no_dither(dither);
    match (source_mode, no_dither) {
        (Mode::ImageRgbPremultiplied8, true) => {
            let DynamicImage::ImageRgbPremultiplied8(image) = source else {
                unreachable!("RGBa binary strategy must retain its native carrier")
            };
            let mut output = GrayImage::new(image.width(), image.height());
            for (destination, pixel) in output.pixels_mut().zip(image.pixels()) {
                let [red, green, blue] = straight_rgb(pixel);
                let luma = 299 * i32::from(red) + 587 * i32::from(green) + 114 * i32::from(blue);
                destination[0] = if luma >= 128_000 { 255 } else { 0 };
            }
            Ok(DynamicImage::One(output))
        }
        (Mode::ImageRgbPremultiplied8, false) => {
            let DynamicImage::ImageRgbPremultiplied8(image) = source else {
                unreachable!("RGBa binary strategy must retain its native carrier")
            };
            let mut output = GrayImage::new(image.width(), image.height());
            diffuse_rgb_samples(image.pixels().map(straight_rgb), &mut output)?;
            Ok(DynamicImage::One(output))
        }
        (_, true) => {
            let gray = binary_luma(source, source_mode, inputs.palette.rgb)?;
            let (width, height) = gray.dimensions();
            let mut output = GrayImage::new(width, height);
            threshold_binary(&gray, &mut output);
            Ok(DynamicImage::One(output))
        }
        (Mode::ImageRgb8, false) => {
            // RGB already contains the device-space samples expected by the
            // generic color error-diffusion kernel.
            let (width, height) = (source.width(), source.height());
            let mut output = GrayImage::new(width, height);
            let rgb_samples = match source {
                DynamicImage::ImageRgb8(image) => image.as_raw(),
                _ => unreachable!("RGB binary strategy must retain its native RGB carrier"),
            };
            diffuse_rgb_rows(rgb_samples, &mut output)?;
            Ok(DynamicImage::One(output))
        }
        (_, false) => {
            let gray = binary_luma(source, source_mode, inputs.palette.rgb)?;
            let (width, height) = gray.dimensions();
            let mut output = GrayImage::new(width, height);
            diffuse_gray_rows(&gray, &mut output)?;
            Ok(DynamicImage::One(output))
        }
    }
}

/// Threshold palette colors directly for P/PA-to-One, without dithering or an RGB raster.
pub(crate) fn palette_indices_to_binary(
    width: u32,
    height: u32,
    indices: impl ExactSizeIterator<Item = u8>,
    palette: PaletteInput<'_>,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError(
            "Convert selected an undeclared binary architecture".into(),
        ));
    }
    let dims = CheckedDims::new_allow_empty(width, height, 1)?;
    if indices.len() != dims.total_pixels() {
        return Err(PilError::InternalError(
            "palette binary source did not match its output extent".into(),
        ));
    }
    let mut output = GrayImage::new(width, height);
    for (destination, index) in output.pixels_mut().zip(indices) {
        let [red, green, blue] =
            crate::compute::pool_cpu::ops::color::convert::helpers::palette::color(index, palette);
        let luminance = 299 * i32::from(red) + 587 * i32::from(green) + 114 * i32::from(blue);
        destination[0] = if luminance >= 128_000 { u8::MAX } else { 0 };
    }
    Ok(DynamicImage::One(output))
}

fn no_dither(dither: Option<&DitherMethod>) -> bool {
    dither.is_some_and(|method| matches!(method, DitherMethod::None))
}

fn luma16_samples(source: &DynamicImage, source_mode: Mode) -> Option<&Luma16Samples> {
    match (source_mode, source) {
        (Mode::ImageI16, DynamicImage::ImageI16(image)) => Some(image),
        (Mode::ImageI16L, DynamicImage::ImageI16L(image)) => Some(image),
        (Mode::ImageI16B, DynamicImage::ImageI16B(image)) => Some(image),
        (Mode::ImageI16N, DynamicImage::ImageI16N(image)) => Some(image),
        _ => None,
    }
}

fn convert_luma16(source: &Luma16Samples, no_dither: bool) -> Result<DynamicImage, PilError> {
    let mut output = GrayImage::new(source.width(), source.height());
    if no_dither {
        for (destination, sample) in output.pixels_mut().zip(source.as_raw()) {
            destination[0] = if clip_luma16(*sample) >= 128 { 255 } else { 0 };
        }
    } else {
        diffuse_gray_samples(
            source.as_raw().iter().copied().map(clip_luma16),
            &mut output,
        )?;
    }
    Ok(DynamicImage::One(output))
}

#[inline]
fn clip_luma16(sample: u16) -> u8 {
    sample.min(u16::from(u8::MAX)) as u8
}

fn binary_luma(
    source: &DynamicImage,
    source_mode: Mode,
    palette: Option<&[u8]>,
) -> Result<GrayImage, PilError> {
    match source_mode {
        Mode::ImageCmyk8 => crate::color::cmyk_to_grayscale_truncate(source),
        Mode::ImageHsv8 | Mode::ImageYCbCr8 | Mode::ImageI32 | Mode::ImageF32 => {
            let rgb = crate::color::convert_from_nonstandard(source_mode, source, palette)
                .ok_or_else(|| {
                    PilError::InternalError(
                        "Convert binary source mode omitted its RGB conversion strategy".into(),
                    )
                })?;
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

#[inline]
fn straight_rgb(pixel: &crate::raster::Rgba<u8>) -> [u8; 3] {
    [
        crate::compute::pool_cpu::ops::color::convert::helpers::pixel::unpremultiply_byte(
            pixel[0], pixel[3],
        ),
        crate::compute::pool_cpu::ops::color::convert::helpers::pixel::unpremultiply_byte(
            pixel[1], pixel[3],
        ),
        crate::compute::pool_cpu::ops::color::convert::helpers::pixel::unpremultiply_byte(
            pixel[2], pixel[3],
        ),
    ]
}

pub(crate) fn diffuse_rgb_rows(rgb: &[u8], output: &mut GrayImage) -> Result<(), PilError> {
    if rgb.len() % 3 != 0 {
        return Err(PilError::InternalError(
            "binary RGB source did not contain complete pixels".into(),
        ));
    }
    diffuse_rgb_samples(
        rgb.chunks_exact(3)
            .map(|pixel| [pixel[0], pixel[1], pixel[2]]),
        output,
    )
}

pub(crate) fn diffuse_rgb_samples(
    samples: impl ExactSizeIterator<Item = [u8; 3]>,
    output: &mut GrayImage,
) -> Result<(), PilError> {
    let (width, height) = output.dimensions();
    let width_usize = width as usize;
    let pixel_count = CheckedDims::new_allow_empty(width, height, 1)?.total_pixels();
    if samples.len() != pixel_count {
        return Err(PilError::InternalError(
            "binary RGB source did not match its output extent".into(),
        ));
    }
    let mut samples = samples;
    let mut errors = vec![0i32; (width + 1) as usize];
    for y in 0..height as usize {
        let mut carry = 0i32;
        let mut delayed_five = 0i32;
        let mut delayed_one = 0i32;
        for x in 0..width_usize {
            let [red, green, blue] = samples.next().ok_or_else(|| {
                PilError::InternalError("binary RGB source ended before its output".into())
            })?;
            let r = i32::from(red);
            let g = i32::from(green);
            let b = i32::from(blue);
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
    if samples.next().is_some() {
        return Err(PilError::InternalError(
            "binary RGB source exceeded its output extent".into(),
        ));
    }
    Ok(())
}

fn diffuse_gray_rows(gray: &GrayImage, output: &mut GrayImage) -> Result<(), PilError> {
    diffuse_gray_samples(gray.as_raw().iter().copied(), output)
}

pub(crate) fn diffuse_gray_samples(
    samples: impl ExactSizeIterator<Item = u8>,
    output: &mut GrayImage,
) -> Result<(), PilError> {
    let (width, height) = output.dimensions();
    let width_usize = width as usize;
    let expected_pixels = CheckedDims::new_allow_empty(width, height, 1)?.total_pixels();
    if samples.len() != expected_pixels {
        return Err(PilError::InternalError(
            "binary grayscale source did not match its output extent".into(),
        ));
    }
    let mut samples = samples;
    let mut errors = vec![0i32; (width + 1) as usize];
    for y in 0..height as usize {
        let mut carry = 0i32;
        let mut delayed_five = 0i32;
        let mut delayed_one = 0i32;
        for x in 0..width_usize {
            let sample = samples.next().ok_or_else(|| {
                PilError::InternalError("binary grayscale source ended before its output".into())
            })?;
            let value = (i32::from(sample) + (carry + errors[x + 1]) / 16).clamp(0, 255);
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
    if samples.next().is_some() {
        return Err(PilError::InternalError(
            "binary grayscale source exceeded its output extent".into(),
        ));
    }
    Ok(())
}
