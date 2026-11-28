//! Shared matrix arithmetic over mode-native source pixels.

use crate::error::PilError;
use crate::raster::{DynamicImage, FromColor, GrayImage, ImageBuffer, Mode, Pixel, Rgb, RgbImage};

/// Applies the preserved RGB-sample matrix contract without materializing an RGB image.
// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn convert_with_matrix(
    image: &DynamicImage,
    target_mode: Mode,
    matrix: &[f64],
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::One(source) => apply_source(source, target_mode, matrix, |pixel| {
            let value = if pixel.channels()[0] == 0 { 0 } else { u8::MAX };
            Rgb([value; 3])
        }),
        DynamicImage::ImageI16(source)
        | DynamicImage::ImageI16L(source)
        | DynamicImage::ImageI16B(source)
        | DynamicImage::ImageI16N(source) => apply_source(source, target_mode, matrix, |pixel| {
            let value = pixel.channels()[0].min(u16::from(u8::MAX)) as u8;
            Rgb([value; 3])
        }),
        DynamicImage::ImageLuma8(source) | DynamicImage::ImagePalette8(source) => {
            apply_native_source(source, target_mode, matrix)
        }
        DynamicImage::ImageLumaA8(source)
        | DynamicImage::ImageLaPremultiplied8(source)
        | DynamicImage::ImagePaletteAlpha8(source) => {
            apply_native_source(source, target_mode, matrix)
        }
        DynamicImage::ImageRgb8(source)
        | DynamicImage::ImageYCbCr8(source)
        | DynamicImage::ImageLab8(source)
        | DynamicImage::ImageHsv8(source) => apply_native_source(source, target_mode, matrix),
        DynamicImage::ImageRgba8(source)
        | DynamicImage::ImageRgbPremultiplied8(source)
        | DynamicImage::ImageRgbx8(source)
        | DynamicImage::ImageCmyk8(source)
        | DynamicImage::ImageI32(source)
        | DynamicImage::ImageF32(source) => apply_native_source(source, target_mode, matrix),
        DynamicImage::ImageLumaA16(source) => apply_native_source(source, target_mode, matrix),
        DynamicImage::ImageRgb16(source) => apply_native_source(source, target_mode, matrix),
        DynamicImage::ImageRgba16(source) => apply_native_source(source, target_mode, matrix),
        DynamicImage::ImageRgb32F(source) => apply_native_source(source, target_mode, matrix),
        DynamicImage::ImageRgba32F(source) => apply_native_source(source, target_mode, matrix),
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn apply_native_source<P>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
    target_mode: Mode,
    matrix: &[f64],
) -> Result<DynamicImage, PilError>
where
    P: Pixel,
    Rgb<u8>: FromColor<P>,
{
    apply_source(source, target_mode, matrix, |pixel| {
        let mut rgb = Rgb([0; 3]);
        rgb.copy_from_color(pixel);
        rgb
    })
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn apply_source<P>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
    target_mode: Mode,
    matrix: &[f64],
    mut to_rgb: impl FnMut(&P) -> Rgb<u8>,
) -> Result<DynamicImage, PilError>
where
    P: Pixel,
{
    let (width, height) = source.dimensions();
    let pixels = source
        .as_raw()
        .chunks_exact(usize::from(P::CHANNEL_COUNT))
        .map(P::from_slice);
    apply_matrix(pixels, &mut to_rgb, width, height, target_mode, matrix)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn apply_matrix<P>(
    pixels: impl ExactSizeIterator<Item = P>,
    mut to_rgb: impl FnMut(P) -> Rgb<u8>,
    width: u32,
    height: u32,
    target_mode: Mode,
    matrix: &[f64],
) -> Result<DynamicImage, PilError> {
    if !matches!(matrix.len(), 4 | 12) {
        return Err(PilError::TypeError(
            "matrix must be tuple of length 4 or 12".into(),
        ));
    }
    match (matrix.len(), target_mode) {
        (4, Mode::ImageRgb8) => {
            // NO_PILLOW_SUPPORT: Pillow accepts four coefficients for RGB,
            // but its output depends on the remaining unspecified matrix
            // rows. Define this Rust extension deterministically by applying
            // the affine row to red and zeroing the other output channels.
            let pixels: Vec<u8> = pixels
                .flat_map(|pixel| {
                    let pixel = to_rgb(pixel);
                    [apply_matrix_row(pixel, &matrix[..4]), 0, 0]
                })
                .collect();
            Ok(DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, pixels)
                    .ok_or_else(|| PilError::ValueError("matrix conversion failed".into()))?,
            ))
        }
        (4 | 12, Mode::ImageLuma8) => {
            // Pillow 13.0 accepts either matrix length for L and uses the
            // first four coefficients; the trailing RGB rows are ignored.
            let pixels: Vec<u8> = pixels
                .map(|pixel| apply_matrix_row(to_rgb(pixel), &matrix[..4]))
                .collect();
            Ok(DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, pixels)
                    .ok_or_else(|| PilError::ValueError("matrix conversion failed".into()))?,
            ))
        }
        (12, Mode::ImageRgb8) => {
            let pixels: Vec<u8> = pixels
                .flat_map(|pixel| apply_rgb_matrix(to_rgb(pixel), matrix))
                .collect();
            Ok(DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, pixels)
                    .ok_or_else(|| PilError::ValueError("matrix conversion failed".into()))?,
            ))
        }
        (12, Mode::ImageLab8) => {
            // NO_PILLOW_SUPPORT: keep the 3x4 matrix output in a sample
            // iterator and feed it to the LAB kernel without an RGB image.
            // TODO(pillow-rs#3ptb): verify L's 12-coefficient route, the
            // deterministic RGB/4 extension, LAB/4 rejection, raw three-band
            // RGB/YCbCr/HSV/LAB sources, CoreOnly source extensions, and
            // Default/fallback receipts in CPU parity.
            let rgb_samples = pixels.map(|pixel| apply_rgb_matrix(to_rgb(pixel), matrix));
            super::lab_target::execute(width, height, rgb_samples)
        }
        (_, Mode::ImageLab8) => Err(PilError::TypeError(
            "matrix coefficient count does not match target mode".into(),
        )),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
#[inline(always)]
fn apply_matrix_row(pixel: Rgb<u8>, row: &[f64]) -> u8 {
    round_clip(
        row[0] as f32 * f32::from(pixel[0])
            + row[1] as f32 * f32::from(pixel[1])
            + row[2] as f32 * f32::from(pixel[2])
            + row[3] as f32,
    )
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
#[inline(always)]
fn apply_rgb_matrix(pixel: Rgb<u8>, matrix: &[f64]) -> [u8; 3] {
    [
        apply_matrix_row(pixel, &matrix[..4]),
        apply_matrix_row(pixel, &matrix[4..8]),
        apply_matrix_row(pixel, &matrix[8..12]),
    ]
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
#[inline(always)]
fn round_clip(value: f32) -> u8 {
    let value = value + 0.5;
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}
