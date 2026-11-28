//! Palette-index conversion used by Pillow's Equalize RGB path.
//!
//! Pillow explicitly converts P images to RGB before histogramming and
//! returns that RGB result. Keep this mode transition because it is part of
//! the operation contract, not to share an unrelated mode implementation.

use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, RgbImage};

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn expand_to_rgb(indices: &GrayImage, palette: PaletteInput<'_>) -> RgbImage {
    match palette.rgb {
        Some(colors) => crate::image::expand_palette(indices, colors, palette.alpha).to_rgb8(),
        None => DynamicImage::ImagePalette8(indices.clone()).to_rgb8(),
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn execute_rgb_in_place(
    mut input: RgbImage,
    lut: &[u8],
) -> Result<DynamicImage, PilError> {
    if lut.len() != 3 * 256 {
        return Err(PilError::InternalError(
            "P Equalize RGB lookup table shape mismatch".into(),
        ));
    }
    for pixel in input.as_mut().chunks_exact_mut(3) {
        pixel[0] = lut[usize::from(pixel[0])];
        pixel[1] = lut[256 + usize::from(pixel[1])];
        pixel[2] = lut[512 + usize::from(pixel[2])];
    }
    Ok(DynamicImage::ImageRgb8(input))
}
