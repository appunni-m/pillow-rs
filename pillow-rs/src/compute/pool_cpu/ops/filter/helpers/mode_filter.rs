//! Native byte-channel implementation for the direct Pillow ModeFilter.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, Mode, RgbImage, RgbaImage};

pub(crate) fn execute_mode_filter(
    image: &DynamicImage,
    mode: Mode,
    size: u32,
) -> Result<DynamicImage, PilError> {
    if image.mode() != mode {
        return Err(PilError::InternalError(
            "ModeFilter strategy received a mismatched raster mode".into(),
        ));
    }
    let channels = native_byte_channels(mode).ok_or_else(wrong_mode)?;
    let dimensions = CheckedDims::new_allow_empty(image.width(), image.height(), channels)?;
    if image.as_bytes().len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "ModeFilter source bytes do not match its exact mode layout".into(),
        ));
    }
    let output = mode_filter_native_bytes(
        image.as_bytes(),
        dimensions.width as usize,
        dimensions.height as usize,
        usize::from(channels),
        size,
    );
    dynamic_image_from_raw(mode, dimensions.width, dimensions.height, output)
}

fn mode_filter_native_bytes(
    raw: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    size: u32,
) -> Vec<u8> {
    let mut output = vec![0u8; width * height * channels];
    let width = width as i32;
    let height = height as i32;
    let half = (size / 2) as i32;

    for y in 0..height {
        for x in 0..width {
            let mut histograms = vec![[0u32; 256]; channels];
            accumulate_neighborhood(raw, &mut histograms, (width, height, channels), (x, y), half);
            write_mode_pixel(raw, &mut output, &histograms, (width, channels), (x, y));
        }
    }
    output
}

fn accumulate_neighborhood(
    raw: &[u8],
    histograms: &mut [[u32; 256]],
    shape: (i32, i32, usize),
    center: (i32, i32),
    half: i32,
) {
    let (width, height, channels) = shape;
    let (x, y) = center;
    for dy in -half..=half {
        let source_y = y + dy;
        if source_y < 0 || source_y >= height {
            continue;
        }
        for dx in -half..=half {
            let source_x = x + dx;
            if source_x < 0 || source_x >= width {
                continue;
            }
            let base = ((source_y * width + source_x) as usize) * channels;
            for (channel, histogram) in histograms.iter_mut().enumerate() {
                histogram[raw[base + channel] as usize] += 1;
            }
        }
    }
}

fn write_mode_pixel(
    raw: &[u8],
    output: &mut [u8],
    histograms: &[[u32; 256]],
    shape: (i32, usize),
    center: (i32, i32),
) {
    let (width, channels) = shape;
    let (x, y) = center;
    let pixel = ((y * width + x) as usize) * channels;
    for (channel, histogram) in histograms.iter().enumerate() {
        let (mode, max_count) = find_mode_with_count(histogram);
        let original = raw[pixel + channel];
        output[pixel + channel] = if max_count > 2 { mode } else { original };
    }
}

fn find_mode_with_count(histogram: &[u32; 256]) -> (u8, u32) {
    let mut mode = 0u8;
    let mut max_count = histogram[0];
    for (value, &count) in histogram.iter().enumerate().skip(1) {
        if count > max_count {
            max_count = count;
            mode = value as u8;
        }
    }
    (mode, max_count)
}

const fn native_byte_channels(mode: Mode) -> Option<u8> {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => Some(1),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => Some(2),
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => Some(3),
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8 => Some(4),
        _ => None,
    }
}

fn dynamic_image_from_raw(
    mode: Mode,
    width: u32,
    height: u32,
    raw: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    let invalid = || {
        PilError::InternalError("ModeFilter output bytes do not match its exact mode".into())
    };
    match mode {
        Mode::One => GrayImage::from_raw(width, height, raw)
            .map(DynamicImage::One)
            .ok_or_else(invalid),
        Mode::ImageLuma8 => GrayImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageLuma8)
            .ok_or_else(invalid),
        Mode::ImagePalette8 => GrayImage::from_raw(width, height, raw)
            .map(DynamicImage::ImagePalette8)
            .ok_or_else(invalid),
        Mode::ImageLumaA8 => GrayAlphaImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(invalid),
        Mode::ImageLaPremultiplied8 => GrayAlphaImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageLaPremultiplied8)
            .ok_or_else(invalid),
        Mode::ImagePaletteAlpha8 => GrayAlphaImage::from_raw(width, height, raw)
            .map(DynamicImage::ImagePaletteAlpha8)
            .ok_or_else(invalid),
        Mode::ImageRgb8 => RgbImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(invalid),
        Mode::ImageYCbCr8 => RgbImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageYCbCr8)
            .ok_or_else(invalid),
        Mode::ImageLab8 => RgbImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageLab8)
            .ok_or_else(invalid),
        Mode::ImageHsv8 => RgbImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageHsv8)
            .ok_or_else(invalid),
        Mode::ImageRgba8 => RgbaImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(invalid),
        Mode::ImageRgbPremultiplied8 => RgbaImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageRgbPremultiplied8)
            .ok_or_else(invalid),
        Mode::ImageRgbx8 => RgbaImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageRgbx8)
            .ok_or_else(invalid),
        Mode::ImageCmyk8 => RgbaImage::from_raw(width, height, raw)
            .map(DynamicImage::ImageCmyk8)
            .ok_or_else(invalid),
        _ => Err(wrong_mode()),
    }
}

fn wrong_mode() -> PilError {
    PilError::ValueError("image has wrong mode".into())
}
