//! Portable-wide SIMD order statistics over native F-mode samples.

use crate::error::PilError;
use wide::f32x8;

const LANES: usize = 8;
const MAX_AREA: usize = 81;
pub(in crate::compute::pool_cpu::ops::filter::f32) const MAX_SIZE: u32 = 9;

// VERIFY: source=reviewed@2026-11-13; parity=pass@f32-rank-comparator-cpu-architecture-matrix-20261113; benchmark=pending
#[inline]
fn sort_vectors(values: &mut [f32x8]) {
    for pass in 0..values.len() {
        let mut index = pass & 1;
        while index + 1 < values.len() {
            let left = values[index];
            let right = values[index + 1];
            values[index] = left.min(right);
            values[index + 1] = left.max(right);
            index += 2;
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pass@f32-rank-comparator-cpu-architecture-matrix-20261113; benchmark=pending
#[inline]
fn sample_window_vector(
    raw: &[u8],
    width: usize,
    height: usize,
    x_start: usize,
    active: usize,
    y: usize,
    half: usize,
    row_offset: usize,
    column_offset: usize,
) -> f32x8 {
    let mut values = [0.0f32; LANES];
    let source_y = y
        .saturating_add(row_offset)
        .saturating_sub(half)
        .min(height - 1);
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(column_offset)
            .saturating_sub(half)
            .min(width - 1);
        let base = (source_y * width + source_x) * 4;
        *value = f32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]]);
    }
    f32x8::from(values)
}

// VERIFY: source=reviewed@2026-11-13; parity=pass@f32-rank-comparator-cpu-architecture-matrix-20261113; benchmark=pending
#[inline]
fn store_vector(output_row: &mut [u8], x_start: usize, active: usize, values: f32x8) {
    for (lane, value) in values.to_array().into_iter().enumerate().take(active) {
        let base = (x_start + lane) * 4;
        output_row[base..base + 4].copy_from_slice(&value.to_le_bytes());
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pass@f32-rank-comparator-cpu-architecture-matrix-20261113; benchmark=pending
fn filter_row(
    raw: &[u8],
    output_row: &mut [u8],
    width: usize,
    height: usize,
    size: usize,
    half: usize,
    area: usize,
    rank: usize,
    y: usize,
) {
    let mut x = 0usize;
    while x < width {
        let active = (width - x).min(LANES);
        let mut values = [f32x8::splat(0.0); MAX_AREA];
        let mut value_index = 0usize;
        for row_offset in 0..size {
            for column_offset in 0..size {
                values[value_index] = sample_window_vector(
                    raw,
                    width,
                    height,
                    x,
                    active,
                    y,
                    half,
                    row_offset,
                    column_offset,
                );
                value_index += 1;
            }
        }
        let selected = if rank == 0 {
            values[..area]
                .iter()
                .copied()
                .fold(f32x8::splat(f32::INFINITY), |current, value| {
                    current.min(value)
                })
        } else if rank + 1 == area {
            values[..area]
                .iter()
                .copied()
                .fold(f32x8::splat(f32::NEG_INFINITY), |current, value| {
                    current.max(value)
                })
        } else {
            sort_vectors(&mut values[..area]);
            values[rank]
        };
        store_vector(output_row, x, active, selected);
        x += active;
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pass@f32-rank-comparator-cpu-architecture-matrix-20261113; benchmark=pending
pub(in crate::compute::pool_cpu::ops::filter::f32) fn execute(
    raw: &[u8],
    width: u32,
    height: u32,
    size: u32,
    rank: u32,
) -> Result<Vec<u8>, PilError> {
    let Some(area) = (size as usize).checked_mul(size as usize) else {
        return Err(PilError::InternalError(
            "F rank vector window overflow".into(),
        ));
    };
    if size == 0 || size % 2 == 0 || size > MAX_SIZE || rank as usize >= area {
        return Err(PilError::InternalError(
            "F rank vector received an unsupported window".into(),
        ));
    }
    let (width, height) = (width as usize, height as usize);
    let expected = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(raw.len()) {
        return Err(PilError::InternalError(
            "F rank vector received invalid native dimensions".into(),
        ));
    }

    let size = size as usize;
    let half = size / 2;
    let rank = rank as usize;
    if area > 1 && super::native_uniform_f_mode_image(raw, width, height) {
        crate::compute::record_pipeline_operation_path("native-identity");
        return Ok(raw.to_vec());
    }

    let mut output = raw.to_vec();
    for y in 0..height {
        let row_start = y * width * 4;
        let row_end = row_start + width * 4;
        filter_row(
            raw,
            &mut output[row_start..row_end],
            width,
            height,
            size,
            half,
            area,
            rank,
            y,
        );
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(
        width.div_ceil(LANES) as u64 * height as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(0);
    Ok(output)
}
