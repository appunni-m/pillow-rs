pub(crate) fn ceil_div_positive_denominator(value: i128, denominator: i128) -> Option<i128> {
    if denominator <= 0 {
        return None;
    }
    let quotient = value.div_euclid(denominator);
    if value.rem_euclid(denominator) == 0 {
        Some(quotient)
    } else {
        quotient.checked_add(1)
    }
}

pub(crate) fn fixed_coordinate_in_bounds_span(
    origin: i64,
    step: i64,
    source_length: u32,
    destination_length: u32,
) -> Option<(usize, usize)> {
    if step <= 0 {
        return None;
    }

    let denominator = i128::from(step);
    let origin = i128::from(origin);
    let source_limit = i128::from(source_length).checked_mul(1 << 16)?;
    let destination_limit = i128::from(destination_length);
    let first_in_bounds = ceil_div_positive_denominator(origin.checked_neg()?, denominator)?;
    let first_past_bounds =
        ceil_div_positive_denominator(source_limit.checked_sub(origin)?, denominator)?;
    let first_in_bounds = first_in_bounds.clamp(0, destination_limit);
    let first_past_bounds = first_past_bounds.clamp(0, destination_limit);

    Some((first_in_bounds as usize, first_past_bounds as usize))
}

pub(crate) fn nearest_positive_step_in_bounds_span(
    y: u32,
    destination_width: u32,
    source_width: u32,
    source_height: u32,
    origin_x: i64,
    origin_y: i64,
    step_x_x: i64,
    step_y_x: i64,
    step_x_y: i64,
    step_y_y: i64,
) -> Option<(usize, usize, i64, i64)> {
    if step_x_x <= 0 || step_x_y <= 0 {
        return None;
    }

    let row = i64::from(y);
    let row_x = origin_x.checked_add(row.checked_mul(step_y_x)?)?;
    let row_y = origin_y.checked_add(row.checked_mul(step_y_y)?)?;
    let x_span = fixed_coordinate_in_bounds_span(row_x, step_x_x, source_width, destination_width)?;
    let y_span =
        fixed_coordinate_in_bounds_span(row_y, step_x_y, source_height, destination_width)?;
    let start = x_span.0.max(y_span.0);
    let end = x_span.1.min(y_span.1);
    if start >= end {
        return Some((start, start, 0, 0));
    }

    let x_offset = i128::try_from(start)
        .ok()?
        .checked_mul(i128::from(step_x_x))?;
    let y_offset = i128::try_from(start)
        .ok()?
        .checked_mul(i128::from(step_x_y))?;
    let source_x = i64::try_from(i128::from(row_x).checked_add(x_offset)?).ok()?;
    let source_y = i64::try_from(i128::from(row_y).checked_add(y_offset)?).ok()?;

    Some((start, end, source_x, source_y))
}

pub(crate) fn fill_rgb_pixels(bytes: &mut [u8], fill_pixel: [u8; 3]) {
    for pixel in bytes.chunks_exact_mut(3) {
        pixel.copy_from_slice(&fill_pixel);
    }
}

pub(crate) fn fill_rgba_pixels(bytes: &mut [u8], fill_pixel: [u8; 4]) {
    for pixel in bytes.chunks_exact_mut(4) {
        pixel.copy_from_slice(&fill_pixel);
    }
}
