//! Typed dependencies and original scalar alpha-insertion loops.

use crate::raster::{GrayAlphaImage, GrayImage, Pixel, RgbImage, RgbaImage};

pub(crate) struct AlphaInput<'a, P: Pixel> {
    pub(crate) image: &'a crate::raster::ImageBuffer<P, Vec<P::Subpixel>>,
    pub(crate) alpha: u8,
}

pub(crate) fn facts<P: Pixel>(input: &AlphaInput<'_, P>)
    -> crate::compute::registry::support::StrategyInputs {
    crate::compute::registry::support::StrategyInputs {
        width: input.image.width(),
        height: input.image.height(),
        byte_len: Some(std::mem::size_of_val(input.image.as_raw().as_slice())),
        palette: crate::compute::registry::support::PaletteKnowledge::Absent,
        request: crate::compute::registry::support::RequestFacts::Unspecified,
    }
}

pub(crate) fn luma_constant(luma: &GrayImage, alpha: u8) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(luma.width(), luma.height());
    for (destination, source) in output.pixels_mut().zip(luma.pixels()) {
        destination[0] = source[0];
        destination[1] = alpha;
    }
    output
}

pub(crate) fn replace_luma_alpha(source: &GrayAlphaImage, alpha: u8) -> GrayAlphaImage {
    let mut output = source.clone();
    for pixel in output.as_mut().chunks_exact_mut(2) {
        pixel[1] = alpha;
    }
    output
}

pub(crate) fn rgb_constant(rgb: &RgbImage, alpha: u8) -> RgbaImage {
    let mut output = RgbaImage::new(rgb.width(), rgb.height());
    for (destination, source) in output.as_mut().chunks_exact_mut(4)
        .zip(rgb.as_raw().chunks_exact(3)) {
        destination[..3].copy_from_slice(source);
        destination[3] = alpha;
    }
    output
}

pub(crate) fn replace_rgba_alpha(mut rgba: RgbaImage, alpha: u8) -> RgbaImage {
    for pixel in rgba.pixels_mut() {
        pixel[3] = alpha;
    }
    rgba
}
