//! Original typed alpha-mask loops and borrowed prepared dependencies.

use super::alpha_mask::{MaskStrategy, normalize_sample};
use crate::raster::{
    GrayAlphaImage, GrayImage, ImageBuffer, Luma, LumaA, Pixel, Rgb, Rgb32FImage, RgbImage, Rgba,
    Rgba32FImage, RgbaImage,
};

pub(crate) struct AlphaDataInput<'a, P: Pixel> {
    pub(crate) image: &'a crate::raster::ImageBuffer<P, Vec<P::Subpixel>>,
    pub(crate) mask: MaskStrategy<'a>,
}

pub(crate) fn facts<P: Pixel>(
    input: &AlphaDataInput<'_, P>,
) -> crate::compute::registry::support::StrategyInputs {
    crate::compute::registry::support::StrategyInputs {
        width: input.image.width(),
        height: input.image.height(),
        byte_len: Some(std::mem::size_of_val(input.image.as_raw().as_slice())),
        palette: crate::compute::registry::support::PaletteKnowledge::Absent,
        request: crate::compute::registry::support::RequestFacts::PutAlphaData {
            mask: input.mask.facts(),
        },
    }
}

pub(crate) fn native_mask<'a, P: Pixel<Subpixel = u8>>(
    input: &AlphaDataInput<'a, P>,
) -> Option<MaskStrategy<'a>> {
    let mask = input.mask.image();
    (input.image.dimensions() == mask.dimensions()
        && input.image.as_raw().len()
            == mask
                .as_raw()
                .len()
                .saturating_mul(usize::from(P::CHANNEL_COUNT)))
    .then_some(input.mask)
}

pub(crate) fn luma_mask(luma: &GrayImage, mask: MaskStrategy<'_>) -> GrayAlphaImage {
    mask.with_samples(
        |samples| luma_mask_samples::<true>(luma, samples),
        |samples| luma_mask_samples::<false>(luma, samples),
    )
}

fn luma_mask_samples<const ONE: bool>(luma: &GrayImage, mask: &[u8]) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(luma.width(), luma.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(luma.pixels()).zip(mask) {
        destination[0] = source[0];
        destination[1] = normalize_sample::<ONE>(*alpha);
    }
    output
}

pub(crate) fn luma_alpha_mask(luma: &GrayAlphaImage, mask: MaskStrategy<'_>) -> GrayAlphaImage {
    mask.with_samples(
        |samples| luma_alpha_mask_samples::<true>(luma, samples),
        |samples| luma_alpha_mask_samples::<false>(luma, samples),
    )
}

fn luma_alpha_mask_samples<const ONE: bool>(luma: &GrayAlphaImage, mask: &[u8]) -> GrayAlphaImage {
    let mut output = luma.clone();
    let hidden_lanes = output.take_hidden_pixel_lanes();
    for (pixel, alpha) in output.as_mut().chunks_exact_mut(2).zip(mask) {
        pixel[1] = normalize_sample::<ONE>(*alpha);
    }
    // putband changes the visible alpha band and leaves Pillow's other raw
    // lanes untouched. Preserve those lanes across the compact carrier write.
    let lanes_restored = output.replace_hidden_pixel_lanes(hidden_lanes);
    debug_assert!(lanes_restored);
    output
}

pub(crate) fn rgba_mask(rgba: &RgbaImage, mask: MaskStrategy<'_>) -> RgbaImage {
    mask.with_samples(
        |samples| rgba_mask_samples::<true>(rgba, samples),
        |samples| rgba_mask_samples::<false>(rgba, samples),
    )
}

fn rgba_mask_samples<const ONE: bool>(rgba: &RgbaImage, mask: &[u8]) -> RgbaImage {
    let mut output = RgbaImage::new(rgba.width(), rgba.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(rgba.pixels()).zip(mask) {
        *destination = *source;
        destination[3] = normalize_sample::<ONE>(*alpha);
    }
    output
}

pub(crate) fn native_rgba_mask(source: &RgbaImage, mask: MaskStrategy<'_>) -> RgbaImage {
    mask.with_samples(
        |samples| native_rgba_mask_samples::<true>(source, samples),
        |samples| native_rgba_mask_samples::<false>(source, samples),
    )
}

fn native_rgba_mask_samples<const ONE: bool>(source: &RgbaImage, mask: &[u8]) -> RgbaImage {
    let mut output = source.clone();
    replace_rgba_mask::<ONE>(&mut output, mask);
    output
}

fn replace_rgba_mask<const ONE: bool>(output: &mut RgbaImage, mask: &[u8]) {
    for (pixel, alpha) in output.as_mut().chunks_exact_mut(4).zip(mask) {
        pixel[3] = normalize_sample::<ONE>(*alpha);
    }
}

pub(crate) fn luma16_mask(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    mask: MaskStrategy<'_>,
) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
    mask.with_samples(
        |samples| luma16_mask_samples::<true>(image, samples),
        |samples| luma16_mask_samples::<false>(image, samples),
    )
}

fn luma16_mask_samples<const ONE: bool>(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    mask: &[u8],
) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
    let mut output = ImageBuffer::new(image.width(), image.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(image.pixels()).zip(mask) {
        let alpha = normalize_sample::<ONE>(*alpha);
        *destination = LumaA([source[0], super::put_alpha::alpha_u8_to_u16(alpha)]);
    }
    output
}

pub(crate) fn replace_luma_alpha16_mask(
    image: &ImageBuffer<LumaA<u16>, Vec<u16>>,
    mask: MaskStrategy<'_>,
) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
    mask.with_samples(
        |samples| replace_luma_alpha16_mask_samples::<true>(image, samples),
        |samples| replace_luma_alpha16_mask_samples::<false>(image, samples),
    )
}

fn replace_luma_alpha16_mask_samples<const ONE: bool>(
    image: &ImageBuffer<LumaA<u16>, Vec<u16>>,
    mask: &[u8],
) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
    let mut output = image.clone();
    for (pixel, alpha) in output.pixels_mut().zip(mask) {
        let alpha = normalize_sample::<ONE>(*alpha);
        pixel[1] = super::put_alpha::alpha_u8_to_u16(alpha);
    }
    output
}

pub(crate) fn rgb16_mask(
    image: &ImageBuffer<Rgb<u16>, Vec<u16>>,
    mask: MaskStrategy<'_>,
) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    mask.with_samples(
        |samples| rgb16_mask_samples::<true>(image, samples),
        |samples| rgb16_mask_samples::<false>(image, samples),
    )
}

fn rgb16_mask_samples<const ONE: bool>(
    image: &ImageBuffer<Rgb<u16>, Vec<u16>>,
    mask: &[u8],
) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    let mut output = ImageBuffer::new(image.width(), image.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(image.pixels()).zip(mask) {
        *destination = Rgba([
            source[0],
            source[1],
            source[2],
            super::put_alpha::alpha_u8_to_u16(normalize_sample::<ONE>(*alpha)),
        ]);
    }
    output
}

pub(crate) fn rgba16_mask(
    image: &ImageBuffer<Rgba<u16>, Vec<u16>>,
    mask: MaskStrategy<'_>,
) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    mask.with_samples(
        |samples| rgba16_mask_samples::<true>(image, samples),
        |samples| rgba16_mask_samples::<false>(image, samples),
    )
}

fn rgba16_mask_samples<const ONE: bool>(
    image: &ImageBuffer<Rgba<u16>, Vec<u16>>,
    mask: &[u8],
) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    let mut output = image.clone();
    for (pixel, alpha) in output.pixels_mut().zip(mask) {
        pixel[3] = super::put_alpha::alpha_u8_to_u16(normalize_sample::<ONE>(*alpha));
    }
    output
}

pub(crate) fn rgb32f_mask(image: &Rgb32FImage, mask: MaskStrategy<'_>) -> Rgba32FImage {
    mask.with_samples(
        |samples| rgb32f_mask_samples::<true>(image, samples),
        |samples| rgb32f_mask_samples::<false>(image, samples),
    )
}

fn rgb32f_mask_samples<const ONE: bool>(image: &Rgb32FImage, mask: &[u8]) -> Rgba32FImage {
    let mut output = Rgba32FImage::new(image.width(), image.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(image.pixels()).zip(mask) {
        *destination = Rgba([
            source[0],
            source[1],
            source[2],
            super::put_alpha::alpha_u8_to_f32(normalize_sample::<ONE>(*alpha)),
        ]);
    }
    output
}

pub(crate) fn rgba32f_mask(image: &Rgba32FImage, mask: MaskStrategy<'_>) -> Rgba32FImage {
    mask.with_samples(
        |samples| rgba32f_mask_samples::<true>(image, samples),
        |samples| rgba32f_mask_samples::<false>(image, samples),
    )
}

fn rgba32f_mask_samples<const ONE: bool>(image: &Rgba32FImage, mask: &[u8]) -> Rgba32FImage {
    let mut output = image.clone();
    for (pixel, alpha) in output.pixels_mut().zip(mask) {
        pixel[3] = super::put_alpha::alpha_u8_to_f32(normalize_sample::<ONE>(*alpha));
    }
    output
}

pub(crate) fn native_rgb_mask(source: &RgbImage, mask: MaskStrategy<'_>) -> RgbaImage {
    mask.with_samples(
        |samples| native_rgb_mask_samples::<true>(source, samples),
        |samples| native_rgb_mask_samples::<false>(source, samples),
    )
}

fn native_rgb_mask_samples<const ONE: bool>(source: &RgbImage, mask: &[u8]) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for ((destination, color), alpha) in output
        .as_mut()
        .chunks_exact_mut(4)
        .zip(source.as_raw().chunks_exact(3))
        .zip(mask)
    {
        destination[..3].copy_from_slice(color);
        destination[3] = normalize_sample::<ONE>(*alpha);
    }
    output
}
