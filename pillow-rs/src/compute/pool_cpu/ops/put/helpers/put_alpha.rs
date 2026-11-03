//! Typed dependencies and original scalar alpha-insertion loops.

use crate::raster::{
    GrayAlphaImage, GrayImage, ImageBuffer, Luma, LumaA, Pixel, Rgb, RgbImage, Rgba, RgbaImage,
    Rgb32FImage, Rgba32FImage,
};

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
    let hidden_lanes = output.take_hidden_pixel_lanes();
    for pixel in output.as_mut().chunks_exact_mut(2) {
        pixel[1] = alpha;
    }
    // Pillow updates only the alpha band in its four-byte LA-family storage.
    // The compact Rust carrier omits the two intervening physical lanes.
    let lanes_restored = output.replace_hidden_pixel_lanes(hidden_lanes);
    debug_assert!(lanes_restored);
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

#[inline]
pub(crate) fn alpha_u8_to_u16(alpha: u8) -> u16 {
    u16::from(alpha) * 257
}

#[inline]
pub(crate) fn alpha_u8_to_f32(alpha: u8) -> f32 {
    f32::from(alpha) / 255.0
}

pub(crate) fn luma16_with_alpha(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    alpha: u16,
) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
    let mut output = ImageBuffer::new(image.width(), image.height());
    for (destination, source) in output.pixels_mut().zip(image.pixels()) {
        *destination = LumaA([source[0], alpha]);
    }
    output
}

pub(crate) fn replace_luma_alpha16(
    image: &ImageBuffer<LumaA<u16>, Vec<u16>>,
    alpha: u16,
) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
    let mut output = image.clone();
    for pixel in output.pixels_mut() {
        pixel[1] = alpha;
    }
    output
}

pub(crate) fn rgb16_with_alpha(
    image: &ImageBuffer<Rgb<u16>, Vec<u16>>,
    alpha: u16,
) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    let mut output = ImageBuffer::new(image.width(), image.height());
    for (destination, source) in output.pixels_mut().zip(image.pixels()) {
        *destination = Rgba([source[0], source[1], source[2], alpha]);
    }
    output
}

pub(crate) fn rgba16_with_alpha(
    image: &ImageBuffer<Rgba<u16>, Vec<u16>>,
    alpha: u16,
) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    let mut output = image.clone();
    for pixel in output.pixels_mut() {
        pixel[3] = alpha;
    }
    output
}

pub(crate) fn rgb32f_with_alpha(image: &Rgb32FImage, alpha: f32) -> Rgba32FImage {
    let mut output = Rgba32FImage::new(image.width(), image.height());
    for (destination, source) in output.pixels_mut().zip(image.pixels()) {
        *destination = Rgba([source[0], source[1], source[2], alpha]);
    }
    output
}

pub(crate) fn rgba32f_with_alpha(image: &Rgba32FImage, alpha: f32) -> Rgba32FImage {
    let mut output = image.clone();
    for pixel in output.pixels_mut() {
        pixel[3] = alpha;
    }
    output
}
