//! Shared matrix arithmetic over mode-native source pixels.

use crate::error::PilError;
use crate::raster::{DynamicImage, FromColor, GrayImage, ImageBuffer, Mode, Pixel, Rgb, RgbImage};

/// Applies the preserved RGB-sample matrix contract without materializing an RGB image.
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
        | DynamicImage::ImageI16N(source) => {
            apply_source(source, target_mode, matrix, |pixel| {
                let value = pixel.channels()[0].min(u16::from(u8::MAX)) as u8;
                Rgb([value; 3])
            })
        }
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

fn apply_matrix<P>(
    pixels: impl ExactSizeIterator<Item = P>,
    mut to_rgb: impl FnMut(P) -> Rgb<u8>,
    width: u32,
    height: u32,
    target_mode: Mode,
    matrix: &[f64],
) -> Result<DynamicImage, PilError> {
    match (matrix.len(), target_mode) {
        (4, Mode::ImageRgb8) => {
            // Pillow 12.2 retains a legacy four-coefficient RGB-output path:
            // the affine expression is written to the first channel and the
            // other channels are zero for the deterministic public fixture.
            let pixels: Vec<u8> = pixels
                .flat_map(|pixel| {
                    let pixel = to_rgb(pixel);
                    [
                        round_clip(
                            matrix[0] * f64::from(pixel[0])
                                + matrix[1] * f64::from(pixel[1])
                                + matrix[2] * f64::from(pixel[2])
                                + matrix[3],
                        ),
                        0,
                        0,
                    ]
                })
                .collect();
            Ok(DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, pixels)
                    .ok_or_else(|| PilError::ValueError("matrix conversion failed".into()))?,
            ))
        }
        (4, Mode::ImageLuma8) => {
            let pixels: Vec<u8> = pixels
                .map(|pixel| {
                    let pixel = to_rgb(pixel);
                    round_clip(
                        matrix[0] * f64::from(pixel[0])
                            + matrix[1] * f64::from(pixel[1])
                            + matrix[2] * f64::from(pixel[2])
                            + matrix[3],
                    )
                })
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
            // TODO(pillow-rs#3ptb): verify source-mode results, four-coefficient
            // rejection, and Default/fallback receipts in CPU parity.
            let rgb_samples = pixels.map(|pixel| apply_rgb_matrix(to_rgb(pixel), matrix));
            super::lab_target::execute(width, height, rgb_samples)
        }
        (length, Mode::ImageLab8) => Err(PilError::ValueError(format!(
            "LAB matrix conversion requires 12 elements, got {length}"
        ))),
        (length, _) => Err(PilError::ValueError(format!(
            "Matrix must be 4 or 12 elements, got {length}"
        ))),
    }
}

#[inline(always)]
fn apply_rgb_matrix(pixel: Rgb<u8>, matrix: &[f64]) -> [u8; 3] {
    let red = f64::from(pixel[0]);
    let green = f64::from(pixel[1]);
    let blue = f64::from(pixel[2]);
    [
        round_clip(matrix[0] * red + matrix[1] * green + matrix[2] * blue + matrix[3]),
        round_clip(matrix[4] * red + matrix[5] * green + matrix[6] * blue + matrix[7]),
        round_clip(matrix[8] * red + matrix[9] * green + matrix[10] * blue + matrix[11]),
    ]
}

#[inline(always)]
fn round_clip(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}
