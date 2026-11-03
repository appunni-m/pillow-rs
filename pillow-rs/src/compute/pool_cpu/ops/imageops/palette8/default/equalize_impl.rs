//! Palette-index conversion used by Pillow's Equalize RGB path.
//!
//! Pillow explicitly converts P images to RGB before histogramming and
//! returns that RGB result. Keep this mode transition because it is part of
//! the operation contract, not to share an unrelated mode implementation.

use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, RgbImage};

pub(crate) fn expand_to_rgb(indices: &GrayImage, palette: PaletteInput<'_>) -> RgbImage {
    match palette.rgb {
        Some(colors) => crate::image::expand_palette(indices, colors, palette.alpha).to_rgb8(),
        None => DynamicImage::ImagePalette8(indices.clone()).to_rgb8(),
    }
}

pub(crate) fn execute_rgb(input: &RgbImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    super::super::super::rgb8::default::equalize_impl::execute(input, lut)
}
