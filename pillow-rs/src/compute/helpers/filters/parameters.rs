//! Exact filter parameter recognizers shared by support declarations and backends.

const BOX_BLUR_SCALE: u32 = 1 << 24;

/// Calculate Pillow's three-pass Gaussian-to-box radius without changing its
/// f32 input or f64 intermediate arithmetic.
pub(crate) fn gaussian_blur_three_pass_radius(sigma: f32) -> f32 {
    let passes = 3.0f64;
    let sigma2 = f64::from(sigma) * f64::from(sigma) / passes;
    let l_val = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l_val.floor();
    let l1 = l + 1.0;
    let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    let a_den = 6.0 * (sigma2 - l1 * l1);
    (l + a_num / a_den) as f32
}

/// Return Pillow's fixed-point box parameters for one radius.
pub(crate) fn box_blur_parameters(radius: f32) -> (usize, u32, u32) {
    let integer_radius = radius as i32 as usize;
    let window_pixels = (2 * integer_radius + 1) as u32;
    let whole_weight = (BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    let fractional_weight =
        BOX_BLUR_SCALE.wrapping_sub(window_pixels.wrapping_mul(whole_weight)) / 2;
    (integer_radius, whole_weight, fractional_weight)
}

/// Return the exact fixed-point box parameters Pillow uses for GaussianBlur.
pub(crate) fn gaussian_blur_box_parameters(sigma: f32) -> (usize, u32, u32) {
    box_blur_parameters(gaussian_blur_three_pass_radius(sigma.abs()))
}

/// Whether a 3×3 request is the normalized binomial kernel used by native paths.
pub(crate) fn binomial3x3_parameters_match(
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
) -> bool {
    const WEIGHTS: [f32; 9] = [1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0];
    scale == 16.0 && offset == 0.0 && kernel == &WEIGHTS
}

/// Whether a 5x5 request is the exact normalized binomial kernel.
pub(crate) fn binomial5x5_parameters_match(
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> bool {
    const WEIGHTS: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];
    scale == 256.0
        && offset == 0.0
        && kernel
            .iter()
            .enumerate()
            .all(|(index, coefficient)| *coefficient == WEIGHTS[index / 5] * WEIGHTS[index % 5])
}

/// Whether a 5x5 request uses unit weights with denominator 25.
pub(crate) fn uniform5x5_parameters_match(
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> bool {
    scale == 25.0 && offset == 0.0 && kernel.iter().all(|&coefficient| coefficient == 1.0)
}

/// Whether an LA 5x5 request has only its final coefficient doubled.
pub(crate) fn la_single_outlier5x5_parameters_match(
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> bool {
    scale == 25.0
        && offset == 0.0
        && kernel[..24].iter().all(|&coefficient| coefficient == 1.0)
        && kernel[24] == 2.0
}
