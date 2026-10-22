//! Shared BoxBlur rolling-line and transpose kernels.

#[cfg(feature = "parallel")]
use super::super::simd_rows_mut_serial;
#[cfg(all(not(feature = "parallel"), target_arch = "x86_64"))]
use super::super::transpose;
#[cfg(all(not(feature = "parallel"), target_arch = "aarch64"))]
use super::super::transpose_native_tiled;
use super::super::{
    SIMD_BOX_BLUR_BIAS, SIMD_BOX_BLUR_SCALE, simd_blur_load_u8x16, simd_blur_pack_u32x16_to_bytes,
};
use crate::checked_dims::CheckedDims;
#[cfg(all(not(feature = "parallel"), target_arch = "x86_64"))]
use crate::pipeline::TransposeMethod;
use wide::{u16x16, u32x8, u32x16};

mod luma;
#[cfg(target_arch = "x86_64")]
mod rgba_avx2;
#[cfg(target_arch = "aarch64")]
mod rgba_neon;

pub(super) use self::luma::simd_luma_box_blur_radius_one_rows;
pub(in crate::compute::pool_simd::ops::adapters) use self::luma::{
    simd_luma_horizontal_radius_one_three_passes_rows, simd_luma_vertical_radius_one_rows,
};
#[cfg(target_arch = "aarch64")]
pub(super) use self::rgba_neon::blur_vertical_rows_radius_seven;

#[derive(Clone, Copy)]
enum SimdBlurRegion {
    Leading,
    Middle,
    Trailing,
    Clamped,
}

#[inline]
fn simd_blur_work(line_length: usize, radius: usize) -> (u64, u64) {
    let edge_a = (radius + 1).min(line_length);
    let edge_b = line_length.saturating_sub(radius + 1);
    let mut vector_blocks = 0u64;
    let scalar_tail = 0u64;
    let mut count_region = |start: usize, end: usize| {
        let length = end.saturating_sub(start);
        // `simd_blur_block` keeps the exact scalar recurrence but performs
        // the independent fixed-point pack for every group of up to eight
        // outputs in `u32x8`. Partial groups therefore still use the vector
        // arithmetic path; they are not scalar tails.
        vector_blocks = vector_blocks.saturating_add(length.div_ceil(8) as u64);
    };
    if edge_a <= edge_b {
        count_region(0, edge_a);
        count_region(edge_a, edge_b);
        count_region(edge_b, line_length);
    } else {
        count_region(0, edge_b);
        count_region(edge_b, edge_a);
        count_region(edge_a, line_length);
    }
    (vector_blocks, scalar_tail)
}

#[inline(always)]
fn simd_blur_block_no_fractional(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; 4],
    element_width: usize,
    radius: usize,
    whole_weight: u32,
    last: usize,
    output_start: usize,
    output_count: usize,
    region: SimdBlurRegion,
) {
    debug_assert!(output_count <= 8);
    debug_assert!(element_width <= accumulator.len());

    let mut accumulated = [[0u32; 8]; 4];
    for lane in 0..output_count {
        let output = output_start + lane;
        let (subtract, add) = match region {
            SimdBlurRegion::Leading => (0, output + radius),
            SimdBlurRegion::Middle => (output - radius - 1, output + radius),
            SimdBlurRegion::Trailing => (output - radius - 1, last),
            SimdBlurRegion::Clamped => (0, last),
        };
        let subtract_base = subtract * element_width;
        let add_base = add * element_width;
        for component in 0..element_width {
            accumulator[component] = accumulator[component]
                .wrapping_sub(source[subtract_base + component] as u32)
                .wrapping_add(source[add_base + component] as u32);
            accumulated[component][lane] = accumulator[component];
        }
    }

    for component in 0..element_width {
        let bulk = u32x8::new(accumulated[component]) * u32x8::splat(whole_weight);
        let values = (bulk + u32x8::splat(SIMD_BOX_BLUR_BIAS) >> 24u32).to_array();
        for lane in 0..output_count {
            destination[(output_start + lane) * element_width + component] = values[lane] as u8;
        }
    }
}

#[inline(always)]
#[allow(unsafe_code)]
fn simd_blur_block<const GAUSSIAN_RGBA_SPECIALIZATION: bool, const GAUSSIAN_RGBA_AVX2: bool>(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; 4],
    element_width: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
    last: usize,
    output_start: usize,
    output_count: usize,
    region: SimdBlurRegion,
) {
    debug_assert!(output_count <= 8);
    debug_assert!(element_width <= accumulator.len());

    #[cfg(target_arch = "aarch64")]
    if GAUSSIAN_RGBA_SPECIALIZATION
        && element_width == 4
        && radius == 7
        && fractional_weight != 0
        && output_count == 8
        && matches!(region, SimdBlurRegion::Middle)
    {
        rgba_neon::blur_middle_block(
            source,
            destination,
            accumulator,
            whole_weight,
            fractional_weight,
            output_start,
        );
        return;
    }

    #[cfg(target_arch = "x86_64")]
    if GAUSSIAN_RGBA_SPECIALIZATION
        && GAUSSIAN_RGBA_AVX2
        && element_width == 4
        && radius == 7
        && fractional_weight != 0
        && output_count == 8
        && matches!(region, SimdBlurRegion::Middle)
    {
        // SAFETY: GaussianBlur selects this specialization only after the
        // runtime AVX2 feature check in `simd_pil_box_blur_xy`.
        unsafe {
            rgba_avx2::blur_middle_block(
                source,
                destination,
                accumulator,
                whole_weight,
                fractional_weight,
                output_start,
            );
        }
        return;
    }

    if fractional_weight == 0 {
        simd_blur_block_no_fractional(
            source,
            destination,
            accumulator,
            element_width,
            radius,
            whole_weight,
            last,
            output_start,
            output_count,
            region,
        );
        return;
    }

    // The rolling recurrence remains scalar because each output accumulator
    // depends on the preceding output.  Its fixed-point multiply, fractional
    // edge term, bias, and pack are independent across eight output pixels,
    // so keep that exact recurrence and vectorize the arithmetic tail. This
    // avoids the radius-sized per-pixel loop without changing Pillow's u32
    // wrapping behavior or its 24-bit rounding boundary.
    let mut accumulated = [[0u32; 8]; 4];
    let mut fractional = [[0u32; 8]; 4];
    for lane in 0..output_count {
        let output = output_start + lane;
        let (subtract, add, far_left, far_right) = match region {
            SimdBlurRegion::Leading => (0, output + radius, 0, output + radius + 1),
            SimdBlurRegion::Middle => (
                output - radius - 1,
                output + radius,
                output - radius - 1,
                output + radius + 1,
            ),
            SimdBlurRegion::Trailing => (output - radius - 1, last, output - radius - 1, last),
            SimdBlurRegion::Clamped => (0, last, 0, last),
        };
        let subtract_base = subtract * element_width;
        let add_base = add * element_width;
        let far_left_base = far_left * element_width;
        let far_right_base = far_right * element_width;
        for component in 0..element_width {
            accumulator[component] = accumulator[component]
                .wrapping_sub(source[subtract_base + component] as u32)
                .wrapping_add(source[add_base + component] as u32);
            accumulated[component][lane] = accumulator[component];
            fractional[component][lane] = (source[far_left_base + component] as u32
                + source[far_right_base + component] as u32)
                .wrapping_mul(fractional_weight);
        }
    }

    for component in 0..element_width {
        let bulk = u32x8::new(accumulated[component]) * u32x8::splat(whole_weight)
            + u32x8::new(fractional[component]);
        let values = (bulk + u32x8::splat(SIMD_BOX_BLUR_BIAS) >> 24u32).to_array();
        for lane in 0..output_count {
            destination[(output_start + lane) * element_width + component] = values[lane] as u8;
        }
    }
}

/// Blur one contiguous native-byte line with Pillow's fixed-point recurrence.
///
/// The edge-region split mirrors `src/libImaging/BoxBlur.c::ImagingLineBoxBlur`
/// exactly. SIMD only changes the independent fixed-point output arithmetic;
/// sample entry/removal and all border indices remain in the scalar order used
/// by the reference CPU implementation.
fn simd_blur_line<const GAUSSIAN_RGBA_SPECIALIZATION: bool>(
    source: &[u8],
    destination: &mut [u8],
    line_length: usize,
    element_width: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    simd_blur_line_impl::<GAUSSIAN_RGBA_SPECIALIZATION, GAUSSIAN_RGBA_SPECIALIZATION>(
        source,
        destination,
        line_length,
        element_width,
        radius,
        whole_weight,
        fractional_weight,
    );
}

fn simd_blur_line_impl<const GAUSSIAN_RGBA_SPECIALIZATION: bool, const GAUSSIAN_RGBA_AVX2: bool>(
    source: &[u8],
    destination: &mut [u8],
    line_length: usize,
    element_width: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(line_length > 0);
    debug_assert!((1..=4).contains(&element_width));
    debug_assert_eq!(source.len(), line_length * element_width);
    debug_assert_eq!(destination.len(), source.len());

    let last = line_length - 1;
    let edge_a = (radius + 1).min(line_length);
    let edge_b = line_length.saturating_sub(radius + 1);
    let mut accumulator = [0u32; 4];

    for component in 0..element_width {
        accumulator[component] = (source[component] as u32).wrapping_mul((radius + 1) as u32);
    }
    for position in 0..edge_a.saturating_sub(1) {
        let base = position * element_width;
        for component in 0..element_width {
            accumulator[component] =
                accumulator[component].wrapping_add(source[base + component] as u32);
        }
    }
    let last_count = radius.saturating_add(1).saturating_sub(edge_a);
    let last_base = last * element_width;
    for component in 0..element_width {
        accumulator[component] = accumulator[component]
            .wrapping_add((source[last_base + component] as u32).wrapping_mul(last_count as u32));
    }

    let mut apply_region = |start: usize, end: usize, region: SimdBlurRegion| {
        let mut output = start;
        while output < end {
            let output_count = (end - output).min(8);
            simd_blur_block::<GAUSSIAN_RGBA_SPECIALIZATION, GAUSSIAN_RGBA_AVX2>(
                source,
                destination,
                &mut accumulator,
                element_width,
                radius,
                whole_weight,
                fractional_weight,
                last,
                output,
                output_count,
                region,
            );
            output += output_count;
        }
    };

    if edge_a <= edge_b {
        apply_region(0, edge_a, SimdBlurRegion::Leading);
        apply_region(edge_a, edge_b, SimdBlurRegion::Middle);
        apply_region(edge_b, line_length, SimdBlurRegion::Trailing);
    } else {
        // When the radius overlaps both edges, the center region is clamped
        // to the two endpoints. Its input indices remain valid even when the
        // radius is larger than the line itself.
        apply_region(0, edge_b, SimdBlurRegion::Leading);
        apply_region(edge_b, edge_a, SimdBlurRegion::Clamped);
        apply_region(edge_a, line_length, SimdBlurRegion::Trailing);
    }
}

#[inline]
fn simd_blur_row<const GAUSSIAN_RGBA_SPECIALIZATION: bool>(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    simd_blur_line::<GAUSSIAN_RGBA_SPECIALIZATION>(
        source,
        destination,
        width,
        channels,
        radius,
        whole_weight,
        fractional_weight,
    );
}

#[inline]
fn simd_blur_rows_serial<const GAUSSIAN_RGBA_SPECIALIZATION: bool>(
    source: &[u8],
    destination: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    for row_index in 0..height {
        let row_start = row_index * row_stride;
        simd_blur_row::<GAUSSIAN_RGBA_SPECIALIZATION>(
            &source[row_start..row_start + row_stride],
            &mut destination[row_start..row_start + row_stride],
            width,
            channels,
            radius,
            whole_weight,
            fractional_weight,
        );
    }
}

pub(crate) fn simd_blur_rows<const GAUSSIAN_RGBA_SPECIALIZATION: bool>(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let dimensions = CheckedDims::new(width as u32, height as u32, channels as u8)
        .expect("native SIMD blur dimensions were validated at the adapter boundary");
    let row_stride = dimensions.row_stride();
    debug_assert_eq!(source.len(), dimensions.total_bytes());
    debug_assert_eq!(destination.len(), dimensions.total_bytes());
    let (vector_blocks_per_line, scalar_tail_per_line) = simd_blur_work(width, radius);
    let vector_blocks = vector_blocks_per_line.saturating_mul(height as u64);
    let scalar_tail = scalar_tail_per_line.saturating_mul(height as u64);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }

    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= SIMD_BLUR_PARALLEL_PIXEL_THRESHOLD {
        simd_rows_mut_serial!(
            destination,
            row_stride,
            height,
            |row_start, row_end, _y, row| {
                simd_blur_row::<GAUSSIAN_RGBA_SPECIALIZATION>(
                    &source[row_start..row_end],
                    row,
                    width,
                    channels,
                    radius,
                    whole_weight,
                    fractional_weight,
                );
            }
        );
    } else {
        simd_blur_rows_serial::<GAUSSIAN_RGBA_SPECIALIZATION>(
            source,
            destination,
            row_stride,
            width,
            height,
            channels,
            radius,
            whole_weight,
            fractional_weight,
        );
    }
    #[cfg(not(feature = "parallel"))]
    simd_blur_rows_serial::<GAUSSIAN_RGBA_SPECIALIZATION>(
        source,
        destination,
        row_stride,
        width,
        height,
        channels,
        radius,
        whole_weight,
        fractional_weight,
    );
}

#[inline]
fn simd_transpose_interleaved_rows_serial(
    source: &[u8],
    destination: &mut [u8],
    source_row_stride: usize,
    destination_row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
) {
    for x in 0..width {
        let destination_start = x * destination_row_stride;
        for y in 0..height {
            let source_start = y * source_row_stride + x * channels;
            let output_start = destination_start + y * channels;
            destination[output_start..output_start + channels]
                .copy_from_slice(&source[source_start..source_start + channels]);
        }
    }
}

#[cfg(feature = "parallel")]
const SIMD_BLUR_PARALLEL_PIXEL_THRESHOLD: usize = 32 * 32;

pub(crate) fn simd_transpose_interleaved_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
) {
    let source_dimensions = CheckedDims::new(width as u32, height as u32, channels as u8)
        .expect("native SIMD blur dimensions were validated at the adapter boundary");
    let destination_dimensions = CheckedDims::new(height as u32, width as u32, channels as u8)
        .expect("native SIMD blur dimensions were validated at the adapter boundary");
    let source_row_stride = source_dimensions.row_stride();
    let destination_row_stride = destination_dimensions.row_stride();
    debug_assert_eq!(source.len(), source_dimensions.total_bytes());
    debug_assert_eq!(destination.len(), destination_dimensions.total_bytes());

    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= SIMD_BLUR_PARALLEL_PIXEL_THRESHOLD {
        simd_rows_mut_serial!(
            destination,
            destination_row_stride,
            width,
            |row_start, row_end, x, row| {
                let _ = (row_start, row_end);
                let x = x as usize;
                for y in 0..height {
                    let source_start = y * source_row_stride + x * channels;
                    let destination_start = y * channels;
                    row[destination_start..destination_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                }
            }
        );
    } else {
        simd_transpose_interleaved_rows_serial(
            source,
            destination,
            source_row_stride,
            destination_row_stride,
            width,
            height,
            channels,
        );
    }
    #[cfg(not(feature = "parallel"))]
    simd_transpose_interleaved_rows_serial(
        source,
        destination,
        source_row_stride,
        destination_row_stride,
        width,
        height,
        channels,
    );
}

#[inline]
pub(crate) fn simd_blur_transpose_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    gaussian_rgba_kernel_candidate: bool,
    use_gaussian_rgba_kernel: bool,
) -> bool {
    #[cfg(all(not(feature = "parallel"), target_arch = "aarch64"))]
    if gaussian_rgba_kernel_candidate && channels == 4 && width >= 256 && height >= 256 {
        // The large ARM64 RGBA Gaussian SIMD path benefits from the existing
        // 4x4 vector transpose; keep other operations and profiles unchanged.
        let _ = transpose_native_tiled::<4, 4>(source, destination, width, height, false, false);
        return false;
    }

    #[cfg(all(not(feature = "parallel"), target_arch = "x86_64"))]
    {
        let _ = gaussian_rgba_kernel_candidate;
        if use_gaussian_rgba_kernel && channels == 4 {
            let started = crate::compute::pipeline_phase_timestamp();
            if let Some((blocks, tail)) = transpose::rgba_avx2(
                source,
                destination,
                width,
                height,
                channels,
                &TransposeMethod::Transpose,
            ) {
                crate::compute::record_transpose_avx2_executor_timing(started);
                crate::compute::record_pipeline_operation_vector_blocks(blocks);
                if tail != 0 {
                    crate::compute::record_pipeline_operation_scalar_tail(tail);
                }
                return true;
            }
        }
    }

    #[cfg(all(not(feature = "parallel"), target_arch = "aarch64"))]
    let _ = use_gaussian_rgba_kernel;
    #[cfg(all(
        not(feature = "parallel"),
        not(any(target_arch = "aarch64", target_arch = "x86_64"))
    ))]
    let _ = (gaussian_rgba_kernel_candidate, use_gaussian_rgba_kernel);
    #[cfg(feature = "parallel")]
    let _ = (gaussian_rgba_kernel_candidate, use_gaussian_rgba_kernel);

    simd_transpose_interleaved_rows(source, destination, width, height, channels);
    false
}
