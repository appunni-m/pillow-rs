//! Sixteen-lane AVX-512F rank selection over native F-mode samples.
// TODO(pillow-rs#fvsp): verify selected/fallback receipts and exact rank output
// on native AVX-512F hardware, then compare the frozen F32 rank workload.

#![allow(unsafe_code)] // The prepared AVX-512F strategy guards the target-feature leaves.

use crate::checked_dims::CheckedDims;
use crate::compute::registry::support::StrategySelection;
use crate::compute::{Architecture, Backend};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

const LANES: usize = super::super::ops::AVX512F_MINIMUM_WIDTH as usize;
const MAX_SIZE: u32 = super::super::ops::MAX_RANK_WINDOW;
const MAX_AREA: usize = (MAX_SIZE * MAX_SIZE) as usize;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    size: u32,
    rank: u32,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    validate(input, size, rank, selection)?;
    let area = (size as usize) * (size as usize);
    if area > 1
        && super::super::helpers::native_uniform_f_mode_image(
            input.as_raw(),
            input.width() as usize,
            input.height() as usize,
        )
    {
        crate::compute::record_pipeline_operation_path("native-identity");
        return Ok(DynamicImage::ImageF32(input.clone()));
    }

    let width = input.width() as usize;
    let height = input.height() as usize;
    let mut output = CheckedDims::new(input.width(), input.height(), 4)?.alloc_buffer();
    for y in 0..height {
        // SAFETY: preparation selected AVX-512F, and validate checks the exact F-mode layout and
        // signed gather-index bound before this target-feature function is entered.
        unsafe {
            filter_row(
                input.as_raw(),
                &mut output,
                width,
                height,
                size as usize,
                rank as usize,
                y,
            )
        };
    }

    crate::compute::record_pipeline_operation_path("avx512f");
    crate::compute::record_pipeline_operation_vector_blocks(
        width.div_ceil(LANES) as u64 * height as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let output = RgbaImage::from_raw(input.width(), input.height(), output)
        .ok_or_else(|| PilError::InternalError("F AVX-512F rank output shape mismatch".into()))?;
    Ok(DynamicImage::ImageF32(output))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn validate(
    input: &RgbaImage,
    size: u32,
    rank: u32,
    selection: &StrategySelection,
) -> Result<(), PilError> {
    let mode = selection.implementation.mode;
    let area = u64::from(size) * u64::from(size);
    let dimensions = CheckedDims::new(input.width(), input.height(), 4)?;
    let pixels = u64::from(input.width()) * u64::from(input.height());
    if selection.implementation.backend != Backend::Cpu
        || selection.requested_backend != Backend::Cpu
        || selection.requested_architecture != Architecture::Avx512F
        || selection.implementation.architecture != Architecture::Avx512F
        || mode != Mode::ImageF32
        || selection.guard
            != (crate::compute::registry::support::StrategyGuard::FilterF32Avx512RankWindow {
                max_size: MAX_SIZE,
                minimum_width: super::super::ops::AVX512F_MINIMUM_WIDTH,
                max_pixels: super::super::ops::AVX512F_MAX_PIXELS,
            })
        || size == 0
        || size.is_multiple_of(2)
        || size > MAX_SIZE
        || u64::from(rank) >= area
        || input.width() < super::super::ops::AVX512F_MINIMUM_WIDTH
        || input.height() == 0
        || pixels > super::super::ops::AVX512F_MAX_PIXELS
        || input.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "F AVX-512F rank received an ineligible strategy request".into(),
        ));
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn filter_row(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    size: usize,
    rank: usize,
    y: usize,
) {
    let area = size * size;
    let half = size / 2;
    let mut x = 0usize;
    while x < width {
        let active = (width - x).min(LANES);
        let mut values = [{ arch::_mm512_setzero_ps() }; MAX_AREA];
        let mut value_index = 0usize;
        for row_offset in 0..size {
            for column_offset in 0..size {
                values[value_index] = unsafe {
                    gather_window(raw, width, height, x, y, half, row_offset, column_offset)
                };
                value_index += 1;
            }
        }
        let selected = if rank == 0 {
            unsafe { select_extreme(&values[..area], true) }
        } else if rank + 1 == area {
            unsafe { select_extreme(&values[..area], false) }
        } else {
            unsafe { sort_vectors(&mut values[..area]) };
            values[rank]
        };
        unsafe { store_row(output, width, x, y, active, selected) };
        x += active;
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn gather_window(
    raw: &[u8],
    width: usize,
    height: usize,
    x_start: usize,
    y: usize,
    half: usize,
    row_offset: usize,
    column_offset: usize,
) -> arch::__m512 {
    let source_y = y
        .saturating_add(row_offset)
        .saturating_sub(half)
        .min(height - 1);
    let mut indices = [0i32; LANES];
    for (lane, index) in indices.iter_mut().enumerate() {
        let pixel_x = x_start.saturating_add(lane).min(width - 1);
        let source_x = pixel_x
            .saturating_add(column_offset)
            .saturating_sub(half)
            .min(width - 1);
        *index = (source_y * width + source_x) as i32;
    }
    // SAFETY: indices name valid, little-endian F-mode samples; the caller checked storage and
    // signed 32-bit gather bounds, and AVX-512F permits unaligned gather loads.
    unsafe {
        let index_vector = arch::_mm512_loadu_si512(indices.as_ptr().cast());
        arch::_mm512_i32gather_ps::<4>(index_vector, raw.as_ptr().cast())
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn sort_vectors(values: &mut [arch::__m512]) {
    for pass in 0..values.len() {
        let mut index = pass & 1;
        while index + 1 < values.len() {
            let left = values[index];
            let right = values[index + 1];
            // SAFETY: this helper is entered only from the AVX-512F rank leaf.
            {
                values[index] = arch::_mm512_min_ps(left, right);
                values[index + 1] = arch::_mm512_max_ps(left, right);
            }
            index += 2;
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn select_extreme(values: &[arch::__m512], minimum: bool) -> arch::__m512 {
    let mut selected = values[0];
    for value in &values[1..] {
        // SAFETY: this helper is entered only from the AVX-512F rank leaf.
        selected = {
            if minimum {
                arch::_mm512_min_ps(selected, *value)
            } else {
                arch::_mm512_max_ps(selected, *value)
            }
        };
    }
    selected
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn store_row(
    output: &mut [u8],
    width: usize,
    x_start: usize,
    y: usize,
    active: usize,
    values: arch::__m512,
) {
    let pixel_offset = y * width + x_start;
    let mask = ((1u32 << active) - 1) as arch::__mmask16;
    // SAFETY: the mask stores only active pixels in the checked output row.
    unsafe {
        arch::_mm512_mask_storeu_ps(
            output.as_mut_ptr().add(pixel_offset * 4).cast(),
            mask,
            values,
        )
    };
}
