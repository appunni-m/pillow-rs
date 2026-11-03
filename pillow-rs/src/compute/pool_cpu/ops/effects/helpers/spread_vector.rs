//! SIMD byte data plane for native-mode EffectSpread.

use crate::compute::helpers::effects::{
    DarwinRand, effect_spread_advance_identity_rng, with_process_rng,
};
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use wide::u8x16;

pub(crate) struct Kernel<'a> {
    source: &'a [u8],
    width: u32,
    height: u32,
    channels: usize,
    distance: u32,
    hidden_lanes: Option<&'a HiddenPixelLanes>,
}

pub(crate) struct Output {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bytes: Vec<u8>,
    pub(crate) hidden_lanes: Option<HiddenPixelLanes>,
}

pub(crate) fn prepare<'a>(
    source: &'a [u8],
    width: u32,
    height: u32,
    channels: usize,
    distance: u32,
) -> Kernel<'a> {
    Kernel { source, width, height, channels, distance, hidden_lanes: None }
}

pub(crate) fn prepare_with_hidden_lanes<'a>(
    source: &'a [u8],
    width: u32,
    height: u32,
    channels: usize,
    distance: u32,
    hidden_lanes: Option<&'a HiddenPixelLanes>,
) -> Kernel<'a> {
    Kernel { source, width, height, channels, distance, hidden_lanes }
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
            hidden_lanes: kernel.hidden_lanes.cloned(),
        });
    }
    if kernel.distance == 0 {
        return copy_vectorized(width, height, source, kernel.hidden_lanes.cloned());
    }
    if kernel.distance == 1 {
        effect_spread_advance_identity_rng(width, height)?;
        crate::compute::record_pipeline_operation_path("native-identity");
        crate::compute::record_pipeline_operation_vector_blocks(0);
        crate::compute::record_pipeline_operation_scalar_tail(0);
        return Ok(Output {
            width,
            height,
            bytes: source.to_vec(),
            hidden_lanes: kernel.hidden_lanes.cloned(),
        });
    }

    let width_i64 = i64::from(width);
    let height_i64 = i64::from(height);
    let distance_u32 = kernel.distance;
    let distance = u64::from(distance_u32);
    let half_distance = i64::from(kernel.distance / 2);
    let mut output = source.to_vec();
    let (vector_blocks, lane_rng) = with_process_rng(|rng| {
        let lane_rng = kernel
            .hidden_lanes
            .as_ref()
            .filter(|lanes| !lanes.is_constant())
            .map(|_| rng.clone());
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
        (vector_blocks, lane_rng)
    })?;

    let hidden_lanes = match (kernel.hidden_lanes, lane_rng) {
        (Some(lanes), Some(rng)) => Some(carry_hidden_lanes(
            lanes,
            width,
            height,
            distance_u32,
            rng,
        )?),
        (Some(lanes), None) => Some(lanes.clone()),
        (None, _) => None,
    };

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    Ok(Output { width, height, bytes: output, hidden_lanes })
}

fn copy_vectorized(
    width: u32,
    height: u32,
    source: &[u8],
    hidden_lanes: Option<HiddenPixelLanes>,
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
    Ok(Output { width, height, bytes: output, hidden_lanes })
}

fn carry_hidden_lanes(
    source: &HiddenPixelLanes,
    width: u32,
    height: u32,
    distance: u32,
    mut rng: DarwinRand,
) -> Result<HiddenPixelLanes, PilError> {
    let pixel_count = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::ValueError("SIMD EffectSpread lane extent overflow".into()))?;
    let lane_count = source.lane_count();
    if !(1..=2).contains(&lane_count) || source.pixel_count() != pixel_count {
        return Err(PilError::InternalError(
            "SIMD EffectSpread lane payload did not match its dimensions".into(),
        ));
    }
    if source.is_constant() || pixel_count == 0 {
        return Ok(source.clone());
    }
    let value_count = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::ValueError("SIMD EffectSpread lane extent overflow".into()))?;
    let mut values = Vec::with_capacity(value_count);
    for pixel in 0..pixel_count {
        values.extend_from_slice(source.for_pixel(pixel).ok_or_else(|| {
            PilError::InternalError("SIMD EffectSpread source lane was unavailable".into())
        })?);
    }
    update_spread_lanes(source, &mut values, width, height, distance, &mut rng)?;
    HiddenPixelLanes::per_pixel(lane_count, pixel_count, values)
        .ok_or_else(|| PilError::InternalError("SIMD EffectSpread lane output was invalid".into()))
}

fn update_spread_lanes(
    source: &HiddenPixelLanes,
    output: &mut [u8],
    width: u32,
    height: u32,
    distance: u32,
    rng: &mut DarwinRand,
) -> Result<(), PilError> {
    let width_i64 = i64::from(width);
    let height_i64 = i64::from(height);
    let half_distance = i64::from(distance / 2);
    let distance = u64::from(distance);
    let lane_count = source.lane_count();
    for y in 0..height_i64 {
        for x in 0..width_i64 {
            let source_pixel = (y * width_i64 + x) as usize;
            let destination_x = x + i64::from((u64::from(rng.next()) % distance) as u32)
                - half_distance;
            let destination_y = y + i64::from((u64::from(rng.next()) % distance) as u32)
                - half_distance;
            if destination_x >= 0
                && destination_x < width_i64
                && destination_y >= 0
                && destination_y < height_i64
            {
                let destination_pixel = (destination_y * width_i64 + destination_x) as usize;
                copy_lane_pixel(source, output, source_pixel, destination_pixel, lane_count)?;
                copy_lane_pixel(source, output, destination_pixel, source_pixel, lane_count)?;
            }
        }
    }
    Ok(())
}

fn copy_lane_pixel(
    source: &HiddenPixelLanes,
    output: &mut [u8],
    source_pixel: usize,
    destination_pixel: usize,
    lane_count: usize,
) -> Result<(), PilError> {
    let source = source.for_pixel(source_pixel).ok_or_else(|| {
        PilError::InternalError("SIMD EffectSpread source lane index was invalid".into())
    })?;
    let destination_start = destination_pixel
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::ValueError("SIMD EffectSpread lane offset overflow".into()))?;
    let destination_end = destination_start
        .checked_add(lane_count)
        .ok_or_else(|| PilError::ValueError("SIMD EffectSpread lane range overflow".into()))?;
    let destination = output.get_mut(destination_start..destination_end).ok_or_else(|| {
        PilError::InternalError("SIMD EffectSpread lane output index was invalid".into())
    })?;
    destination.copy_from_slice(source);
    Ok(())
}

fn load_pixel(source: &[u8], start: usize, channels: usize) -> [u8; 16] {
    let mut block = [0u8; 16];
    block[..channels].copy_from_slice(&source[start..start + channels]);
    block
}
