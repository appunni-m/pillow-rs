//! Native-sample kernels and arithmetic shared by Filter5x5 mode strategies.

use super::uniform_image::{SMALL_UNIFORM_BYTE_MAX_PIXELS, native_small_uniform_bytes};

const VECTOR_SMALL_UNIFORM_FILTER5X5_MAX_PIXELS: usize = 256 * 256;

#[path = "filter5x5/native_bytes.rs"]
mod native_bytes;

#[path = "rgb_blur_ring.rs"]
pub(crate) mod rgb_blur_ring;
#[path = "uniform_box.rs"]
mod uniform_box;
#[cfg(test)]
#[path = "uniform_box_tests.rs"]
mod uniform_box_tests;

pub(crate) use native_bytes::{
    binomial5x5_parameters_match, filter_5x5_byte_rows,
    filter_5x5_constant_bytes, filter_5x5_la_single_outlier_rows,
    filter_5x5_uniform_luma_rows, filter_5x5_uniform_rgba_identity,
    filter_5x5_uniform_rgba_rows, la_single_outlier5x5_parameters_match,
    uniform5x5_parameters_match,
};
#[cfg(test)]
pub(crate) use native_bytes::filter_5x5_byte_row;
pub(in crate::compute::pool_cpu::ops::filter) use native_bytes::{
    filter_5x5_binomial_la_rows, filter_5x5_binomial_luma_rows,
};

/// Bounded scan limit used only by the exact-mode L/LA/RGB/RGBA leaves.
pub(crate) const SMALL_UNIFORM_LIMIT: usize = SMALL_UNIFORM_BYTE_MAX_PIXELS;

/// Detect a bounded uniform byte image for a mode-owned shortcut.
#[inline]
pub(crate) fn is_small_uniform_bytes(
    raw: &[u8],
    width: usize,
    height: usize,
    channels: usize,
) -> bool {
    native_small_uniform_bytes(
        raw,
        width as u32,
        height as u32,
        channels,
        SMALL_UNIFORM_LIMIT,
    )
}

/// Normalize once outside the pixel loop with the original raw f32 divisor.
#[inline]
pub(crate) fn normalized_kernel(kernel: &[f32; 25], scale: f32) -> [f32; 25] {
    std::array::from_fn(|index| kernel[index] / scale)
}

/// Run the shared native-byte rows without changing per-pixel arithmetic order.
#[inline(never)]
pub(crate) fn apply_native_rows(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    filter_5x5_byte_rows(
        raw,
        output,
        width as i32,
        height as i32,
        channels,
        kernel,
        rounding_bias,
    );
}

/// Run the shared native-byte convolution for modes without a specialized path.
pub(crate) fn execute_filter5x5_native_bytes(
    raw: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Vec<u8> {
    let normalized = normalized_kernel(kernel, scale);
    let mut output = raw.to_vec();
    apply_native_rows(
        raw,
        &mut output,
        width,
        height,
        channels,
        &normalized,
        offset + 0.5,
    );
    output
}

/// Prove and copy a bounded uniform image when Filter5x5 is an identity.
pub(crate) fn execute_filter5x5_small_uniform_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Option<Vec<u8>> {
    if !(1..=4).contains(&channels) {
        return None;
    }
    let pixel_count = (width as usize).checked_mul(height as usize)?;
    let byte_count = pixel_count.checked_mul(channels)?;
    if pixel_count > VECTOR_SMALL_UNIFORM_FILTER5X5_MAX_PIXELS
        || raw.len() != byte_count
        || !native_small_uniform_bytes(
            raw,
            width,
            height,
            channels,
            VECTOR_SMALL_UNIFORM_FILTER5X5_MAX_PIXELS,
        )
    {
        return None;
    }
    if width < 5 || height < 5 {
        return None;
    }
    let first_pixel = raw.get(..channels)?;
    let normalized = normalized_kernel(kernel, scale);
    for channel in 0..channels {
        let sample = f32::from(first_pixel[channel]);
        let mut total = offset + 0.5;
        for row in 0..5 {
            let kernel_start = row * 5;
            let mut row_sum = sample * normalized[kernel_start + 2];
            for column in 0..2 {
                row_sum = sample.mul_add(normalized[kernel_start + column], row_sum);
            }
            for column in 3..5 {
                row_sum = sample.mul_add(normalized[kernel_start + column], row_sum);
            }
            total += row_sum;
        }
        let filtered = if total <= 0.0 {
            0
        } else if total >= 255.0 {
            255
        } else {
            total as u8
        };
        if filtered != first_pixel[channel] {
            return None;
        }
    }
    Some(
        crate::compute::pool_cpu::ops::filter::helpers::execute_native_filter_identity_copy(raw)
            .ok()?,
    )
}

/// Apply the retained large-uniform or vector RGB BLUR-ring fast path.
pub(crate) fn execute_filter5x5_rgb_vector_fast_path(
    raw: &[u8],
    width: u32,
    height: u32,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Option<Vec<u8>> {
    let width_usize = width as usize;
    let height_usize = height as usize;
    let pixel_count = width_usize.checked_mul(height_usize)?;
    if width_usize < 5 || height_usize < 5 {
        return None;
    }
    if pixel_count > VECTOR_SMALL_UNIFORM_FILTER5X5_MAX_PIXELS
        && uniform5x5_parameters_match(kernel, scale, offset)
    {
        if let Some(evidence) = super::filter5x5_rgb_vector::scan_uniform(
            raw,
            width_usize,
            height_usize,
        ) {
            if evidence.vector_blocks != 0 {
                crate::compute::record_pipeline_operation_path("vector");
                crate::compute::record_pipeline_operation_vector_blocks(evidence.vector_blocks);
            }
            if evidence.scalar_tail_pixels != 0 {
                crate::compute::record_pipeline_operation_scalar_tail(
                    evidence.scalar_tail_pixels,
                );
            }
            if evidence.is_uniform {
                return Some(raw.to_vec());
            }
        }
    }
    if pixel_count <= VECTOR_SMALL_UNIFORM_FILTER5X5_MAX_PIXELS
        || !super::rgb_blur_ring::parameters_match(kernel, scale, offset)
    {
        return None;
    }
    let (output, vector_blocks, scalar_tail) =
        super::filter5x5_rgb_vector::apply_blur_ring(raw, width_usize, height_usize)?;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    Some(output)
}

