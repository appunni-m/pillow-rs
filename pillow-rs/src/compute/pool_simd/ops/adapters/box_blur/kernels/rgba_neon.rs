//! AArch64 NEON blocks for four-channel fractional box blur.

use super::SIMD_BOX_BLUR_BIAS;
use core::arch::aarch64::{
    int16x8_t, uint8x8_t, uint8x8x4_t, uint16x8_t, uint16x8x4_t, vadd_u16, vaddq_s16, vaddq_u16,
    vaddq_u32, vcombine_u16, vdupq_n_s16, vdupq_n_u16, vdupq_n_u32, vextq_s16, vget_high_s16,
    vget_high_u16, vget_low_s16, vget_low_u16, vgetq_lane_s16, vld4_u8, vmovl_s16, vmovl_u8,
    vmovl_u16, vmovn_u16, vmovn_u32, vmulq_u32, vreinterpretq_s16_u16, vreinterpretq_u32_s32,
    vshrq_n_u32, vst4_u8, vsubq_u16,
};

/// Process one complete eight-pixel middle block with a NEON prefix scan.
///
/// The incoming accumulator is the window sum immediately before this block.
/// Each output receives the exact scalar rolling sum for its lane, then uses
/// Pillow's existing Q24 multiply, fractional taps, bias, and byte boundary.
#[allow(unsafe_code)]
pub(super) fn blur_middle_block(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; 4],
    whole_weight: u32,
    fractional_weight: u32,
    output_start: usize,
) {
    const CHANNELS: usize = 4;
    const RADIUS: usize = 7;
    const BLOCK_PIXELS: usize = 8;
    let line_length = source.len() / CHANNELS;
    debug_assert_eq!(source.len() % CHANNELS, 0);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert!(output_start > RADIUS);
    debug_assert!(output_start + BLOCK_PIXELS + RADIUS < line_length);
    debug_assert_ne!(fractional_weight, 0);

    // The three streams are entering samples, leaving samples, and the far
    // fractional tap. vld4 deinterleaves the native RGBA layout without an
    // intermediate pixel-format conversion.
    unsafe {
        let leaving_start = (output_start - RADIUS - 1) * CHANNELS;
        let entering_start = (output_start + RADIUS) * CHANNELS;
        let far_right_start = (output_start + RADIUS + 1) * CHANNELS;
        let leaving = vld4_u8(source.as_ptr().add(leaving_start));
        let entering = vld4_u8(source.as_ptr().add(entering_start));
        let far_right = vld4_u8(source.as_ptr().add(far_right_start));

        let (red, red_accumulator) = process_channel(
            leaving.0,
            entering.0,
            far_right.0,
            accumulator[0],
            whole_weight,
            fractional_weight,
        );
        let (green, green_accumulator) = process_channel(
            leaving.1,
            entering.1,
            far_right.1,
            accumulator[1],
            whole_weight,
            fractional_weight,
        );
        let (blue, blue_accumulator) = process_channel(
            leaving.2,
            entering.2,
            far_right.2,
            accumulator[2],
            whole_weight,
            fractional_weight,
        );
        let (alpha, alpha_accumulator) = process_channel(
            leaving.3,
            entering.3,
            far_right.3,
            accumulator[3],
            whole_weight,
            fractional_weight,
        );

        accumulator[0] = red_accumulator;
        accumulator[1] = green_accumulator;
        accumulator[2] = blue_accumulator;
        accumulator[3] = alpha_accumulator;
        let rgba = uint8x8x4_t(red, green, blue, alpha);
        vst4_u8(destination.as_mut_ptr().add(output_start * CHANNELS), rgba);
    }
}

/// Blur RGBA columns directly in row-major storage without transposing them.
///
/// The four NEON accumulators are the exact rounded-window sums for one group
/// of eight adjacent pixels and stay live while advancing through rows. This
/// preserves each vertical pass's Q24 boundary while keeping destination
/// stores in native RGBA order.
#[allow(unsafe_code)]
pub(crate) fn blur_vertical_rows_radius_seven(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> (u64, u64) {
    const CHANNELS: usize = 4;
    const BLOCK_PIXELS: usize = 8;
    const RADIUS: usize = 7;
    let row_stride = width * CHANNELS;
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert_ne!(fractional_weight, 0);

    if width == 0 || height == 0 {
        return (0, 0);
    }

    let block_count = width / BLOCK_PIXELS;
    let vector_width = block_count * BLOCK_PIXELS;
    let last_row = height - 1;
    let vector_blocks = (block_count * height) as u64;
    let scalar_tail = ((width - vector_width) * height) as u64;

    for block in 0..block_count {
        let x_bytes = block * BLOCK_PIXELS * CHANNELS;
        let mut sums = initial_rgba_column_sums(source, row_stride, x_bytes, last_row);

        for y in 0..height {
            let far_left_y = y.saturating_sub(RADIUS + 1);
            let far_right_y = y.saturating_add(RADIUS + 1).min(last_row);
            let far_left =
                unsafe { vld4_u8(source.as_ptr().add(far_left_y * row_stride + x_bytes)) };
            let far_right =
                unsafe { vld4_u8(source.as_ptr().add(far_right_y * row_stride + x_bytes)) };
            let red = unsafe {
                apply_channel(
                    sums.0,
                    far_left.0,
                    far_right.0,
                    whole_weight,
                    fractional_weight,
                )
            };
            let green = unsafe {
                apply_channel(
                    sums.1,
                    far_left.1,
                    far_right.1,
                    whole_weight,
                    fractional_weight,
                )
            };
            let blue = unsafe {
                apply_channel(
                    sums.2,
                    far_left.2,
                    far_right.2,
                    whole_weight,
                    fractional_weight,
                )
            };
            let alpha = unsafe {
                apply_channel(
                    sums.3,
                    far_left.3,
                    far_right.3,
                    whole_weight,
                    fractional_weight,
                )
            };
            let output = uint8x8x4_t(red, green, blue, alpha);
            unsafe {
                vst4_u8(
                    destination.as_mut_ptr().add(y * row_stride + x_bytes),
                    output,
                );
            }

            if y + 1 < height {
                let leaving_y = y.saturating_sub(RADIUS);
                let entering_y = y.saturating_add(RADIUS + 1).min(last_row);
                let leaving =
                    unsafe { vld4_u8(source.as_ptr().add(leaving_y * row_stride + x_bytes)) };
                let entering =
                    unsafe { vld4_u8(source.as_ptr().add(entering_y * row_stride + x_bytes)) };
                unsafe {
                    sums.0 =
                        vaddq_u16(vsubq_u16(sums.0, vmovl_u8(leaving.0)), vmovl_u8(entering.0));
                    sums.1 =
                        vaddq_u16(vsubq_u16(sums.1, vmovl_u8(leaving.1)), vmovl_u8(entering.1));
                    sums.2 =
                        vaddq_u16(vsubq_u16(sums.2, vmovl_u8(leaving.2)), vmovl_u8(entering.2));
                    sums.3 =
                        vaddq_u16(vsubq_u16(sums.3, vmovl_u8(leaving.3)), vmovl_u8(entering.3));
                }
            }
        }
    }

    blur_vertical_scalar_columns(
        source,
        destination,
        width,
        height,
        row_stride,
        vector_width,
        last_row,
        whole_weight,
        fractional_weight,
    );

    (vector_blocks, scalar_tail)
}

#[inline(always)]
#[allow(unsafe_code)]
fn initial_rgba_column_sums(
    source: &[u8],
    row_stride: usize,
    x_bytes: usize,
    last_row: usize,
) -> uint16x8x4_t {
    const RADIUS: usize = 7;
    // SAFETY: AArch64 guarantees NEON, and this module is target-gated.
    let mut sums = unsafe {
        uint16x8x4_t(
            vdupq_n_u16(0),
            vdupq_n_u16(0),
            vdupq_n_u16(0),
            vdupq_n_u16(0),
        )
    };
    for offset in -(RADIUS as isize)..=RADIUS as isize {
        let sample_y = offset.clamp(0, last_row as isize) as usize;
        let sample = unsafe { vld4_u8(source.as_ptr().add(sample_y * row_stride + x_bytes)) };
        unsafe {
            sums.0 = vaddq_u16(sums.0, vmovl_u8(sample.0));
            sums.1 = vaddq_u16(sums.1, vmovl_u8(sample.1));
            sums.2 = vaddq_u16(sums.2, vmovl_u8(sample.2));
            sums.3 = vaddq_u16(sums.3, vmovl_u8(sample.3));
        }
    }
    sums
}

fn blur_vertical_scalar_columns(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    row_stride: usize,
    start_x: usize,
    last_row: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    const CHANNELS: usize = 4;
    const RADIUS: usize = 7;
    for x in start_x..width {
        let x_bytes = x * CHANNELS;
        let mut sums = [0u32; CHANNELS];
        for offset in -(RADIUS as isize)..=RADIUS as isize {
            let sample_y = offset.clamp(0, last_row as isize) as usize;
            for (channel, sum) in sums.iter_mut().enumerate() {
                *sum += source[sample_y * row_stride + x_bytes + channel] as u32;
            }
        }
        for y in 0..height {
            let far_left_y = y.saturating_sub(RADIUS + 1);
            let far_right_y = y.saturating_add(RADIUS + 1).min(last_row);
            for (channel, sum) in sums.iter().copied().enumerate() {
                let far_left = source[far_left_y * row_stride + x_bytes + channel] as u32;
                let far_right = source[far_right_y * row_stride + x_bytes + channel] as u32;
                destination[y * row_stride + x_bytes + channel] =
                    sum.wrapping_mul(whole_weight)
                        .wrapping_add((far_left + far_right).wrapping_mul(fractional_weight))
                        .wrapping_add(SIMD_BOX_BLUR_BIAS)
                        .wrapping_shr(24) as u8;
            }
            if y + 1 < height {
                let leaving_y = y.saturating_sub(RADIUS);
                let entering_y = y.saturating_add(RADIUS + 1).min(last_row);
                for (channel, sum) in sums.iter_mut().enumerate() {
                    *sum = sum
                        .wrapping_sub(source[leaving_y * row_stride + x_bytes + channel] as u32)
                        .wrapping_add(source[entering_y * row_stride + x_bytes + channel] as u32);
                }
            }
        }
    }
}

#[inline(always)]
#[allow(unsafe_code)]
unsafe fn apply_channel(
    sum: uint16x8_t,
    far_left: uint8x8_t,
    far_right: uint8x8_t,
    whole_weight: u32,
    fractional_weight: u32,
) -> uint8x8_t {
    unsafe {
        let sum_low = vmovl_u16(vget_low_u16(sum));
        let sum_high = vmovl_u16(vget_high_u16(sum));
        let far_left_wide = vmovl_u8(far_left);
        let far_right_wide = vmovl_u8(far_right);
        let fractional_low = vmulq_u32(
            vmovl_u16(vget_low_u16(vaddq_u16(far_left_wide, far_right_wide))),
            vdupq_n_u32(fractional_weight),
        );
        let fractional_high = vmulq_u32(
            vmovl_u16(vget_high_u16(vaddq_u16(far_left_wide, far_right_wide))),
            vdupq_n_u32(fractional_weight),
        );
        let whole = vdupq_n_u32(whole_weight);
        let bias = vdupq_n_u32(SIMD_BOX_BLUR_BIAS);
        let result_low = vshrq_n_u32::<24>(vaddq_u32(
            vaddq_u32(vmulq_u32(sum_low, whole), fractional_low),
            bias,
        ));
        let result_high = vshrq_n_u32::<24>(vaddq_u32(
            vaddq_u32(vmulq_u32(sum_high, whole), fractional_high),
            bias,
        ));
        vmovn_u16(vcombine_u16(vmovn_u32(result_low), vmovn_u32(result_high)))
    }
}

#[inline(always)]
#[allow(unsafe_code)]
unsafe fn process_channel(
    leaving: uint8x8_t,
    entering: uint8x8_t,
    far_right: uint8x8_t,
    accumulator: u32,
    whole_weight: u32,
    fractional_weight: u32,
) -> (uint8x8_t, u32) {
    unsafe {
        let leaving_wide: uint16x8_t = vmovl_u8(leaving);
        let entering_wide: uint16x8_t = vmovl_u8(entering);
        let far_right_wide: uint16x8_t = vmovl_u8(far_right);

        // Byte differences are within [-255, 255]. An eight-lane prefix is
        // within [-2040, 2040], so signed 16-bit lanes preserve every delta
        // exactly before widening for the existing Q24 arithmetic.
        let delta: int16x8_t = vreinterpretq_s16_u16(vsubq_u16(entering_wide, leaving_wide));
        let zero = vdupq_n_s16(0);
        let prefix_one = vaddq_s16(delta, vextq_s16::<7>(zero, delta));
        let prefix_two = vaddq_s16(prefix_one, vextq_s16::<6>(zero, prefix_one));
        let prefix = vaddq_s16(prefix_two, vextq_s16::<4>(zero, prefix_two));
        let prefix_low = vreinterpretq_u32_s32(vmovl_s16(vget_low_s16(prefix)));
        let prefix_high = vreinterpretq_u32_s32(vmovl_s16(vget_high_s16(prefix)));
        let base = vdupq_n_u32(accumulator);
        let accumulated_low = vaddq_u32(prefix_low, base);
        let accumulated_high = vaddq_u32(prefix_high, base);

        let leaving_low = vget_low_u16(leaving_wide);
        let leaving_high = vget_high_u16(leaving_wide);
        let far_right_low = vget_low_u16(far_right_wide);
        let far_right_high = vget_high_u16(far_right_wide);
        let fractional_low = vmulq_u32(
            vmovl_u16(vadd_u16(leaving_low, far_right_low)),
            vdupq_n_u32(fractional_weight),
        );
        let fractional_high = vmulq_u32(
            vmovl_u16(vadd_u16(leaving_high, far_right_high)),
            vdupq_n_u32(fractional_weight),
        );
        let weight = vdupq_n_u32(whole_weight);
        let bias = vdupq_n_u32(SIMD_BOX_BLUR_BIAS);
        let result_low = vshrq_n_u32::<24>(vaddq_u32(
            vaddq_u32(vmulq_u32(accumulated_low, weight), fractional_low),
            bias,
        ));
        let result_high = vshrq_n_u32::<24>(vaddq_u32(
            vaddq_u32(vmulq_u32(accumulated_high, weight), fractional_high),
            bias,
        ));
        let result_bytes = vmovn_u16(vcombine_u16(vmovn_u32(result_low), vmovn_u32(result_high)));
        let final_accumulator = accumulator.wrapping_add(vgetq_lane_s16::<7>(prefix) as i32 as u32);
        (result_bytes, final_accumulator)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{SimdBlurRegion, simd_blur_line};
    use super::blur_vertical_rows_radius_seven;

    const WHOLE_WEIGHT: u32 = 1_116_425;
    const FRACTIONAL_WEIGHT: u32 = 15_420;
    const RADIUS: usize = 7;

    fn scalar_reference(
        source: &[u8],
        width: usize,
        whole_weight: u32,
        fractional_weight: u32,
    ) -> Vec<u8> {
        let last = width - 1;
        let edge_a = (RADIUS + 1).min(width);
        let edge_b = width.saturating_sub(RADIUS + 1);
        let mut accumulator = [0u32; 4];
        for component in 0..4 {
            accumulator[component] = (source[component] as u32).wrapping_mul((RADIUS + 1) as u32);
        }
        for position in 0..edge_a.saturating_sub(1) {
            for component in 0..4 {
                accumulator[component] =
                    accumulator[component].wrapping_add(source[position * 4 + component] as u32);
            }
        }
        let last_count = RADIUS.saturating_add(1).saturating_sub(edge_a);
        for component in 0..4 {
            accumulator[component] = accumulator[component].wrapping_add(
                (source[last * 4 + component] as u32).wrapping_mul(last_count as u32),
            );
        }

        let mut output = vec![0; source.len()];
        for x in 0..width {
            let region = if edge_a <= edge_b {
                if x < edge_a {
                    SimdBlurRegion::Leading
                } else if x < edge_b {
                    SimdBlurRegion::Middle
                } else {
                    SimdBlurRegion::Trailing
                }
            } else if x < edge_b {
                SimdBlurRegion::Leading
            } else if x < edge_a {
                SimdBlurRegion::Clamped
            } else {
                SimdBlurRegion::Trailing
            };
            let (subtract, add, far_left, far_right) = match region {
                SimdBlurRegion::Leading => (0, x + RADIUS, 0, x + RADIUS + 1),
                SimdBlurRegion::Middle => {
                    (x - RADIUS - 1, x + RADIUS, x - RADIUS - 1, x + RADIUS + 1)
                }
                SimdBlurRegion::Trailing => (x - RADIUS - 1, last, x - RADIUS - 1, last),
                SimdBlurRegion::Clamped => (0, last, 0, last),
            };
            for component in 0..4 {
                accumulator[component] = accumulator[component]
                    .wrapping_sub(source[subtract * 4 + component] as u32)
                    .wrapping_add(source[add * 4 + component] as u32);
                let fractional = (source[far_left * 4 + component] as u32
                    + source[far_right * 4 + component] as u32)
                    .wrapping_mul(fractional_weight);
                output[x * 4 + component] = (accumulator[component]
                    .wrapping_mul(whole_weight)
                    .wrapping_add(fractional)
                    .wrapping_add(super::super::SIMD_BOX_BLUR_BIAS)
                    >> 24) as u8;
            }
        }
        output
    }

    #[test]
    fn rgba_radius_seven_neon_prefix_blocks_match_scalar_edges_and_tails() {
        for width in [1, 7, 8, 15, 16, 17, 31, 32, 33, 64, 1024] {
            let source: Vec<u8> = (0..width * 4)
                .map(|index| ((index * 73 + width * 31 + (index >> 3) * 29) % 256) as u8)
                .collect();
            let expected = scalar_reference(&source, width, WHOLE_WEIGHT, FRACTIONAL_WEIGHT);
            let mut actual = vec![0; source.len()];
            simd_blur_line::<true>(
                &source,
                &mut actual,
                width,
                4,
                RADIUS,
                WHOLE_WEIGHT,
                FRACTIONAL_WEIGHT,
            );
            assert_eq!(actual, expected, "width={width}");
        }
    }

    #[test]
    fn rgba_radius_seven_row_major_vertical_matches_scalar_edges_and_tails() {
        for width in [1, 7, 8, 15, 16, 17, 31, 32, 33, 64, 1024, 1025] {
            for height in [1, 7, 8, 15, 16, 17, 31, 64, 769] {
                let source: Vec<u8> = (0..width * height * 4)
                    .map(|index| ((index * 73 + width * 31 + (index >> 3) * 29) % 256) as u8)
                    .collect();
                let mut expected = vec![0; source.len()];
                for x in 0..width {
                    let mut column = vec![0; height * 4];
                    for y in 0..height {
                        let source_start = (y * width + x) * 4;
                        column[y * 4..y * 4 + 4]
                            .copy_from_slice(&source[source_start..source_start + 4]);
                    }
                    let expected_column =
                        scalar_reference(&column, height, WHOLE_WEIGHT, FRACTIONAL_WEIGHT);
                    for y in 0..height {
                        let destination_start = (y * width + x) * 4;
                        expected[destination_start..destination_start + 4]
                            .copy_from_slice(&expected_column[y * 4..y * 4 + 4]);
                    }
                }

                let mut actual = vec![0; source.len()];
                let (vector_blocks, scalar_tail) = blur_vertical_rows_radius_seven(
                    &source,
                    &mut actual,
                    width,
                    height,
                    WHOLE_WEIGHT,
                    FRACTIONAL_WEIGHT,
                );
                assert_eq!(actual, expected, "width={width}, height={height}");
                assert_eq!(vector_blocks, (width / 8 * height) as u64);
                assert_eq!(scalar_tail, (width % 8 * height) as u64);
            }
        }
    }
}
