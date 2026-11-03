//! Original typed alpha-mask loops and borrowed prepared dependencies.

use crate::raster::{
    GrayAlphaImage, GrayImage, ImageBuffer, Luma, LumaA, Pixel, Rgb, RgbImage, Rgba, RgbaImage,
    Rgb32FImage, Rgba32FImage,
};
use super::alpha_mask::MaskStrategy;

pub(crate) struct AlphaDataInput<'a, P: Pixel> {
    pub(crate) image: &'a crate::raster::ImageBuffer<P, Vec<P::Subpixel>>,
    pub(crate) mask: MaskStrategy<'a>,
}

pub(crate) fn facts<P: Pixel>(input: &AlphaDataInput<'_, P>)
    -> crate::compute::registry::support::StrategyInputs {
    crate::compute::registry::support::StrategyInputs {
        width: input.image.width(), height: input.image.height(),
        byte_len: Some(std::mem::size_of_val(input.image.as_raw().as_slice())),
        palette: crate::compute::registry::support::PaletteKnowledge::Absent,
        request: crate::compute::registry::support::RequestFacts::PutAlphaData {
            mask: input.mask.facts(),
        },
    }
}

pub(crate) fn native_mask<'a, P: Pixel<Subpixel = u8>>(
    input: &AlphaDataInput<'a, P>,
) -> Option<&'a GrayImage> {
    let mask = input.mask.image();
    (input.image.dimensions() == mask.dimensions()
        && input.image.as_raw().len()
            == mask.as_raw().len().saturating_mul(usize::from(P::CHANNEL_COUNT)))
    .then_some(mask)
}

pub(crate) fn luma_mask(luma: &GrayImage, mask: MaskStrategy<'_>) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(luma.width(), luma.height());
    let mask = mask.samples();
    for ((destination, source), alpha) in output.pixels_mut().zip(luma.pixels())
        .zip(mask) {
        destination[0] = source[0];
        destination[1] = *alpha;
    }
    output
}

pub(crate) fn luma_alpha_mask(luma: &GrayAlphaImage, mask: MaskStrategy<'_>) -> GrayAlphaImage {
    let mut output = luma.clone();
    let hidden_lanes = output.take_hidden_pixel_lanes();
    let mask = mask.samples();
    for (pixel, alpha) in output.as_mut().chunks_exact_mut(2).zip(mask) {
        pixel[1] = *alpha;
    }
    // putband changes the visible alpha band and leaves Pillow's other raw
    // lanes untouched. Preserve those lanes across the compact carrier write.
    let lanes_restored = output.replace_hidden_pixel_lanes(hidden_lanes);
    debug_assert!(lanes_restored);
    output
}

pub(crate) fn rgba_mask(rgba: &RgbaImage, mask: MaskStrategy<'_>) -> RgbaImage {
    let mut output = RgbaImage::new(rgba.width(), rgba.height());
    let mask = mask.samples();
    for ((destination, source), alpha) in output.pixels_mut().zip(rgba.pixels())
        .zip(mask) {
        *destination = *source;
        destination[3] = *alpha;
    }
    output
}

pub(crate) fn native_rgba_mask(source: &RgbaImage, mask: &GrayImage) -> RgbaImage {
    let mut output = source.clone();
    replace_rgba_mask(&mut output, mask.as_raw());
    output
}

pub(crate) fn replace_rgba_mask(output: &mut RgbaImage, mask: &[u8]) {
    for (pixel, alpha) in output.as_mut().chunks_exact_mut(4).zip(mask) {
        pixel[3] = *alpha;
    }
}

pub(crate) fn luma16_mask(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    mask: &[u8],
) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
    let mut output = ImageBuffer::new(image.width(), image.height());
    for ((destination, source), alpha) in output
        .pixels_mut()
        .zip(image.pixels())
        .zip(mask)
    {
        *destination = LumaA([source[0], super::put_alpha::alpha_u8_to_u16(*alpha)]);
    }
    output
}

pub(crate) fn replace_luma_alpha16_mask(
    image: &ImageBuffer<LumaA<u16>, Vec<u16>>,
    mask: &[u8],
) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
    let mut output = image.clone();
    for (pixel, alpha) in output.pixels_mut().zip(mask) {
        pixel[1] = super::put_alpha::alpha_u8_to_u16(*alpha);
    }
    output
}

pub(crate) fn rgb16_mask(
    image: &ImageBuffer<Rgb<u16>, Vec<u16>>,
    mask: &[u8],
) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    let mut output = ImageBuffer::new(image.width(), image.height());
    for ((destination, source), alpha) in output
        .pixels_mut()
        .zip(image.pixels())
        .zip(mask)
    {
        *destination = Rgba([
            source[0],
            source[1],
            source[2],
            super::put_alpha::alpha_u8_to_u16(*alpha),
        ]);
    }
    output
}

pub(crate) fn rgba16_mask(
    image: &ImageBuffer<Rgba<u16>, Vec<u16>>,
    mask: &[u8],
) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    let mut output = image.clone();
    for (pixel, alpha) in output.pixels_mut().zip(mask) {
        pixel[3] = super::put_alpha::alpha_u8_to_u16(*alpha);
    }
    output
}

pub(crate) fn rgb32f_mask(image: &Rgb32FImage, mask: &[u8]) -> Rgba32FImage {
    let mut output = Rgba32FImage::new(image.width(), image.height());
    for ((destination, source), alpha) in output
        .pixels_mut()
        .zip(image.pixels())
        .zip(mask)
    {
        *destination = Rgba([
            source[0],
            source[1],
            source[2],
            super::put_alpha::alpha_u8_to_f32(*alpha),
        ]);
    }
    output
}

pub(crate) fn rgba32f_mask(image: &Rgba32FImage, mask: &[u8]) -> Rgba32FImage {
    let mut output = image.clone();
    for (pixel, alpha) in output.pixels_mut().zip(mask) {
        pixel[3] = super::put_alpha::alpha_u8_to_f32(*alpha);
    }
    output
}

pub(crate) fn native_rgb_mask(source: &RgbImage, mask: &[u8]) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for ((destination, color), alpha) in output.as_mut().chunks_exact_mut(4)
        .zip(source.as_raw().chunks_exact(3)).zip(mask) {
        destination[..3].copy_from_slice(color);
        destination[3] = *alpha;
    }
    output
}
