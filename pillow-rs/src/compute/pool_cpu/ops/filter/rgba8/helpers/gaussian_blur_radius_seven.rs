//! Shared RGBA GaussianBlur radius-seven horizontal vector kernel.

use wide::u32x8;

const CHANNELS: usize = 4;
const BLOCK_PIXELS: usize = 8;
const RADIUS: usize = 7;
const BOX_BLUR_BIAS: u32 = crate::compute::pool_cpu::ops::filter::helpers::BOX_BLUR_BIAS;

/// Architecture-specific middle-block hook, monomorphized at the mode leaf.
pub(super) trait RadiusSevenHorizontalKernel {
    const HAS_OPTIMIZED_MIDDLE_BLOCK: bool;

    fn blur_middle_block(
        source: &[u8],
        destination: &mut [u8],
        accumulator: &mut [u32; 4],
        whole_weight: u32,
        fractional_weight: u32,
        output_start: usize,
    );
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

/// Blur RGBA rows with the archived eight-pixel fixed-point vector loop.
pub(super) fn blur_horizontal_rows<K: RadiusSevenHorizontalKernel>(
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
            sums[channel] = sums[channel]
                .wrapping_add(u32::from(source[position * CHANNELS + channel]));
        }
        sums[channel] = sums[channel]
            .wrapping_add(u32::from(source[last * CHANNELS + channel]) * last_count as u32);
    }
    debug_assert_eq!(source.len(), width * CHANNELS);
    sums
}

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
    while output < end {
        let output_count = (end - output).min(BLOCK_PIXELS);
        if K::HAS_OPTIMIZED_MIDDLE_BLOCK
            && matches!(region, BlurRegion::Middle)
            && output_count == BLOCK_PIXELS
            && output > RADIUS
            && output + BLOCK_PIXELS + RADIUS < context.width
        {
            K::blur_middle_block(
                source,
                destination,
                accumulator,
                context.whole_weight,
                context.fractional_weight,
                output,
            );
        } else {
            blur_horizontal_block(
                source,
                destination,
                accumulator,
                context,
                output,
                output_count,
                region,
            );
        }
        output += output_count;
    }
    end.saturating_sub(start).div_ceil(BLOCK_PIXELS) as u64
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
    let mut accumulated = [[0u32; BLOCK_PIXELS]; CHANNELS];
    let mut fractional = [[0u32; BLOCK_PIXELS]; CHANNELS];
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
    accumulated: [[u32; BLOCK_PIXELS]; CHANNELS],
    fractional: [[u32; BLOCK_PIXELS]; CHANNELS],
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

