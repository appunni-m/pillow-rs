//! NEON-selected CMYK-to-RGB vector implementation.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage, RgbaImage};
use fearless_simd::prelude::{SimdBase, SimdInterleaved, SimdNarrow, SimdWiden};
use fearless_simd::{Neon, u8x16 as SafeU8x16, u16x8 as SafeU16x8};

pub(crate) fn execute(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    // SAFETY: this function is reached only through the prepared Convert
    // strategy whose requirements include the host's NEON capability.
    unsafe { execute_neon(source) }.map(DynamicImage::ImageRgb8)
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[allow(unsafe_code)]
unsafe fn execute_neon(source: &RgbaImage) -> Result<RgbImage, PilError> {
    let source_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    let output_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 3)?;
    if source.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "CMYK Convert source storage did not match its dimensions".into(),
        ));
    }
    let neon = Neon::assume_supported();
    let pixels = source_dims.total_pixels();
    let vector_pixels = pixels / 16 * 16;
    let vector_source_bytes = vector_pixels * 4;
    let vector_output_bytes = vector_pixels * 3;
    let mut output = vec![0u8; output_dims.total_bytes()];
    convert_vector_blocks(
        neon,
        &source.as_raw()[..vector_source_bytes],
        &mut output[..vector_output_bytes],
    );
    convert_scalar_tail(source.as_raw(), &mut output, vector_pixels, pixels);
    let vector_blocks = (vector_pixels / 16) as u64;
    let scalar_tail = (pixels - vector_pixels) as u64;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path(if pixels == 0 {
        "scalar-control"
    } else {
        "vector"
    });
    if output.capacity() != 0 {
        crate::compute::record_pipeline_allocation(output.capacity());
    }
    RgbImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("CMYK Convert RGB output shape mismatch".into()))
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn convert_vector_blocks(neon: Neon, source: &[u8], output: &mut [u8]) {
    for (source_block, output_block) in source
        .chunks_exact(64)
        .zip(output.chunks_exact_mut(48))
    {
        let channels = SafeU8x16::<Neon>::load_four_interleaved(
            neon,
            source_block.try_into().expect("complete CMYK NEON block"),
        );
        let ink = SafeU8x16::<Neon>::splat(neon, 255) - channels[3];
        let red = ink - muldiv255_neon(neon, channels[0], ink);
        let green = ink - muldiv255_neon(neon, channels[1], ink);
        let blue = ink - muldiv255_neon(neon, channels[2], ink);
        let mut red_lanes = [0u8; 16];
        let mut green_lanes = [0u8; 16];
        let mut blue_lanes = [0u8; 16];
        red.store_array(&mut red_lanes);
        green.store_array(&mut green_lanes);
        blue.store_array(&mut blue_lanes);
        for lane in 0..16 {
            output_block[lane * 3..lane * 3 + 3]
                .copy_from_slice(&[red_lanes[lane], green_lanes[lane], blue_lanes[lane]]);
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn muldiv255_neon(
    neon: Neon,
    value: SafeU8x16<Neon>,
    ink: SafeU8x16<Neon>,
) -> SafeU8x16<Neon> {
    let (value_low, value_high) = value.widen();
    let (ink_low, ink_high) = ink.widen();
    let round = |product: SafeU16x8<Neon>| {
        let value = product + SafeU16x8::<Neon>::splat(neon, 128);
        ((value >> 8u32) + value) >> 8u32
    };
    round(value_low * ink_low).narrow(round(value_high * ink_high))
}

#[cfg(target_arch = "aarch64")]
fn convert_scalar_tail(source: &[u8], output: &mut [u8], start: usize, end: usize) {
    for pixel in start..end {
        let source_start = pixel * 4;
        let output_start = pixel * 3;
        let cmyk = &source[source_start..source_start + 4];
        let ink = 255u32 - u32::from(cmyk[3]);
        output[output_start] = (ink - crate::color::muldiv255(u32::from(cmyk[0]), ink)) as u8;
        output[output_start + 1] =
            (ink - crate::color::muldiv255(u32::from(cmyk[1]), ink)) as u8;
        output[output_start + 2] =
            (ink - crate::color::muldiv255(u32::from(cmyk[2]), ink)) as u8;
    }
}
