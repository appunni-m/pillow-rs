//! Shared RGBA GaussianBlur radius-seven horizontal vector kernel.

#[cfg(target_arch = "x86_64")]
use crate::checked_dims::CheckedDims;
#[cfg(target_arch = "x86_64")]
use crate::compute::Architecture;
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::{DynamicImage, RgbaImage};
use wide::u32x8;

const CHANNELS: usize = 4;
const SCALAR_BLOCK_PIXELS: usize = 8;
const RADIUS: usize = 7;
const BOX_BLUR_BIAS: u32 = crate::compute::pool_cpu::ops::filter::helpers::BOX_BLUR_BIAS;

/// Architecture-specific middle-block hook, monomorphized at the mode leaf.
pub(in crate::compute::pool_cpu::ops::filter::rgba8) trait RadiusSevenHorizontalKernel {
    const HAS_OPTIMIZED_MIDDLE_BLOCK: bool;
    const BLOCK_PIXELS: usize;

    fn blur_middle_block(
        source: &[u8],
        destination: &mut [u8],
        accumulator: &mut [u32; 4],
        whole_weight: u32,
        fractional_weight: u32,
        output_start: usize,
    );
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[cfg(target_arch = "x86_64")]
pub(in crate::compute::pool_cpu::ops::filter::rgba8) fn execute_radius_seven<
    K: RadiusSevenHorizontalKernel,
>(
    image: &DynamicImage,
    sigma: f32,
    context: &ExecutionContext,
    execution_path: &'static str,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(input) = image else {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur radius-seven strategy received a mismatched raster".into(),
        ));
    };
    if !sigma.is_finite() || sigma == 0.0 {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur radius-seven strategy received invalid sigma".into(),
        ));
    }
    let (radius, whole_weight, fractional_weight) =
        crate::compute::pool_cpu::ops::filter::helpers::gaussian_blur_box_parameters(sigma);
    if radius != 7 || fractional_weight == 0 {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur radius-seven strategy received unsupported weights".into(),
        ));
    }

    let dimensions = CheckedDims::new(input.width(), input.height(), CHANNELS as u8)?;
    if dimensions.width == 0
        || dimensions.height == 0
        || input.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur radius-seven strategy received invalid native dimensions".into(),
        ));
    }

    let (mut work, mut scratch) = blur_passes::<K>(
        input.as_raw().to_vec(),
        dimensions.alloc_buffer(),
        dimensions.width as usize,
        dimensions.height as usize,
        whole_weight,
        fractional_weight,
    );
    transpose_rgba_in_place(
        &mut work,
        &mut scratch,
        input.width(),
        input.height(),
        context,
    )?;
    let transposed_dims = CheckedDims::new(input.height(), input.width(), CHANNELS as u8)?;
    debug_assert_eq!(scratch.len(), transposed_dims.total_bytes());
    (work, scratch) = blur_passes::<K>(
        work,
        scratch,
        transposed_dims.width as usize,
        transposed_dims.height as usize,
        whole_weight,
        fractional_weight,
    );
    transpose_rgba_in_place(
        &mut work,
        &mut scratch,
        transposed_dims.width,
        transposed_dims.height,
        context,
    )?;
    let output = RgbaImage::from_raw(input.width(), input.height(), work)
        .ok_or_else(|| PilError::InternalError("RGBA GaussianBlur output shape mismatch".into()))?;
    crate::compute::record_pipeline_operation_path(execution_path);
    Ok(DynamicImage::ImageRgba8(output))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[cfg(target_arch = "x86_64")]
fn blur_passes<K: RadiusSevenHorizontalKernel>(
    mut work: Vec<u8>,
    mut scratch: Vec<u8>,
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> (Vec<u8>, Vec<u8>) {
    crate::compute::record_pipeline_operation_path("vector");
    for _ in 0..3 {
        let blocks = blur_horizontal_rows::<K>(
            &work,
            &mut scratch,
            width,
            height,
            whole_weight,
            fractional_weight,
        );
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
        std::mem::swap(&mut work, &mut scratch);
    }
    (work, scratch)
}

/// Transpose RGBA storage through the architecture strategy selected by context.
pub(in crate::compute::pool_cpu::ops::filter::rgba8) fn transpose_rgba_in_place(
    work: &mut Vec<u8>,
    _scratch: &mut Vec<u8>,
    width: u32,
    height: u32,
    context: &ExecutionContext,
) -> Result<(), PilError> {
    let image = RgbaImage::from_raw(width, height, std::mem::take(work)).ok_or_else(|| {
        PilError::InternalError("RGBA GaussianBlur transpose input shape mismatch".into())
    })?;
    let source = DynamicImage::ImageRgba8(image);
    let strategy = crate::compute::pool_cpu::ops::transpose::ops::prepare_transpose(
        &source,
        &TransposeMethod::Transpose,
        context,
    )?;
    #[cfg(target_arch = "x86_64")]
    if strategy.selection().implementation.architecture == Architecture::Avx2 {
        strategy.execute_rgba_avx2_into(_scratch)?;
        let DynamicImage::ImageRgba8(input) = source else {
            return Err(PilError::InternalError(
                "RGBA GaussianBlur transpose source changed mode".into(),
            ));
        };
        *work = input.into_raw();
        std::mem::swap(work, _scratch);
        return Ok(());
    }

    let DynamicImage::ImageRgba8(transposed) = strategy.execute()? else {
        return Err(PilError::InternalError(
            "RGBA GaussianBlur transpose changed mode".into(),
        ));
    };
    *work = transposed.into_raw();
    Ok(())
}

#[derive(Clone, Copy)]
enum BlurRegion {
    Leading,
    Middle,
    Trailing,
    Clamped,
}

#[derive(Clone, Copy)]
struct HorizontalBlurContext {
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
}

/// Blur RGBA rows with each strategy's fixed-point vector block width.
pub(in crate::compute::pool_cpu::ops::filter::rgba8) fn blur_horizontal_rows<
    K: RadiusSevenHorizontalKernel,
>(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> u64 {
    let row_stride = width * CHANNELS;
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert_ne!(fractional_weight, 0);
    let mut vector_blocks = 0u64;
    for row in 0..height {
        let row_start = row * row_stride;
        vector_blocks = vector_blocks.saturating_add(blur_horizontal_line::<K>(
            &source[row_start..row_start + row_stride],
            &mut destination[row_start..row_start + row_stride],
            width,
            whole_weight,
            fractional_weight,
        ));
    }
    vector_blocks
}

fn blur_horizontal_line<K: RadiusSevenHorizontalKernel>(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> u64 {
    debug_assert!(width > 0);
    let last = width - 1;
    let edge_a = (RADIUS + 1).min(width);
    let edge_b = width.saturating_sub(RADIUS + 1);
    let context = HorizontalBlurContext {
        width,
        whole_weight,
        fractional_weight,
    };
    let mut accumulator = initial_horizontal_sums(source, width, edge_a, last);
    let mut vector_blocks = 0;
    if edge_a <= edge_b {
        vector_blocks += blur_horizontal_region::<K>(
            source,
            destination,
            &mut accumulator,
            context,
            0,
            edge_a,
            BlurRegion::Leading,
        );
        vector_blocks += blur_horizontal_region::<K>(
            source,
            destination,
            &mut accumulator,
            context,
            edge_a,
            edge_b,
            BlurRegion::Middle,
        );
        vector_blocks += blur_horizontal_region::<K>(
            source,
            destination,
            &mut accumulator,
            context,
            edge_b,
            width,
            BlurRegion::Trailing,
        );
    } else {
        vector_blocks += blur_horizontal_region::<K>(
            source,
            destination,
            &mut accumulator,
            context,
            0,
            edge_b,
            BlurRegion::Leading,
        );
        vector_blocks += blur_horizontal_region::<K>(
            source,
            destination,
            &mut accumulator,
            context,
            edge_b,
            edge_a,
            BlurRegion::Clamped,
        );
        vector_blocks += blur_horizontal_region::<K>(
            source,
            destination,
            &mut accumulator,
            context,
            edge_a,
            width,
            BlurRegion::Trailing,
        );
    }
    vector_blocks
}

fn initial_horizontal_sums(
    source: &[u8],
    width: usize,
    edge_a: usize,
    last: usize,
) -> [u32; CHANNELS] {
    let mut sums = [0u32; CHANNELS];
    let last_count = RADIUS + 1 - edge_a;
    for channel in 0..CHANNELS {
        sums[channel] = u32::from(source[channel]) * (RADIUS + 1) as u32;
        for position in 0..edge_a.saturating_sub(1) {
            sums[channel] =
                sums[channel].wrapping_add(u32::from(source[position * CHANNELS + channel]));
        }
        sums[channel] = sums[channel]
            .wrapping_add(u32::from(source[last * CHANNELS + channel]) * last_count as u32);
    }
    debug_assert_eq!(source.len(), width * CHANNELS);
    sums
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[inline(always)]
fn blur_horizontal_region<K: RadiusSevenHorizontalKernel>(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; CHANNELS],
    context: HorizontalBlurContext,
    start: usize,
    end: usize,
    region: BlurRegion,
) -> u64 {
    let mut output = start;
    let mut vector_blocks = 0u64;
    while output < end {
        let output_count = (end - output).min(K::BLOCK_PIXELS);
        let optimized_middle = K::HAS_OPTIMIZED_MIDDLE_BLOCK
            && matches!(region, BlurRegion::Middle)
            && output_count == K::BLOCK_PIXELS
            && output > RADIUS
            && output + K::BLOCK_PIXELS + RADIUS < context.width;
        if optimized_middle {
            K::blur_middle_block(
                source,
                destination,
                accumulator,
                context.whole_weight,
                context.fractional_weight,
                output,
            );
            vector_blocks += 1;
        } else {
            let mut fallback_start = output;
            let mut remaining = output_count;
            while remaining != 0 {
                let fallback_count = remaining.min(SCALAR_BLOCK_PIXELS);
                blur_horizontal_block(
                    source,
                    destination,
                    accumulator,
                    context,
                    fallback_start,
                    fallback_count,
                    region,
                );
                fallback_start += fallback_count;
                remaining -= fallback_count;
                vector_blocks += 1;
            }
        }
        output += output_count;
    }
    vector_blocks
}

#[inline(always)]
fn blur_horizontal_block(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; CHANNELS],
    context: HorizontalBlurContext,
    output_start: usize,
    output_count: usize,
    region: BlurRegion,
) {
    let last = context.width - 1;
    let mut accumulated = [[0u32; SCALAR_BLOCK_PIXELS]; CHANNELS];
    let mut fractional = [[0u32; SCALAR_BLOCK_PIXELS]; CHANNELS];
    for lane in 0..output_count {
        let output = output_start + lane;
        let (subtract, add, far_left, far_right) = horizontal_sample_indices(output, last, region);
        for channel in 0..CHANNELS {
            let leaving = u32::from(source[subtract * CHANNELS + channel]);
            let entering = u32::from(source[add * CHANNELS + channel]);
            accumulator[channel] = accumulator[channel]
                .wrapping_sub(leaving)
                .wrapping_add(entering);
            accumulated[channel][lane] = accumulator[channel];
            fractional[channel][lane] = (u32::from(source[far_left * CHANNELS + channel])
                + u32::from(source[far_right * CHANNELS + channel]))
            .wrapping_mul(context.fractional_weight);
        }
    }
    store_horizontal_block(
        destination,
        output_start,
        output_count,
        context.whole_weight,
        accumulated,
        fractional,
    );
}

#[inline(always)]
fn horizontal_sample_indices(
    output: usize,
    last: usize,
    region: BlurRegion,
) -> (usize, usize, usize, usize) {
    match region {
        BlurRegion::Leading => (0, output + RADIUS, 0, output + RADIUS + 1),
        BlurRegion::Middle => (
            output - RADIUS - 1,
            output + RADIUS,
            output - RADIUS - 1,
            output + RADIUS + 1,
        ),
        BlurRegion::Trailing => (output - RADIUS - 1, last, output - RADIUS - 1, last),
        BlurRegion::Clamped => (0, last, 0, last),
    }
}

#[inline(always)]
fn store_horizontal_block(
    destination: &mut [u8],
    output_start: usize,
    output_count: usize,
    whole_weight: u32,
    accumulated: [[u32; SCALAR_BLOCK_PIXELS]; CHANNELS],
    fractional: [[u32; SCALAR_BLOCK_PIXELS]; CHANNELS],
) {
    for channel in 0..CHANNELS {
        let bulk = u32x8::new(accumulated[channel]) * u32x8::splat(whole_weight)
            + u32x8::new(fractional[channel]);
        let packed = ((bulk + u32x8::splat(BOX_BLUR_BIAS)) >> 24u32).to_array();
        for lane in 0..output_count {
            destination[(output_start + lane) * CHANNELS + channel] = packed[lane] as u8;
        }
    }
}
