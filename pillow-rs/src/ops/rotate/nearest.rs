/// Bound a run where Pillow's fixed-point nearest sampler advances through
/// consecutive source columns while staying on one source row.
///
/// The caller must cap `max_len` at the source row boundary and verify that
/// the first sample is in bounds. This control-plane helper is shared by the
/// serial CPU and native SIMD RGB Rotate kernels.
pub(crate) fn nearest_contiguous_rgb_run_len(
    source_x: i64,
    source_y: i64,
    step_x_x: i64,
    step_x_y: i64,
    max_len: usize,
) -> Option<usize> {
    if max_len == 0 || step_x_x <= 0 || step_x_y < 0 {
        return None;
    }

    let fixed_one = 1_i128 << 16;
    let fixed_last = fixed_one - 1;
    let x_fraction = i128::from(source_x).rem_euclid(fixed_one);
    let y_fraction = i128::from(source_y).rem_euclid(fixed_one);
    let x_delta_from_one = i128::from(step_x_x) - fixed_one;
    let remaining = i128::try_from(max_len).ok()?;

    // For a contiguous source slice, each destination step must advance the
    // integer source x by exactly one. Bound the run where the fractional
    // error relative to 1.0 remains in the current fixed-point pixel.
    let x_run = if x_delta_from_one > 0 {
        (fixed_last - x_fraction) / x_delta_from_one + 1
    } else if x_delta_from_one < 0 {
        x_fraction / -x_delta_from_one + 1
    } else {
        remaining
    };

    // The source y coordinate must stay on one row for the same run.
    let y_step = i128::from(step_x_y);
    let y_run = if y_step == 0 {
        remaining
    } else {
        (fixed_last - y_fraction) / y_step + 1
    };

    usize::try_from(x_run.min(y_run).min(remaining)).ok()
}
