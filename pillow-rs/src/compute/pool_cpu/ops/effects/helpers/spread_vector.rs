//! SIMD byte data plane for native-mode EffectSpread.

use crate::compute::helpers::effects::{effect_spread_advance_identity_rng, with_process_rng};
use crate::error::PilError;
use wide::u8x16;

pub(crate) struct Kernel<'a> {
    source: &'a [u8],
    width: u32,
    height: u32,
    channels: usize,
    distance: u32,
}

pub(crate) struct Output {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) fn prepare<'a>(
    source: &'a [u8],
    width: u32,
    height: u32,
    channels: usize,
    distance: u32,
) -> Kernel<'a> {
    Kernel { source, width, height, channels, distance }
}

pub(crate) fn execute_sse2(kernel: Kernel<'_>) -> Result<Output, PilError> {
    execute_vector(kernel)
}

pub(crate) fn execute_neon(kernel: Kernel<'_>) -> Result<Output, PilError> {
    execute_vector(kernel)
}

pub(crate) fn execute_wasm_simd128(kernel: Kernel<'_>) -> Result<Output, PilError> {
    execute_vector(kernel)
}

fn execute_vector(kernel: Kernel<'_>) -> Result<Output, PilError> {
    let width = kernel.width;
    let height = kernel.height;
    let channels = kernel.channels;
    let expected_len = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD EffectSpread image length overflow".into()))?;
    let source = kernel.source;
    if source.len() != expected_len {
        return Err(PilError::InternalError(
            "SIMD EffectSpread source byte length did not match its dimensions".into(),
        ));
    }

    if width == 0 || height == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Output {
            width,
            height,
            bytes: Vec::new(),
        });
    }
    if kernel.distance == 0 {
        return copy_vectorized(width, height, source);
    }
    if kernel.distance == 1 {
        effect_spread_advance_identity_rng(width, height)?;
        crate::compute::record_pipeline_operation_path("native-identity");
        crate::compute::record_pipeline_operation_vector_blocks(0);
        crate::compute::record_pipeline_operation_scalar_tail(0);
        return Ok(Output { width, height, bytes: source.to_vec() });
    }

    let width_i64 = i64::from(width);
    let height_i64 = i64::from(height);
    let distance = u64::from(kernel.distance);
    let half_distance = i64::from(kernel.distance / 2);
    let mut output = source.to_vec();
    let vector_blocks = with_process_rng(|rng| {
        let mut vector_blocks = 0u64;
        for y in 0..height_i64 {
            for x in 0..width_i64 {
                let source_pixel = (y * width_i64 + x) as usize;
                let source_start = source_pixel * channels;
                let destination_x = x + i64::from((u64::from(rng.next()) % distance) as u32)
                    - half_distance;
                let destination_y = y + i64::from((u64::from(rng.next()) % distance) as u32)
                    - half_distance;
                let source_block = load_pixel(source, source_start, channels);
                if destination_x >= 0
                    && destination_x < width_i64
                    && destination_y >= 0
                    && destination_y < height_i64
                {
                    let destination_pixel = (destination_y * width_i64 + destination_x) as usize;
                    let destination_start = destination_pixel * channels;
                    let destination_block = load_pixel(source, destination_start, channels);
                    let source_vector = u8x16::new(source_block).to_array();
                    let destination_vector = u8x16::new(destination_block).to_array();
                    output[destination_start..destination_start + channels]
                        .copy_from_slice(&source_vector[..channels]);
                    output[source_start..source_start + channels]
                        .copy_from_slice(&destination_vector[..channels]);
                } else {
                    let source_vector = u8x16::new(source_block).to_array();
                    output[source_start..source_start + channels]
                        .copy_from_slice(&source_vector[..channels]);
                }
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
        vector_blocks
    })?;

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    Ok(Output { width, height, bytes: output })
}

fn copy_vectorized(
    width: u32,
    height: u32,
    source: &[u8],
) -> Result<Output, PilError> {
    let mut output = vec![0u8; source.len()];
    let vector_len = source.len() / 16 * 16;
    for (input, destination) in source[..vector_len]
        .chunks_exact(16)
        .zip(output[..vector_len].chunks_exact_mut(16))
    {
        let vector = u8x16::new(<[u8; 16]>::try_from(input).map_err(|_| {
            PilError::InternalError("SIMD EffectSpread vector width mismatch".into())
        })?);
        destination.copy_from_slice(&vector.to_array());
    }
    output[vector_len..].copy_from_slice(&source[vector_len..]);
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks((vector_len / 16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((source.len() - vector_len) as u64);
    Ok(Output { width, height, bytes: output })
}

fn load_pixel(source: &[u8], start: usize, channels: usize) -> [u8; 16] {
    let mut block = [0u8; 16];
    block[..channels].copy_from_slice(&source[start..start + channels]);
    block
}
