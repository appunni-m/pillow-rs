//! Native byte formulas shared by exact-mode ColorSaturation strategies.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::channel::{
    gather::gather_interleaved_channel,
    wide::simd_pack_u16x16,
};
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};
use wide::{u8x16, u16x16};

pub(crate) const BASE_LUMA: u8 = 0;
pub(crate) const BASE_RGB: u8 = 1;
pub(crate) const BASE_RGBX: u8 = 2;
pub(crate) const BASE_CMYK: u8 = 3;

pub(crate) fn execute<P, const CHANNELS: u8, const BASE: u8>(
    source: &ImageBuffer<P, Vec<u8>>,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), CHANNELS)?;
    let source_bytes = source.as_raw();
    if source_bytes.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "ColorSaturation source bytes did not match its exact mode".into(),
        ));
    }

    let mut output = vec![0; source_bytes.len()];
    if CHANNELS == 4 && BASE == BASE_RGB && factor == 0.0 {
        for (input, result) in source_bytes.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
            let gray = crate::color::rgb_to_luma_u8(input[0], input[1], input[2]);
            result[..3].fill(gray);
            result[3] = input[3];
        }
    } else {
        let factor = factor as f32;
        for (input, result) in source_bytes
            .chunks_exact(usize::from(CHANNELS))
            .zip(output.chunks_exact_mut(usize::from(CHANNELS)))
        {
            let base = pixel_base::<BASE>(input);
            for (channel, destination) in result.iter_mut().enumerate() {
                let base = f32::from(base[channel]);
                *destination = factor
                    .mul_add(f32::from(input[channel]) - base, base)
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }

    ImageBuffer::from_raw(source.width(), source.height(), output).ok_or_else(|| {
        PilError::InternalError("ColorSaturation output dimensions were invalid".into())
    })
}

/// Execute the archived eight-byte f32 blend for an exact native mode layout.
pub(crate) fn execute_vector<P, const CHANNELS: u8, const BASE: u8>(
    source: &ImageBuffer<P, Vec<u8>>,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), CHANNELS)?;
    let source_bytes = source.as_raw();
    if source_bytes.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "ColorSaturation source bytes did not match its exact mode".into(),
        ));
    }
    let factor_f32 = factor as f32;
    if !factor.is_finite() || !factor_f32.is_finite() || source_bytes.len() < 8 {
        return Err(PilError::InternalError(
            "ColorSaturation vector strategy received an ineligible request".into(),
        ));
    }
    if CHANNELS == 4 && BASE == BASE_RGB && factor == 0.0 {
        return rgba_color_degenerate(source);
    }

    let factor = factor_f32;
    let vector_bytes = source_bytes.len() / 8 * 8;
    let mut output = vec![0; source_bytes.len()];
    let scale = wide::f32x8::splat(factor);
    for (block, result) in output[..vector_bytes].chunks_exact_mut(8).enumerate() {
        let offset = block * 8;
        let input = wide::f32x8::new(std::array::from_fn(|lane| {
            f32::from(source_bytes[offset + lane])
        }));
        let base = wide::f32x8::new(std::array::from_fn(|lane| {
            let index = offset + lane;
            let pixel_start = index / usize::from(CHANNELS) * usize::from(CHANNELS);
            f32::from(pixel_base::<BASE>(
                &source_bytes[pixel_start..pixel_start + usize::from(CHANNELS)],
            )[index % usize::from(CHANNELS)])
        }));
        let values = scale.mul_add(input - base, base).to_array();
        for (destination, value) in result.iter_mut().zip(values) {
            *destination = value.clamp(0.0, 255.0) as u8;
        }
    }

    for (tail_index, destination) in output[vector_bytes..].iter_mut().enumerate() {
        let index = vector_bytes + tail_index;
        let pixel_start = index / usize::from(CHANNELS) * usize::from(CHANNELS);
        let base = f32::from(
            pixel_base::<BASE>(
                &source_bytes[pixel_start..pixel_start + usize::from(CHANNELS)],
            )[index % usize::from(CHANNELS)],
        );
        *destination = factor
            .mul_add(f32::from(source_bytes[index]) - base, base)
            .clamp(0.0, 255.0) as u8;
    }

    crate::compute::record_pipeline_operation_vector_blocks((vector_bytes / 8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((source_bytes.len() - vector_bytes) as u64);
    crate::compute::record_pipeline_operation_path("vector");
    ImageBuffer::from_raw(source.width(), source.height(), output).ok_or_else(|| {
        PilError::InternalError("ColorSaturation output dimensions were invalid".into())
    })
}

const LUMA_TO_RGBA_0: u8x16 = u8x16::new([0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3]);
const LUMA_TO_RGBA_1: u8x16 = u8x16::new([4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7]);
const LUMA_TO_RGBA_2: u8x16 = u8x16::new([
    8, 8, 8, 8, 9, 9, 9, 9, 10, 10, 10, 10, 11, 11, 11, 11,
]);
const LUMA_TO_RGBA_3: u8x16 = u8x16::new([
    12, 12, 12, 12, 13, 13, 13, 13, 14, 14, 14, 14, 15, 15, 15, 15,
]);
const RGBA_ALPHA_LANES: u8x16 =
    u8x16::new([0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
const RGBA_COLOR_LANES: u8x16 = u8x16::new([
    255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0,
]);

fn rgba_color_degenerate<P>(
    source: &ImageBuffer<P, Vec<u8>>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    let source_bytes = source.as_raw();
    if source_bytes.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "ColorSaturation RGBA source bytes did not match its dimensions".into(),
        ));
    }
    let mut output = Vec::with_capacity(source_bytes.len());
    for source_block in source_bytes.chunks(64) {
        let active_pixels = source_block.len() / 4;
        let mut padded = [0u8; 64];
        padded[..source_block.len()].copy_from_slice(source_block);
        let load = |offset| {
            u8x16::new(
                padded[offset..offset + 16]
                    .try_into()
                    .expect("complete ColorSaturation RGBA vector"),
            )
        };
        let blocks = [load(0), load(16), load(32), load(48)];
        let red = u16x16::from(gather_interleaved_channel::<4, 0>(&blocks));
        let green = u16x16::from(gather_interleaved_channel::<4, 1>(&blocks));
        let blue = u16x16::from(gather_interleaved_channel::<4, 2>(&blocks));
        let base = red * const { u16x16::splat(77) }
            + green * const { u16x16::splat(150) }
            + blue * const { u16x16::splat(29) };
        let residual = green * const { u16x16::splat(70) }
            + blue * const { u16x16::splat(47) }
            - red * const { u16x16::splat(117) }
            + const { u16x16::splat(32768) };
        let luma = u8x16::new(
            simd_pack_u16x16((base + (residual >> 8u32)) >> 8u32).to_array(),
        );
        let alpha = gather_interleaved_channel::<4, 3>(&blocks);
        let output_vectors = [LUMA_TO_RGBA_0, LUMA_TO_RGBA_1, LUMA_TO_RGBA_2, LUMA_TO_RGBA_3]
            .map(|indices| {
                ((luma.shuffle(indices) & RGBA_COLOR_LANES)
                    | (alpha.shuffle(indices) & RGBA_ALPHA_LANES))
                    .to_array()
            });
        for (block_index, vector) in output_vectors.into_iter().enumerate() {
            let block_pixels = active_pixels.saturating_sub(block_index * 4).min(4);
            output.extend_from_slice(&vector[..block_pixels * 4]);
        }
    }
    crate::compute::record_pipeline_operation_vector_blocks(
        dimensions.total_pixels().div_ceil(4) as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    ImageBuffer::from_raw(source.width(), source.height(), output).ok_or_else(|| {
        PilError::InternalError("ColorSaturation output dimensions were invalid".into())
    })
}

fn pixel_base<const BASE: u8>(pixel: &[u8]) -> [u8; 4] {
    match BASE {
        BASE_LUMA => [pixel[0], *pixel.get(1).unwrap_or(&0), 0, 0],
        BASE_RGB => {
            let gray = crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]);
            [gray, gray, gray, *pixel.get(3).unwrap_or(&255)]
        }
        BASE_RGBX => {
            let gray = crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]);
            [gray, gray, gray, 255]
        }
        BASE_CMYK => {
            let inverse_black = 255 - u32::from(pixel[3]);
            let rgb = |component: u8| {
                (inverse_black
                    - crate::color::muldiv255(u32::from(component), inverse_black)) as u8
            };
            let gray = crate::color::rgb_to_luma_u8(rgb(pixel[0]), rgb(pixel[1]), rgb(pixel[2]));
            [0, 0, 0, 255 - gray]
        }
        _ => unreachable!("ColorSaturation mode strategy used an unknown base layout"),
    }
}
