//! Original serial band algorithms, shared only by modes with the same contract.

use crate::checked_dims::CheckedDims;
use crate::compute::registry::support::{PaletteKnowledge, RequestFacts, StrategyInputs};
use crate::error::PilError;
use crate::raster::{
    DynamicImage, FromPrimitive, GrayImage, ImageBuffer, Luma, Pixel, Rgb,
};

pub(crate) struct CheckedBand<'a, P: Pixel> {
    pub(crate) input: &'a ImageBuffer<P, Vec<P::Subpixel>>,
    pub(crate) dimensions: CheckedDims,
    pub(crate) index: usize,
}

impl<'a, P: Pixel> CheckedBand<'a, P> {
    pub(crate) fn new(input: &'a ImageBuffer<P, Vec<P::Subpixel>>, index: u8)
        -> Result<Self, PilError> {
        let dimensions = CheckedDims::new_allow_empty(input.width(), input.height(), 1)?;
        Ok(Self { input, dimensions, index: usize::from(index) })
    }
}

pub(crate) fn facts<P: Pixel>(input: &ImageBuffer<P, Vec<P::Subpixel>>, index: u8)
    -> StrategyInputs {
    StrategyInputs {
        width: input.width(), height: input.height(),
        byte_len: Some(core::mem::size_of_val(input.as_raw().as_slice())),
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::ExtractBand { index },
    }
}

fn finish_bytes(width: u32, height: u32, samples: Vec<u8>) -> Result<DynamicImage, PilError> {
    if samples.capacity() != 0 {
        crate::compute::record_pipeline_allocation(samples.capacity());
    }
    crate::image_utils::raw_bytes_to_image_allow_empty(width, height, samples, 1)
}

fn gray(width: u32, height: u32) -> GrayImage {
    let output = GrayImage::new(width, height);
    if output.as_raw().capacity() != 0 {
        crate::compute::record_pipeline_allocation(output.as_raw().capacity());
    }
    output
}

pub(crate) fn luma(input: CheckedBand<'_, Luma<u8>>) -> Result<DynamicImage, PilError> {
    let pixel_count = input.dimensions.total_pixels();
    let samples = if input.index < 3 {
        input.input.as_raw().iter().copied().take(pixel_count).collect()
    } else {
        vec![u8::MAX; pixel_count]
    };
    finish_bytes(input.input.width(), input.input.height(), samples)
}

pub(crate) fn rgb(input: CheckedBand<'_, Rgb<u8>>) -> Result<DynamicImage, PilError> {
    let channel = input.index.min(3);
    let pixel_count = input.dimensions.total_pixels();
    let samples = if channel < 3 {
        input.input.as_raw().chunks_exact(3).take(pixel_count)
            .map(|pixel| pixel[channel]).collect()
    } else {
        vec![u8::MAX; pixel_count]
    };
    finish_bytes(input.input.width(), input.input.height(), samples)
}

/// LA and RGBA retain their original four-pixel unrolling and logical tail.
pub(crate) fn grouped<P: Pixel<Subpixel = u8>, const CHANNELS: usize>(
    input: CheckedBand<'_, P>,
) -> Result<DynamicImage, PilError> {
    let mut gray = gray(input.input.width(), input.input.height());
    let band = input.index.min(CHANNELS - 1);
    let source = input.input.as_raw();
    let output = gray.as_mut();
    let pixel_count = input.dimensions.total_pixels();
    let grouped_pixels = pixel_count / 4 * 4;
    for (dst, pixels) in output[..grouped_pixels].chunks_exact_mut(4)
        .zip(source.chunks_exact(CHANNELS * 4).take(grouped_pixels / 4)) {
        dst[0] = pixels[band];
        dst[1] = pixels[band + CHANNELS];
        dst[2] = pixels[band + CHANNELS * 2];
        dst[3] = pixels[band + CHANNELS * 3];
    }
    let source_tail = source.get(grouped_pixels * CHANNELS..).unwrap_or_default();
    for (dst, pixel) in output[grouped_pixels..].iter_mut()
        .zip(source_tail.chunks_exact(CHANNELS).take(pixel_count - grouped_pixels)) {
        *dst = pixel[band];
    }
    Ok(DynamicImage::ImageLuma8(gray))
}

/// Extracts one high-depth band directly from its native samples.
///
/// None selects the opaque byte used by the existing RGBA conversion for
/// defensive out-of-range RGB band requests.
pub(crate) fn native<P: Pixel>(
    input: CheckedBand<'_, P>,
    selected: Option<usize>,
) -> Result<DynamicImage, PilError>
where
    u8: FromPrimitive<P::Subpixel>,
{
    let channel = selected.filter(|&index| index < usize::from(P::CHANNEL_COUNT));
    let samples = input
        .input
        .as_raw()
        .chunks_exact(usize::from(P::CHANNEL_COUNT))
        .take(input.dimensions.total_pixels())
        .map(|pixel| channel.map_or(u8::MAX, |index| u8::from_primitive(pixel[index])))
        .collect();
    finish_bytes(input.input.width(), input.input.height(), samples)
}

pub(crate) fn clipped_luma16(
    input: CheckedBand<'_, Luma<u16>>,
) -> Result<DynamicImage, PilError> {
    let pixel_count = input.dimensions.total_pixels();
    let samples = if input.index < 3 {
        input
            .input
            .as_raw()
            .iter()
            .take(pixel_count)
            .map(|sample| (*sample).min(u16::from(u8::MAX)) as u8)
            .collect()
    } else {
        vec![u8::MAX; pixel_count]
    };
    finish_bytes(input.input.width(), input.input.height(), samples)
}

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", any(target_feature = "sse2", target_feature = "ssse3")),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) fn vector_output(
    width: u32, height: u32, output: (Vec<u8>, u64, u64), path: &'static str,
) -> Result<DynamicImage, PilError> {
    let (output, blocks, tail) = output;
    if blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
    }
    if tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(tail);
    }
    crate::compute::record_pipeline_operation_path(if blocks == 0 { "scalar-gather" } else { path });
    GrayImage::from_raw(width, height, output).map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("SIMD ExtractBand buffer shape mismatch".into()))
}
