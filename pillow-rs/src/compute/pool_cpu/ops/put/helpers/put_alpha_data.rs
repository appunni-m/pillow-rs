//! Original typed alpha-mask loops and borrowed prepared dependencies.

use crate::raster::{GrayAlphaImage, GrayImage, Pixel, RgbImage, RgbaImage};
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
    input.mask.native_luma().filter(|mask| {
        input.image.dimensions() == mask.dimensions()
            && input.image.as_raw().len() == mask.as_raw().len()
                .saturating_mul(usize::from(P::CHANNEL_COUNT))
    })
}

pub(crate) fn luma_mask(luma: &GrayImage, mask: MaskStrategy<'_>) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(luma.width(), luma.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(luma.pixels())
        .zip(mask.to_luma8().pixels()) {
        destination[0] = source[0];
        destination[1] = alpha[0];
    }
    output
}

pub(crate) fn luma_alpha_mask(luma: &GrayAlphaImage, mask: MaskStrategy<'_>) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(luma.width(), luma.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(luma.pixels())
        .zip(mask.to_luma8().pixels()) {
        destination[0] = source[0];
        destination[1] = alpha[0];
    }
    output
}

pub(crate) fn rgba_mask(rgba: &RgbaImage, mask: MaskStrategy<'_>) -> RgbaImage {
    let mut output = RgbaImage::new(rgba.width(), rgba.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(rgba.pixels())
        .zip(mask.to_luma8().pixels()) {
        *destination = *source;
        destination[3] = alpha[0];
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

pub(crate) fn native_rgb_mask(source: &RgbImage, mask: &GrayImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for ((destination, color), alpha) in output.as_mut().chunks_exact_mut(4)
        .zip(source.as_raw().chunks_exact(3)).zip(mask.as_raw()) {
        destination[..3].copy_from_slice(color);
        destination[3] = *alpha;
    }
    output
}
