//! Sixteen-lane AVX-512F I-mode convolution strategies.
// TODO(pillow-rs#fvsp): verify Filter3x3/Filter5x5 selected and fallback
// receipts, exact Pillow output, and the frozen AVX-512F workloads on hardware.

#![allow(unsafe_code)] // The prepared AVX-512F strategy guards the target-feature leaves.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::filter::ops::FilterRequest;
use crate::compute::registry::support::{OperationId, StrategyGuard, StrategySelection};
use crate::compute::{Architecture, Backend};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

const LANES: usize = 16;

const fn make_tap_indices() -> [[i32; LANES]; 5] {
    let mut indices = [[0; LANES]; 5];
    let mut tap = 0;
    while tap < 5 {
        let mut lane = 0;
        while lane < LANES {
            let source = tap + lane;
            indices[tap][lane] = if source < LANES {
                source as i32
            } else {
                (source + 12) as i32
            };
            lane += 1;
        }
        tap += 1;
    }
    indices
}

const TAP_INDICES: [[i32; LANES]; 5] = make_tap_indices();

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &RgbaImage,
    request: FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match request {
        FilterRequest::Filter3x3 {
            kernel,
            scale,
            offset,
        } => execute_3x3(image, &kernel, scale, offset, selection),
        FilterRequest::Filter5x5 {
            kernel,
            scale,
            offset,
        } => execute_5x5(image, &kernel, scale, offset, selection),
        _ => Err(PilError::InternalError(
            "I AVX-512F convolution received a non-convolution request".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn execute_3x3(
    input: &RgbaImage,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    validate(
        input,
        OperationId::Filter3x3,
        StrategyGuard::Filter3x3I32Avx512F,
        18,
        3,
        selection,
    )?;
    let kernel = std::array::from_fn(|index| kernel[index] / scale);
    let rounding_bias = offset + 0.5;
    let width = input.width() as usize;
    let height = input.height() as usize;
    let mut output = input.as_raw().to_vec();
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 1..height - 1 {
        let mut x = 1usize;
        while x + LANES <= width - 1 {
            // SAFETY: the selected guard guarantees a complete I32 interior block and AVX-512F.
            unsafe {
                filter_3x3_block(
                    input.as_raw(),
                    &mut output,
                    width,
                    y,
                    x,
                    &kernel,
                    rounding_bias,
                )
            };
            vector_blocks += 1;
            x += LANES;
        }
        while x < width - 1 {
            let value = super::super::helpers::filter_i32_3x3_scalar(
                input.as_raw(),
                width,
                y,
                x,
                &kernel,
                rounding_bias,
            );
            write_sample(&mut output, y * width + x, value);
            scalar_tail += 1;
            x += 1;
        }
    }
    record_execution(vector_blocks, scalar_tail);
    image_result(input, output)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn execute_5x5(
    input: &RgbaImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    validate(
        input,
        OperationId::Filter5x5,
        StrategyGuard::Filter5x5I32Avx512F,
        20,
        5,
        selection,
    )?;
    let kernel = std::array::from_fn(|index| kernel[index] / scale);
    let rounding_bias = offset + 0.5;
    let width = input.width() as usize;
    let height = input.height() as usize;
    if super::super::default::filter5x5_impl::uniform_i32_filter5x5_is_identity(
        input.as_raw(),
        width,
        height,
        &kernel,
        rounding_bias,
    ) {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(DynamicImage::ImageI32(input.clone()));
    }

    let mut output = input.as_raw().to_vec();
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 2..height - 2 {
        let mut x = 2usize;
        while x + LANES <= width - 2 {
            // SAFETY: the selected guard guarantees all five rows and sixteen I32 lanes.
            unsafe {
                filter_5x5_block(
                    input.as_raw(),
                    &mut output,
                    width,
                    y,
                    x,
                    &kernel,
                    rounding_bias,
                )
            };
            vector_blocks += 1;
            x += LANES;
        }
        while x < width - 2 {
            let value = super::super::helpers::filter_i32_5x5_scalar(
                input.as_raw(),
                width,
                y,
                x,
                &kernel,
                rounding_bias,
            );
            write_sample(&mut output, y * width + x, value);
            scalar_tail += 1;
            x += 1;
        }
    }
    record_execution(vector_blocks, scalar_tail);
    image_result(input, output)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn validate(
    input: &RgbaImage,
    operation: OperationId,
    guard: StrategyGuard,
    minimum_width: u32,
    minimum_height: u32,
    selection: &StrategySelection,
) -> Result<(), PilError> {
    let dimensions = CheckedDims::new(input.width(), input.height(), 4)?;
    if selection.implementation.backend != Backend::Cpu
        || selection.requested_backend != Backend::Cpu
        || selection.requested_architecture != Architecture::Avx512F
        || selection.implementation.architecture != Architecture::Avx512F
        || selection.implementation.operation != operation
        || selection.implementation.mode != Mode::ImageI32
        || selection.guard != guard
        || input.width() < minimum_width
        || input.height() < minimum_height
        || input.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "I AVX-512F convolution received an ineligible strategy request".into(),
        ));
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn record_execution(vector_blocks: u64, scalar_tail: u64) {
    crate::compute::record_pipeline_operation_path("avx512f");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn image_result(input: &RgbaImage, output: Vec<u8>) -> Result<DynamicImage, PilError> {
    let output = RgbaImage::from_raw(input.width(), input.height(), output)
        .ok_or_else(|| PilError::InternalError("I AVX-512F filter output shape mismatch".into()))?;
    Ok(DynamicImage::ImageI32(output))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn write_sample(output: &mut [u8], pixel: usize, value: i32) {
    let start = pixel * 4;
    output[start..start + 4].copy_from_slice(&value.to_le_bytes());
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn load_i32x16(raw: &[u8], first_pixel: usize) -> arch::__m512 {
    let start = first_pixel * 4;
    let pointer = unsafe { raw.as_ptr().add(start).cast::<arch::__m512i>() };
    let samples = unsafe { arch::_mm512_loadu_si512(pointer) };
    arch::_mm512_cvtepi32_ps(samples)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn filter_3x3_block(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    y: usize,
    x: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) {
    let row = |dy: isize, kernel_start: usize| {
        let source_y = (y as isize + dy) as usize;
        let left = unsafe { load_i32x16(raw, source_y * width + x - 1) };
        let middle = unsafe { load_i32x16(raw, source_y * width + x) };
        let right = unsafe { load_i32x16(raw, source_y * width + x + 1) };
        let sum = { arch::_mm512_mul_ps(middle, arch::_mm512_set1_ps(kernel[kernel_start + 1])) };
        let sum = { arch::_mm512_fmadd_ps(left, arch::_mm512_set1_ps(kernel[kernel_start]), sum) };
        arch::_mm512_fmadd_ps(right, arch::_mm512_set1_ps(kernel[kernel_start + 2]), sum)
    };
    let mut total = { arch::_mm512_set1_ps(rounding_bias) };
    total = arch::_mm512_add_ps(total, row(1, 0));
    total = arch::_mm512_add_ps(total, row(0, 3));
    total = arch::_mm512_add_ps(total, row(-1, 6));
    unsafe { store_i32x16(output, y * width + x, truncate_nonnegative(total)) };
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn load_tap_indices(tap: usize) -> arch::__m512i {
    unsafe { arch::_mm512_loadu_si512(TAP_INDICES[tap].as_ptr().cast()) }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn filter_5x5_row(
    raw: &[u8],
    width: usize,
    source_y: usize,
    x: usize,
    kernel: &[f32; 25],
    kernel_start: usize,
) -> arch::__m512 {
    let first_pixel = source_y * width + x - 2;
    let first_start = first_pixel * 4;
    let second_start = (first_pixel + 4) * 4;
    let first_pointer = unsafe { raw.as_ptr().add(first_start).cast::<arch::__m512i>() };
    let second_pointer = unsafe { raw.as_ptr().add(second_start).cast::<arch::__m512i>() };
    let first = unsafe { arch::_mm512_loadu_si512(first_pointer) };
    let second = unsafe { arch::_mm512_loadu_si512(second_pointer) };
    let tap = |index: usize| unsafe {
        arch::_mm512_cvtepi32_ps(arch::_mm512_permutex2var_epi32(
            first,
            load_tap_indices(index),
            second,
        ))
    };
    let sum = { arch::_mm512_mul_ps(tap(1), arch::_mm512_set1_ps(kernel[kernel_start + 1])) };
    let sum = { arch::_mm512_fmadd_ps(tap(0), arch::_mm512_set1_ps(kernel[kernel_start]), sum) };
    let sum =
        { arch::_mm512_fmadd_ps(tap(2), arch::_mm512_set1_ps(kernel[kernel_start + 2]), sum) };
    let sum =
        { arch::_mm512_fmadd_ps(tap(3), arch::_mm512_set1_ps(kernel[kernel_start + 3]), sum) };
    arch::_mm512_fmadd_ps(tap(4), arch::_mm512_set1_ps(kernel[kernel_start + 4]), sum)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn filter_5x5_block(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    y: usize,
    x: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    let mut total = { arch::_mm512_set1_ps(rounding_bias) };
    total = unsafe { arch::_mm512_add_ps(total, filter_5x5_row(raw, width, y + 2, x, kernel, 0)) };
    total = unsafe { arch::_mm512_add_ps(total, filter_5x5_row(raw, width, y + 1, x, kernel, 5)) };
    total = unsafe { arch::_mm512_add_ps(total, filter_5x5_row(raw, width, y, x, kernel, 10)) };
    total = unsafe { arch::_mm512_add_ps(total, filter_5x5_row(raw, width, y - 1, x, kernel, 15)) };
    total = unsafe { arch::_mm512_add_ps(total, filter_5x5_row(raw, width, y - 2, x, kernel, 20)) };
    unsafe { store_i32x16(output, y * width + x, truncate_nonnegative(total)) };
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn truncate_nonnegative(value: arch::__m512) -> arch::__m512i {
    let zero = { arch::_mm512_setzero_ps() };
    let nonnegative = { arch::_mm512_cmp_ps_mask(value, zero, arch::_CMP_GE_OQ) };
    let value = { arch::_mm512_maskz_mov_ps(nonnegative, value) };
    let converted = { arch::_mm512_cvttps_epi32(value) };
    let upper_bound = { arch::_mm512_set1_ps(2_147_483_648.0) };
    let overflow = { arch::_mm512_cmp_ps_mask(value, upper_bound, arch::_CMP_GE_OQ) };
    arch::_mm512_mask_mov_epi32(converted, overflow, arch::_mm512_set1_epi32(i32::MAX))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn store_i32x16(output: &mut [u8], first_pixel: usize, values: arch::__m512i) {
    let start = first_pixel * 4;
    let pointer = unsafe { output.as_mut_ptr().add(start).cast::<arch::__m512i>() };
    unsafe { arch::_mm512_storeu_si512(pointer, values) };
}
