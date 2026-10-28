// A native I-mode convolution can amortize a uniformity proof over a larger
// image than the byte filters: each interior output otherwise reads and
// converts 25 four-byte samples. Keep the proof bounded so a nearly-uniform
// large image cannot add an unbounded extra full-frame scan.
pub(super) const I32_UNIFORM_FILTER5X5_MAX_PIXELS: usize = 1024 * 1024;
pub(super) const I32_BINOMIAL_FILTER5X5_WEIGHTS: [i32; 5] = [1, 4, 6, 4, 1];
pub(super) const I32_BINOMIAL_FILTER5X5_MAX_ABS_SAMPLE: u32 = 65_535;

pub(super) fn i32_filter5x5_binomial_kernel_is_applicable(kernel: &[f32; 25]) -> bool {
    for (y, vertical) in I32_BINOMIAL_FILTER5X5_WEIGHTS.iter().enumerate() {
        for (x, horizontal) in I32_BINOMIAL_FILTER5X5_WEIGHTS.iter().enumerate() {
            let expected = (*vertical * *horizontal) as f32 / 256.0;
            if kernel[y * 5 + x] != expected {
                return false;
            }
        }
    }
    true
}

/// Check the dimensions, sample layout, offset, and kernel before selecting
/// the integer binomial path for I-mode data.
pub(super) fn i32_filter5x5_bounded_binomial_kernel_is_applicable(
    sample_count: usize,
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    offset: f32,
) -> bool {
    if width < 5 || height < 5 || offset != 0.0 {
        return false;
    }
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    if sample_count != pixel_count {
        return false;
    }
    i32_filter5x5_binomial_kernel_is_applicable(kernel)
}
