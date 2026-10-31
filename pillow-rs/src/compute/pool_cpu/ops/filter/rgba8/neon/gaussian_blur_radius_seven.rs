//! RGBA GaussianBlur radius-seven horizontal and vertical NEON kernels.

use core::arch::aarch64::{
    int16x8_t, uint8x8_t, uint8x8x4_t, uint16x8_t, uint16x8x4_t, vadd_u16, vaddq_s16,
    vaddq_u16, vaddq_u32, vcombine_u16, vdupq_n_s16, vdupq_n_u16, vdupq_n_u32,
    vextq_s16, vget_high_s16, vget_high_u16, vget_low_s16, vget_low_u16, vgetq_lane_s16, vld4_u8,
    vmovl_s16, vmovl_u8, vmovl_u16, vmovn_u16, vmovn_u32, vmulq_u32,
    vreinterpretq_s16_u16, vreinterpretq_u32_s32, vshrq_n_u32, vst4_u8, vsubq_u16,
};

const CHANNELS: usize = 4;
const BLOCK_PIXELS: usize = 8;
const RADIUS: usize = 7;
const BOX_BLUR_BIAS: u32 = crate::compute::pool_cpu::ops::filter::helpers::BOX_BLUR_BIAS;


pub(super) struct NeonKernel;

impl super::super::helpers::RadiusSevenHorizontalKernel for NeonKernel {
    const HAS_OPTIMIZED_MIDDLE_BLOCK: bool = true;

    fn blur_middle_block(
        source: &[u8],
        destination: &mut [u8],
        accumulator: &mut [u32; CHANNELS],
        whole_weight: u32,
        fractional_weight: u32,
        output_start: usize,
    ) {
        blur_horizontal_middle_block(
            source,
            destination,
            accumulator,
            whole_weight,
            fractional_weight,
            output_start,
        );
    }
}

#[allow(unsafe_code)]
fn blur_horizontal_middle_block(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; CHANNELS],
    whole_weight: u32,
    fractional_weight: u32,
    output_start: usize,
) {
    let leaving_start = (output_start - RADIUS - 1) * CHANNELS;
    let entering_start = (output_start + RADIUS) * CHANNELS;
    let far_right_start = (output_start + RADIUS + 1) * CHANNELS;
    // SAFETY: the region guard guarantees that all three interleaved loads
    // cover complete eight-pixel spans inside this validated row.
    let (leaving, entering, far_right) = unsafe {
        (
            vld4_u8(source.as_ptr().add(leaving_start)),
            vld4_u8(source.as_ptr().add(entering_start)),
            vld4_u8(source.as_ptr().add(far_right_start)),
        )
    };
    let red = process_horizontal_channel(
        leaving.0,
        entering.0,
        far_right.0,
        accumulator[0],
        whole_weight,
        fractional_weight,
    );
    let green = process_horizontal_channel(
        leaving.1,
        entering.1,
        far_right.1,
        accumulator[1],
        whole_weight,
        fractional_weight,
    );
    let blue = process_horizontal_channel(
        leaving.2,
        entering.2,
        far_right.2,
        accumulator[2],
        whole_weight,
        fractional_weight,
    );
    let alpha = process_horizontal_channel(
        leaving.3,
        entering.3,
        far_right.3,
        accumulator[3],
        whole_weight,
        fractional_weight,
    );
    accumulator[0] = red.1;
    accumulator[1] = green.1;
    accumulator[2] = blue.1;
    accumulator[3] = alpha.1;
    // SAFETY: the full eight-pixel destination block is inside this row.
    unsafe {
        vst4_u8(
            destination.as_mut_ptr().add(output_start * CHANNELS),
            uint8x8x4_t(red.0, green.0, blue.0, alpha.0),
        );
    }
}

#[allow(unsafe_code)]
#[inline(always)]
fn process_horizontal_channel(
    leaving: uint8x8_t,
    entering: uint8x8_t,
    far_right: uint8x8_t,
    accumulator: u32,
    whole_weight: u32,
    fractional_weight: u32,
) -> (uint8x8_t, u32) {
    // SAFETY: prefix deltas stay within [-2040, 2040], and the radius-seven
    // Q24 products fit the unsigned 32-bit arithmetic used by Pillow.
    unsafe {
        let leaving_wide: uint16x8_t = vmovl_u8(leaving);
        let entering_wide: uint16x8_t = vmovl_u8(entering);
        let far_right_wide: uint16x8_t = vmovl_u8(far_right);
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
        let bias = vdupq_n_u32(BOX_BLUR_BIAS);
        let result_low = vshrq_n_u32::<24>(vaddq_u32(
            vaddq_u32(vmulq_u32(accumulated_low, weight), fractional_low),
            bias,
        ));
        let result_high = vshrq_n_u32::<24>(vaddq_u32(
            vaddq_u32(vmulq_u32(accumulated_high, weight), fractional_high),
            bias,
        ));
        let result = vmovn_u16(vcombine_u16(vmovn_u32(result_low), vmovn_u32(result_high)));
        let final_accumulator = accumulator.wrapping_add(vgetq_lane_s16::<7>(prefix) as i32 as u32);
        (result, final_accumulator)
    }
}


pub(super) fn blur_vertical_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> (u64, u64) {
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
    for block in 0..block_count {
        blur_vector_column_block(
            source,
            destination,
            row_stride,
            block * BLOCK_PIXELS * CHANNELS,
            last_row,
            whole_weight,
            fractional_weight,
        );
    }
    blur_scalar_columns(
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
    (
        (block_count * height) as u64,
        ((width - vector_width) * height) as u64,
    )
}

#[allow(unsafe_code)]
fn blur_vector_column_block(
    source: &[u8],
    destination: &mut [u8],
    row_stride: usize,
    x_bytes: usize,
    last_row: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let mut sums = initial_column_sums(source, row_stride, x_bytes, last_row);
    for y in 0..=last_row {
        let far_left_y = y.saturating_sub(RADIUS + 1);
        let far_right_y = y.saturating_add(RADIUS + 1).min(last_row);
        // SAFETY: the block index and row bounds keep both eight-pixel RGBA
        // reads inside the already validated source image.
        let (far_left, far_right) = unsafe {
            (
                vld4_u8(source.as_ptr().add(far_left_y * row_stride + x_bytes)),
                vld4_u8(source.as_ptr().add(far_right_y * row_stride + x_bytes)),
            )
        };
        let red = apply_channel(sums.0, far_left.0, far_right.0, whole_weight, fractional_weight);
        let green = apply_channel(sums.1, far_left.1, far_right.1, whole_weight, fractional_weight);
        let blue = apply_channel(sums.2, far_left.2, far_right.2, whole_weight, fractional_weight);
        let alpha = apply_channel(sums.3, far_left.3, far_right.3, whole_weight, fractional_weight);
        // SAFETY: this block writes eight complete RGBA pixels in the same
        // validated row-major span used by the reads above.
        unsafe {
            vst4_u8(
                destination.as_mut_ptr().add(y * row_stride + x_bytes),
                uint8x8x4_t(red, green, blue, alpha),
            );
        }
        if y < last_row {
            advance_column_sums(
                source,
                row_stride,
                x_bytes,
                y,
                last_row,
                &mut sums,
            );
        }
    }
}

#[allow(unsafe_code)]
fn initial_column_sums(
    source: &[u8],
    row_stride: usize,
    x_bytes: usize,
    last_row: usize,
) -> uint16x8x4_t {
    // SAFETY: each clamped sample row exists, and x_bytes identifies a full
    // eight-pixel block within the validated image width.
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
        // SAFETY: `sample_y` is clamped to an existing row; the complete
        // vector block is inside the checked RGBA row.
        let sample = unsafe { vld4_u8(source.as_ptr().add(sample_y * row_stride + x_bytes)) };
        // SAFETY: widening and adding these byte samples cannot overflow u16.
        unsafe {
            sums.0 = vaddq_u16(sums.0, vmovl_u8(sample.0));
            sums.1 = vaddq_u16(sums.1, vmovl_u8(sample.1));
            sums.2 = vaddq_u16(sums.2, vmovl_u8(sample.2));
            sums.3 = vaddq_u16(sums.3, vmovl_u8(sample.3));
        }
    }
    sums
}

#[allow(unsafe_code)]
fn advance_column_sums(
    source: &[u8],
    row_stride: usize,
    x_bytes: usize,
    y: usize,
    last_row: usize,
    sums: &mut uint16x8x4_t,
) {
    let leaving_y = y.saturating_sub(RADIUS);
    let entering_y = y.saturating_add(RADIUS + 1).min(last_row);
    // SAFETY: both rows are clamped within the checked image, and this vector
    // block is the same complete block used to initialize the rolling sums.
    let (leaving, entering) = unsafe {
        (
            vld4_u8(source.as_ptr().add(leaving_y * row_stride + x_bytes)),
            vld4_u8(source.as_ptr().add(entering_y * row_stride + x_bytes)),
        )
    };
    // SAFETY: every rolling sum remains bounded by 15 * 255.
    unsafe {
        sums.0 = vaddq_u16(vsubq_u16(sums.0, vmovl_u8(leaving.0)), vmovl_u8(entering.0));
        sums.1 = vaddq_u16(vsubq_u16(sums.1, vmovl_u8(leaving.1)), vmovl_u8(entering.1));
        sums.2 = vaddq_u16(vsubq_u16(sums.2, vmovl_u8(leaving.2)), vmovl_u8(entering.2));
        sums.3 = vaddq_u16(vsubq_u16(sums.3, vmovl_u8(leaving.3)), vmovl_u8(entering.3));
    }
}

#[inline(always)]
#[allow(unsafe_code)]
fn apply_channel(
    sum: uint16x8_t,
    far_left: uint8x8_t,
    far_right: uint8x8_t,
    whole_weight: u32,
    fractional_weight: u32,
) -> uint8x8_t {
    // SAFETY: all vector operations are lane-wise arithmetic on values whose
    // Q24 products fit in u32 for this 15-sample radius-seven window.
    unsafe {
        let sum_low = vmovl_u16(vget_low_u16(sum));
        let sum_high = vmovl_u16(vget_high_u16(sum));
        let far_sum = vaddq_u16(vmovl_u8(far_left), vmovl_u8(far_right));
        let fractional_low = vmulq_u32(
            vmovl_u16(vget_low_u16(far_sum)),
            vdupq_n_u32(fractional_weight),
        );
        let fractional_high = vmulq_u32(
            vmovl_u16(vget_high_u16(far_sum)),
            vdupq_n_u32(fractional_weight),
        );
        let whole = vdupq_n_u32(whole_weight);
        let bias = vdupq_n_u32(BOX_BLUR_BIAS);
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

fn blur_scalar_columns(
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
    for x in start_x..width {
        let x_bytes = x * CHANNELS;
        let mut sums = [0u32; CHANNELS];
        for offset in -(RADIUS as isize)..=RADIUS as isize {
            let sample_y = offset.clamp(0, last_row as isize) as usize;
            for (channel, sum) in sums.iter_mut().enumerate() {
                *sum += u32::from(source[sample_y * row_stride + x_bytes + channel]);
            }
        }
        blur_scalar_column_rows(
            source,
            destination,
            x_bytes,
            row_stride,
            height,
            last_row,
            whole_weight,
            fractional_weight,
            &mut sums,
        );
    }
}

fn blur_scalar_column_rows(
    source: &[u8],
    destination: &mut [u8],
    x_bytes: usize,
    row_stride: usize,
    height: usize,
    last_row: usize,
    whole_weight: u32,
    fractional_weight: u32,
    sums: &mut [u32; CHANNELS],
) {
    for y in 0..height {
        let far_left_y = y.saturating_sub(RADIUS + 1);
        let far_right_y = y.saturating_add(RADIUS + 1).min(last_row);
        for (channel, sum) in sums.iter().copied().enumerate() {
            let far_left = u32::from(source[far_left_y * row_stride + x_bytes + channel]);
            let far_right = u32::from(source[far_right_y * row_stride + x_bytes + channel]);
            destination[y * row_stride + x_bytes + channel] = sum
                .wrapping_mul(whole_weight)
                .wrapping_add((far_left + far_right).wrapping_mul(fractional_weight))
                .wrapping_add(BOX_BLUR_BIAS)
                .wrapping_shr(24) as u8;
        }
        if y < last_row {
            let leaving_y = y.saturating_sub(RADIUS);
            let entering_y = y.saturating_add(RADIUS + 1).min(last_row);
            for (channel, sum) in sums.iter_mut().enumerate() {
                *sum = sum
                    .wrapping_sub(u32::from(
                        source[leaving_y * row_stride + x_bytes + channel],
                    ))
                    .wrapping_add(u32::from(
                        source[entering_y * row_stride + x_bytes + channel],
                    ));
            }
        }
    }
}
