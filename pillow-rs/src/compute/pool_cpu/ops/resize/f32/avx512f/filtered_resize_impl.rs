//! AVX-512F eight-output F32 filtered Resize strategy.

#![allow(unsafe_code)] // The prepared exact-mode strategy guards AVX-512F leaves.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::resize::helpers::coefficients;
use crate::compute::pool_cpu::ops::resize::helpers::strategy::require_architecture;
use crate::compute::registry::support::{OperationId, StrategyGuard, StrategySelection};
use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::error::PilError;
use crate::ops::pil_resize::{FilterCoeffsF64, PortableSeparateMulAdd, f32_samples_from_le_bytes};
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

const LANES: usize = 8;
const GATHER_LANES: usize = 16;

// TODO: verify direct AVX-512F execution on capable hardware; QEMU TCG here omits AVX-512F.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    context: &ExecutionContext,
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
    selection: &StrategySelection,
) -> Result<RgbaImage, PilError> {
    let architecture = require_architecture(context, selection)?;
    if architecture != Architecture::Avx512F
        || selection.implementation.operation != OperationId::Resize
        || selection.implementation.mode != crate::raster::Mode::ImageF32
        || selection.guard != StrategyGuard::ResizeFilteredAvx512F
        || context.backend() != Backend::Cpu
        || bounds.is_some()
        || matches!(filter, ResampleFilter::Nearest)
    {
        return Err(PilError::InternalError(
            "ImageF32 AVX-512F filtered Resize received an ineligible request".into(),
        ));
    }

    let source_size = input.dimensions();
    let source_dims = CheckedDims::new_allow_empty(source_size.0, source_size.1, 4)?;
    let output_dims = CheckedDims::new_allow_empty(width, height, 4)?;
    if input.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "ImageF32 AVX-512F Resize received invalid source storage".into(),
        ));
    }
    let samples = f32_samples_from_le_bytes(input.as_raw(), source_dims.total_pixels());
    if samples.len() != source_dims.total_pixels() {
        return Err(PilError::InternalError(
            "ImageF32 AVX-512F Resize sample count mismatch".into(),
        ));
    }
    if !samples.iter().all(|sample| sample.is_finite()) || constant_sample_shortcut(&samples) {
        return fallback(input, width, height, filter, bounds);
    }

    let result = resize_horizontal_first(&samples, source_size, (width, height), filter)?;
    let Some((output, vector_blocks, scalar_tail)) = result else {
        return fallback(input, width, height, filter, bounds);
    };
    crate::compute::record_pipeline_operation_path(if vector_blocks == 0 {
        "default-fallback"
    } else {
        "avx512f"
    });
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    encode(width, height, &output, output_dims)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn resize_horizontal_first(
    samples: &[f32],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
) -> Result<Option<(Vec<f32>, u64, u64)>, PilError> {
    let (kernel, support) = super::super::x86_64::filters::filter_from_resample(filter);
    let (horizontal, vertical) =
        coefficients::for_resize_with_kernel(source_size, destination_size, kernel, support, None);
    let (need_horizontal, need_vertical) =
        coefficients::resample_pass_requirements(source_size, destination_size, None);
    let mut blocks = 0;
    let mut tail = 0;
    let intermediate = if need_horizontal {
        validate_coefficients(&horizontal, source_size.0, destination_size.0)?;
        if !coefficients_allow_fma(&horizontal) {
            return Ok(None);
        }
        let (intermediate, pass_blocks, pass_tail) = horizontal_pass(
            samples,
            source_size.0,
            source_size.1,
            destination_size.0,
            &horizontal,
        )?;
        blocks += pass_blocks;
        tail += pass_tail;
        intermediate
    } else {
        samples.to_vec()
    };
    if !need_vertical {
        return Ok(Some((intermediate, blocks, tail)));
    }
    validate_coefficients(&vertical, source_size.1, destination_size.1)?;
    let (output, pass_blocks, pass_tail) = if intermediate.iter().all(|sample| sample.is_finite())
        && coefficients_allow_fma(&vertical)
    {
        vertical_pass(
            &intermediate,
            destination_size.0,
            source_size.1,
            destination_size.1,
            &vertical,
        )?
    } else {
        let output_dims = CheckedDims::new_allow_empty(destination_size.0, destination_size.1, 1)?;
        let mut output = vec![0.0; output_dims.total_pixels()];
        super::super::helpers::filtered::vertical_pass::<false, _>(
            &intermediate,
            &mut output,
            destination_size.0 as usize,
            destination_size.1 as usize,
            &vertical,
            &PortableSeparateMulAdd,
        );
        (output, 0, output_dims.total_pixels() as u64)
    };
    blocks += pass_blocks;
    tail += pass_tail;
    Ok(Some((output, blocks, tail)))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn horizontal_pass(
    source: &[f32],
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    coefficients: &FilterCoeffsF64,
) -> Result<(Vec<f32>, u64, u64), PilError> {
    let source_width = usize::try_from(source_width)
        .map_err(|_| PilError::ValueError("F32 Resize source width overflow".into()))?;
    let source_height = usize::try_from(source_height)
        .map_err(|_| PilError::ValueError("F32 Resize source height overflow".into()))?;
    let destination_width = usize::try_from(destination_width)
        .map_err(|_| PilError::ValueError("F32 Resize output width overflow".into()))?;
    if source.len()
        != source_width
            .checked_mul(source_height)
            .unwrap_or(usize::MAX)
    {
        return Err(PilError::InternalError(
            "AVX-512F horizontal Resize source shape mismatch".into(),
        ));
    }
    let output_dims =
        CheckedDims::new_allow_empty(destination_width as u32, source_height as u32, 1)?;
    let mut output = vec![0.0; output_dims.total_pixels()];
    let mut blocks = 0u64;
    for y in 0..source_height {
        let source_row = &source[y * source_width..(y + 1) * source_width];
        let output_row = &mut output[y * destination_width..(y + 1) * destination_width];
        for x in (0..destination_width).step_by(LANES) {
            let active = (destination_width - x).min(LANES);
            // SAFETY: coefficient validation proves every active source index
            // lies in this row; source and output slices cover each block.
            let values = unsafe { horizontal_block(source_row.as_ptr(), coefficients, x, active) };
            output_row[x..x + active].copy_from_slice(&values[..active]);
            blocks += 1;
        }
    }
    Ok((output, blocks, 0))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn vertical_pass(
    source: &[f32],
    width: u32,
    source_height: u32,
    destination_height: u32,
    coefficients: &FilterCoeffsF64,
) -> Result<(Vec<f32>, u64, u64), PilError> {
    let width = usize::try_from(width)
        .map_err(|_| PilError::ValueError("F32 Resize output width overflow".into()))?;
    let source_height = usize::try_from(source_height)
        .map_err(|_| PilError::ValueError("F32 Resize source height overflow".into()))?;
    if source.len() != width.checked_mul(source_height).unwrap_or(usize::MAX) {
        return Err(PilError::InternalError(
            "AVX-512F vertical Resize source shape mismatch".into(),
        ));
    }
    let output_dims = CheckedDims::new_allow_empty(width as u32, destination_height, 1)?;
    let mut output = vec![0.0; output_dims.total_pixels()];
    let mut blocks = 0u64;
    for y in 0..destination_height as usize {
        let output_row = &mut output[y * width..(y + 1) * width];
        for x in (0..width).step_by(LANES) {
            let active = (width - x).min(LANES);
            // SAFETY: coefficient validation proves each sampled row exists;
            // the active width and source extent were checked above.
            let values = unsafe { vertical_block(source, width, coefficients, y, x, active) };
            output_row[x..x + active].copy_from_slice(&values[..active]);
            blocks += 1;
        }
    }
    Ok((output, blocks, 0))
}

/// # Safety
/// AVX-512F must be enabled, each active coefficient range must be validated,
/// and the source row must contain every gathered sample.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn horizontal_block(
    source_row: *const f32,
    coefficients: &FilterCoeffsF64,
    output_x: usize,
    active: usize,
) -> [f32; LANES] {
    let max_taps = (0..active)
        .map(|lane| coefficients.count[output_x + lane])
        .max()
        .unwrap_or(0);
    let mut accumulator = arch::_mm512_setzero_pd();
    for tap in 0..max_taps {
        let mut indices = [0i32; GATHER_LANES];
        let mut weights = [0.0f64; LANES];
        let mut gather_mask = 0u16;
        for lane in 0..active {
            let count = coefficients.count[output_x + lane];
            if tap < count {
                let source_x = coefficients.xmin[output_x + lane] as usize + tap;
                // SAFETY: strategy guard restricts source widths to i32 indexes.
                indices[lane] = source_x as i32;
                weights[lane] = coefficients.weights[output_x + lane][tap];
                gather_mask |= 1 << lane;
            }
        }
        // SAFETY: all enabled gather indexes are in the validated row; masked
        // lanes use index zero and are initialized with a zero source value.
        let index_vector = unsafe { arch::_mm512_loadu_si512(indices.as_ptr().cast()) };
        let samples = unsafe {
            arch::_mm512_mask_i32gather_ps::<4>(
                arch::_mm512_setzero_ps(),
                gather_mask,
                index_vector,
                source_row,
            )
        };
        let samples = arch::_mm512_cvtps_pd(arch::_mm512_castps512_ps256(samples));
        let weights = unsafe { arch::_mm512_loadu_pd(weights.as_ptr()) };
        let products = std::hint::black_box(arch::_mm512_mul_pd(weights, samples));
        accumulator =
            arch::_mm512_mask_add_pd(accumulator, gather_mask as u8, products, accumulator);
    }
    let packed = arch::_mm512_cvtpd_ps(accumulator);
    let mut output = [0.0f32; LANES];
    // SAFETY: output is a writable eight-lane array and AVX-512F includes AVX.
    unsafe { arch::_mm256_storeu_ps(output.as_mut_ptr(), packed) };
    output
}

/// # Safety
/// AVX-512F must be enabled, coefficient ranges must be validated, and the
/// source must contain every active row and column sample.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn vertical_block(
    source: &[f32],
    width: usize,
    coefficients: &FilterCoeffsF64,
    output_y: usize,
    output_x: usize,
    active: usize,
) -> [f32; LANES] {
    let mut accumulator = arch::_mm512_setzero_pd();
    let first_source_y = coefficients.xmin[output_y] as usize;
    for (tap, &weight) in coefficients.weights[output_y].iter().enumerate() {
        let row_start = (first_source_y + tap) * width;
        let sample_row = &source[row_start + output_x..row_start + output_x + active];
        let mut sample_lanes = [0.0f32; LANES];
        if active == LANES {
            // SAFETY: a full active block is inside the row and aligned access
            // is unnecessary for this unaligned source address.
            let packed = unsafe { arch::_mm256_loadu_ps(sample_row.as_ptr()) };
            unsafe { arch::_mm256_storeu_ps(sample_lanes.as_mut_ptr(), packed) };
        } else {
            sample_lanes[..active].copy_from_slice(sample_row);
        }
        let samples = unsafe { arch::_mm256_loadu_ps(sample_lanes.as_ptr()) };
        let samples = arch::_mm512_cvtps_pd(samples);
        let weights = arch::_mm512_set1_pd(weight);
        let products = arch::_mm512_mul_pd(weights, samples);
        accumulator = arch::_mm512_add_pd(accumulator, products);
    }
    let packed = arch::_mm512_cvtpd_ps(accumulator);
    let mut output = [0.0f32; LANES];
    // SAFETY: output is a writable eight-lane array and AVX-512F includes AVX.
    unsafe { arch::_mm256_storeu_ps(output.as_mut_ptr(), packed) };
    output
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn validate_coefficients(
    coefficients: &FilterCoeffsF64,
    source_extent: u32,
    destination_extent: u32,
) -> Result<(), PilError> {
    let destination = usize::try_from(destination_extent)
        .map_err(|_| PilError::ValueError("F32 Resize destination extent overflow".into()))?;
    if coefficients.xmin.len() != destination
        || coefficients.count.len() != destination
        || coefficients.weights.len() != destination
    {
        return Err(PilError::InternalError(
            "AVX-512F Resize coefficient table shape mismatch".into(),
        ));
    }
    for index in 0..destination {
        let count = coefficients.count[index];
        if coefficients.weights[index].len() != count {
            return Err(PilError::InternalError(
                "AVX-512F Resize coefficient row shape mismatch".into(),
            ));
        }
        if count > 0 {
            let first = coefficients.xmin[index];
            let end = first.checked_add(count as i64);
            if first < 0 || end.is_none_or(|end| end > i64::from(source_extent)) {
                return Err(PilError::InternalError(
                    "AVX-512F Resize coefficient index is out of range".into(),
                ));
            }
        }
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn coefficients_allow_fma(coefficients: &FilterCoeffsF64) -> bool {
    crate::ops::pil_resize::f_resize_coefficients_allow_finite_fma(coefficients)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn constant_sample_shortcut(samples: &[f32]) -> bool {
    samples.first().is_some_and(|first| {
        first.is_finite()
            && first.to_bits() != (-0.0f32).to_bits()
            && samples
                .iter()
                .all(|sample| sample.to_bits() == first.to_bits())
    })
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn fallback(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> Result<RgbaImage, PilError> {
    crate::compute::record_pipeline_operation_path("default-fallback");
    super::super::helpers::resize::execute_with_fma_and_filter::<{ usize::MAX }, _>(
        input,
        width,
        height,
        filter,
        bounds,
        &PortableSeparateMulAdd,
        super::super::x86_64::filters::filter_from_resample,
    )
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn encode(
    width: u32,
    height: u32,
    samples: &[f32],
    dimensions: CheckedDims,
) -> Result<RgbaImage, PilError> {
    if samples.len() != dimensions.total_pixels() {
        return Err(PilError::InternalError(
            "AVX-512F Resize output sample count mismatch".into(),
        ));
    }
    let bytes = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("AVX-512F Resize output shape mismatch".into()))
}
