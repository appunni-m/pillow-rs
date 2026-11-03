//! Native-u16 Kernel arithmetic shared by the four exact I;16 mode strategies.

#[inline]
fn kernel_row<const SIZE: usize>(samples: [u16; SIZE], kernel: &[f32]) -> f32 {
    let mut result = 0.0f32;
    for index in 0..SIZE {
        result += f32::from(samples[index]) * kernel[index];
    }
    result
}

#[inline]
fn window3(input: &[u16], row: usize, x: usize) -> [u16; 3] {
    [input[row + x - 1], input[row + x], input[row + x + 1]]
}

#[inline]
fn window5(input: &[u16], row: usize, x: usize) -> [u16; 5] {
    [
        input[row + x - 2],
        input[row + x - 1],
        input[row + x],
        input[row + x + 1],
        input[row + x + 2],
    ]
}

#[inline]
fn result_to_sample(result: f32) -> u16 {
    // Pillow rounds half away from zero, then clips the output bytes separately.
    // Rust defines float-to-int cases where Pillow's C cast has undefined behavior.
    // TODO(pillow-rs#3ptb): freeze non-finite and out-of-i32-range coefficient parity;
    // Pillow's float-to-int cast is undefined for those convolution results.
    let result = result.round() as i32;
    let low = (result % 256).clamp(0, 255) as u16;
    let high = (result >> 8).clamp(0, 255) as u16;
    low | (high << 8)
}

/// Runs Pillow's bottom-to-top 3x3 I;16 convolution directly on u16 samples.
pub(in crate::compute::pool_cpu::ops::filter) fn execute_u16_filter3x3(
    input: &[u16],
    width: usize,
    height: usize,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
) -> Vec<u16> {
    let kernel = kernel.map(|weight| weight / scale);
    let mut output = input.to_vec();
    if width < 3 || height < 3 {
        return output;
    }

    let rounding_offset = offset + 0.5;
    for y in 1..height - 1 {
        let bottom_row = (y + 1) * width;
        let middle_row = y * width;
        let top_row = (y - 1) * width;
        for x in 1..width - 1 {
            let bottom = kernel_row(window3(input, bottom_row, x), &kernel[0..3]);
            let middle = kernel_row(window3(input, middle_row, x), &kernel[3..6]);
            let top = kernel_row(window3(input, top_row, x), &kernel[6..9]);
            let mut value = rounding_offset;
            value += bottom;
            value += middle;
            value += top;
            output[middle_row + x] = result_to_sample(value);
        }
    }
    output
}

/// Runs Pillow's bottom-to-top 5x5 I;16 convolution directly on u16 samples.
pub(in crate::compute::pool_cpu::ops::filter) fn execute_u16_filter5x5(
    input: &[u16],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Vec<u16> {
    let kernel = kernel.map(|weight| weight / scale);
    let mut output = input.to_vec();
    if width < 5 || height < 5 {
        return output;
    }

    let rounding_offset = offset + 0.5;
    for y in 2..height - 2 {
        let rows = [
            (y + 2) * width,
            (y + 1) * width,
            y * width,
            (y - 1) * width,
            (y - 2) * width,
        ];
        for x in 2..width - 2 {
            let bottom = kernel_row(window5(input, rows[0], x), &kernel[0..5]);
            let lower = kernel_row(window5(input, rows[1], x), &kernel[5..10]);
            let middle = kernel_row(window5(input, rows[2], x), &kernel[10..15]);
            let upper = kernel_row(window5(input, rows[3], x), &kernel[15..20]);
            let top = kernel_row(window5(input, rows[4], x), &kernel[20..25]);
            let mut value = rounding_offset;
            value += bottom;
            value += lower;
            value += middle;
            value += upper;
            value += top;
            output[y * width + x] = result_to_sample(value);
        }
    }
    output
}
