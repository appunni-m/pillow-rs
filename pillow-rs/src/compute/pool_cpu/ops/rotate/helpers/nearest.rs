//! Row spans and contiguous-copy helpers for Pillow's fixed-point nearest path.

pub(crate) fn fixed_coordinate_in_bounds_span(
    origin: i64,
    step: i64,
    source_length: u32,
    destination_length: u32,
) -> Option<(usize, usize)> {
    if step <= 0 { return None; }
    let denominator = i128::from(step);
    let origin = i128::from(origin);
    let source_limit = i128::from(source_length).checked_mul(1 << 16)?;
    let destination_limit = i128::from(destination_length);
    let first = ceil_div_positive_denominator(origin.checked_neg()?, denominator)?;
    let past = ceil_div_positive_denominator(source_limit.checked_sub(origin)?, denominator)?;
    Some((
        first.clamp(0, destination_limit) as usize,
        past.clamp(0, destination_limit) as usize,
    ))
}

fn ceil_div_positive_denominator(value: i128, denominator: i128) -> Option<i128> {
    if denominator <= 0 { return None; }
    let quotient = value.div_euclid(denominator);
    if value.rem_euclid(denominator) == 0 { Some(quotient) } else { quotient.checked_add(1) }
}

pub(crate) fn positive_step_in_bounds_span(
    y: u32,
    destination_width: u32,
    source_size: (u32, u32),
    fixed: (i64, i64, i64, i64, i64, i64),
) -> Option<(usize, usize, i64, i64)> {
    let (origin_x, origin_y, step_x_x, step_y_x, step_x_y, step_y_y) = fixed;
    if step_x_x <= 0 || step_x_y <= 0 { return None; }
    let row = i64::from(y);
    let row_x = origin_x.checked_add(row.checked_mul(step_y_x)?)?;
    let row_y = origin_y.checked_add(row.checked_mul(step_y_y)?)?;
    let x_span = fixed_coordinate_in_bounds_span(row_x, step_x_x, source_size.0, destination_width)?;
    let y_span = fixed_coordinate_in_bounds_span(row_y, step_x_y, source_size.1, destination_width)?;
    let (start, end) = (x_span.0.max(y_span.0), x_span.1.min(y_span.1));
    if start >= end { return Some((start, start, 0, 0)); }
    let offset = i128::try_from(start).ok()?;
    let source_x = i64::try_from(i128::from(row_x).checked_add(offset.checked_mul(i128::from(step_x_x))?)?).ok()?;
    let source_y = i64::try_from(i128::from(row_y).checked_add(offset.checked_mul(i128::from(step_x_y))?)?).ok()?;
    Some((start, end, source_x, source_y))
}

pub(crate) fn contiguous_three_byte_run_len(
    source_x: i64,
    source_y: i64,
    step_x_x: i64,
    step_x_y: i64,
    max_len: usize,
) -> Option<usize> {
    if max_len == 0 || step_x_x <= 0 || step_x_y < 0 { return None; }
    let one = 1_i128 << 16;
    let last = one - 1;
    let x_fraction = i128::from(source_x).rem_euclid(one);
    let y_fraction = i128::from(source_y).rem_euclid(one);
    let x_delta = i128::from(step_x_x) - one;
    let remaining = i128::try_from(max_len).ok()?;
    let x_run = if x_delta > 0 { (last - x_fraction) / x_delta + 1 }
        else if x_delta < 0 { x_fraction / -x_delta + 1 }
        else { remaining };
    let y_step = i128::from(step_x_y);
    let y_run = if y_step == 0 { remaining } else { (last - y_fraction) / y_step + 1 };
    usize::try_from(x_run.min(y_run).min(remaining)).ok()
}

pub(crate) fn fill_three_byte(bytes: &mut [u8], fill: [u8; 3]) {
    for pixel in bytes.chunks_exact_mut(3) { pixel.copy_from_slice(&fill); }
}

pub(crate) fn fill_rgba(bytes: &mut [u8], fill: [u8; 4]) {
    for pixel in bytes.chunks_exact_mut(4) { pixel.copy_from_slice(&fill); }
}
