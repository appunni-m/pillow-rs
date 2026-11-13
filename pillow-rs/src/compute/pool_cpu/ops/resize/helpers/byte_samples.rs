//! Serial interleaved byte kernels for modes with identical channel arithmetic.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::{
    FilterCoeffs, compact_resize_coeffs_to_source_span, fixed_point_to_u8, precompute_coeffs,
    precompute_coeffs_boxed_for_filter,
};
use crate::pipeline::ResampleFilter;

type Bounds = Option<(f64, f64, f64, f64)>;

struct ResizePlan<'a> {
    source: &'a [u8],
    sparse_source_rows: Option<&'a [bool]>,
    source_row_offset: u32,
    source_size: (u32, u32),
    destination_size: (u32, u32),
    horizontal: &'a FilterCoeffs,
    vertical: &'a FilterCoeffs,
    need_horizontal: bool,
    need_vertical: bool,
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn resize<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: Bounds,
) -> Result<Vec<u8>, PilError> {
    resize_with_sparse_source_rows::<CHANNELS>(
        source,
        source_size,
        destination_size,
        filter,
        bounds,
        None,
    )
}

/// Execute a boxed byte-sample resize while skipping rows proven to contain only zeros.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn resize_boxed_with_sparse_source_rows<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    sparse_source_rows: &[bool],
) -> Result<Vec<u8>, PilError> {
    resize_with_sparse_source_rows::<CHANNELS>(
        source,
        source_size,
        destination_size,
        filter,
        Some(bounds),
        Some(sparse_source_rows),
    )
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn resize_with_sparse_source_rows<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: Bounds,
    sparse_source_rows: Option<&[bool]>,
) -> Result<Vec<u8>, PilError> {
    let channels = checked_channels::<CHANNELS>()?;
    let source_dims = CheckedDims::new_allow_empty(source_size.0, source_size.1, channels)?;
    if source.len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "byte-sample Resize strategy received an invalid source buffer".into(),
        ));
    }
    let destination_dims =
        CheckedDims::new_allow_empty(destination_size.0, destination_size.1, channels)?;
    if let Some(bounds) = bounds
        && let Some(output) = boxed_early::<CHANNELS>(
            source,
            source_size,
            destination_size,
            bounds,
            destination_dims,
        )
    {
        return Ok(output);
    }
    if is_empty(source_size, destination_size) {
        return Ok(destination_dims.alloc_buffer());
    }
    if bounds.is_none() && source_size == destination_size {
        return Ok(source.to_vec());
    }
    let destination_dims = CheckedDims::new(destination_size.0, destination_size.1, channels)?;
    if matches!(filter, ResampleFilter::Nearest) {
        return Ok(match bounds {
            Some(bounds) => nearest_boxed::<CHANNELS>(
                source,
                source_size,
                destination_size,
                bounds,
                destination_dims,
            ),
            None => nearest::<CHANNELS>(source, source_size, destination_size, destination_dims),
        });
    }
    filtered::<CHANNELS>(
        source,
        source_size,
        destination_size,
        filter,
        bounds,
        sparse_source_rows,
    )
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn is_empty(source_size: (u32, u32), destination_size: (u32, u32)) -> bool {
    source_size.0 == 0 || source_size.1 == 0 || destination_size.0 == 0 || destination_size.1 == 0
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn checked_channels<const CHANNELS: usize>() -> Result<u8, PilError> {
    u8::try_from(CHANNELS)
        .ok()
        .filter(|channels| *channels != 0)
        .ok_or_else(|| PilError::DimensionError("invalid byte-sample channel count".into()))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn boxed_early<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
    destination_dims: CheckedDims,
) -> Option<Vec<u8>> {
    let (left, top, right, bottom) = bounds;
    let (left, top, right, bottom) = (left as f32, top as f32, right as f32, bottom as f32);
    let finite = left.is_finite() && top.is_finite() && right.is_finite() && bottom.is_finite();
    if finite
        && source_size == destination_size
        && left == 0.0
        && top == 0.0
        && right == source_size.0 as f32
        && bottom == source_size.1 as f32
    {
        return Some(source.to_vec());
    }
    if finite
        && left.fract() == 0.0
        && top.fract() == 0.0
        && right - left == destination_size.0 as f32
        && bottom - top == destination_size.1 as f32
    {
        return Some(integer_crop::<CHANNELS>(
            source,
            source_size.0,
            destination_size,
            (left as u32, top as u32),
            destination_dims,
        ));
    }
    None
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn integer_crop<const CHANNELS: usize>(
    source: &[u8],
    source_width: u32,
    destination_size: (u32, u32),
    source_origin: (u32, u32),
    destination_dims: CheckedDims,
) -> Vec<u8> {
    let (destination_width, destination_height) = destination_size;
    let mut output = destination_dims.alloc_buffer();
    let source_stride = source_width as usize * CHANNELS;
    let destination_stride = destination_width as usize * CHANNELS;
    let source_x = source_origin.0 as usize * CHANNELS;
    if destination_stride == 0 {
        return output;
    }
    for destination_y in 0..destination_height as usize {
        let source_y = source_origin.1.saturating_add(destination_y as u32) as usize;
        let source_start = source_y * source_stride + source_x;
        let destination_start = destination_y * destination_stride;
        output[destination_start..destination_start + destination_stride]
            .copy_from_slice(&source[source_start..source_start + destination_stride]);
    }
    output
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn nearest<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    destination_dims: CheckedDims,
) -> Vec<u8> {
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    let mut output = destination_dims.alloc_buffer();
    let scale_x = f64::from(source_width) / f64::from(destination_width);
    let scale_y = f64::from(source_height) / f64::from(destination_height);
    let mut x_table = Vec::with_capacity(destination_width as usize);
    let mut source_x = scale_x * 0.5;
    for _ in 0..destination_width {
        let index = source_x as u32;
        x_table.push(if index >= source_width {
            source_width - 1
        } else {
            index
        });
        source_x += scale_x;
    }
    let source_stride = source_width as usize * CHANNELS;
    let destination_stride = destination_width as usize * CHANNELS;
    let mut source_y = scale_y * 0.5;
    for destination_y in 0..destination_height as usize {
        let source_y_index = if source_y >= f64::from(source_height) {
            source_height - 1
        } else {
            source_y as u32
        } as usize;
        let source_row = source_y_index * source_stride;
        let destination_row = destination_y * destination_stride;
        for (destination_x, &source_x) in x_table.iter().enumerate() {
            let source_start = source_row + source_x as usize * CHANNELS;
            let destination_start = destination_row + destination_x * CHANNELS;
            output[destination_start..destination_start + CHANNELS]
                .copy_from_slice(&source[source_start..source_start + CHANNELS]);
        }
        source_y += scale_y;
    }
    output
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn nearest_boxed<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
    destination_dims: CheckedDims,
) -> Vec<u8> {
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    let (left, top, right, bottom) = bounds;
    let (left, top, right, bottom) = (left as f32, top as f32, right as f32, bottom as f32);
    let scale_x = f64::from(right - left) / f64::from(destination_width);
    let scale_y = f64::from(bottom - top) / f64::from(destination_height);
    let last_x = f64::from(source_width - 1);
    let last_y = f64::from(source_height - 1);
    let source_stride = source_width as usize * CHANNELS;
    let destination_stride = destination_width as usize * CHANNELS;
    let mut output = destination_dims.alloc_buffer();
    let mut source_y = f64::from(top) + scale_y * 0.5;
    for destination_y in 0..destination_height as usize {
        let source_y_index = source_y.floor().clamp(0.0, last_y) as usize;
        let source_row = source_y_index * source_stride;
        let destination_row = destination_y * destination_stride;
        let mut source_x = f64::from(left) + scale_x * 0.5;
        for destination_x in 0..destination_width as usize {
            let source_x_index = source_x.floor().clamp(0.0, last_x) as usize;
            let source_start = source_row + source_x_index * CHANNELS;
            let destination_start = destination_row + destination_x * CHANNELS;
            output[destination_start..destination_start + CHANNELS]
                .copy_from_slice(&source[source_start..source_start + CHANNELS]);
            source_x += scale_x;
        }
        source_y += scale_y;
    }
    output
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn filtered<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: Bounds,
    sparse_source_rows: Option<&[bool]>,
) -> Result<Vec<u8>, PilError> {
    match bounds {
        Some(bounds) => filtered_boxed::<CHANNELS>(
            source,
            source_size,
            destination_size,
            filter,
            bounds,
            sparse_source_rows,
        ),
        None => filtered_standard::<CHANNELS>(source, source_size, destination_size, filter),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn filtered_standard<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    let horizontal = precompute_coeffs(destination_size.0, source_size.0, filter);
    let vertical = precompute_coeffs(destination_size.1, source_size.1, filter);
    filtered_standard_with_coefficients::<CHANNELS>(
        source,
        source_size,
        destination_size,
        &horizontal,
        &vertical,
    )
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn filtered_standard_with_coefficients<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Result<Vec<u8>, PilError> {
    if source_size.0 != destination_size.0
        && source_size.1 != destination_size.1
        && !super::coefficients::horizontal_first(source_size, destination_size)
    {
        let source_dims = CheckedDims::new_allow_empty(
            source_size.0,
            source_size.1,
            checked_channels::<CHANNELS>()?,
        )?;
        let source =
            transpose_samples::<CHANNELS>(source, source_size.1, source_size.0, source_dims);
        let transposed = filtered_with_coefficients::<CHANNELS>(ResizePlan {
            source: &source,
            sparse_source_rows: None,
            source_row_offset: 0,
            source_size: (source_size.1, source_size.0),
            destination_size: (destination_size.1, destination_size.0),
            horizontal: vertical,
            vertical: horizontal,
            need_horizontal: true,
            need_vertical: true,
        })?;
        let output_dims = CheckedDims::new(
            destination_size.0,
            destination_size.1,
            checked_channels::<CHANNELS>()?,
        )?;
        return Ok(transpose_samples::<CHANNELS>(
            &transposed,
            destination_size.0,
            destination_size.1,
            output_dims,
        ));
    }
    filtered_with_coefficients::<CHANNELS>(ResizePlan {
        source,
        sparse_source_rows: None,
        source_row_offset: 0,
        source_size,
        destination_size,
        horizontal,
        vertical,
        need_horizontal: true,
        need_vertical: true,
    })
}

/// Writes a standard filtered resize into a caller-owned native-byte window.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn filtered_standard_into<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
    output: &mut [u8],
) -> Result<(), PilError> {
    let channels = checked_channels::<CHANNELS>()?;
    let source_dims = CheckedDims::new_allow_empty(source_size.0, source_size.1, channels)?;
    let output_dims = CheckedDims::new(destination_size.0, destination_size.1, channels)?;
    if source.len() != source_dims.total_bytes() || output.len() != output_dims.total_bytes() {
        return Err(PilError::InternalError(
            "byte-sample Resize window received an invalid buffer".into(),
        ));
    }
    if source_size.0 != destination_size.0
        && source_size.1 != destination_size.1
        && !super::coefficients::horizontal_first(source_size, destination_size)
    {
        let resized = filtered_standard_with_coefficients::<CHANNELS>(
            source,
            source_size,
            destination_size,
            horizontal,
            vertical,
        )?;
        output.copy_from_slice(&resized);
        return Ok(());
    }
    filtered_with_coefficients_into::<CHANNELS>(
        ResizePlan {
            source,
            sparse_source_rows: None,
            source_row_offset: 0,
            source_size,
            destination_size,
            horizontal,
            vertical,
            need_horizontal: true,
            need_vertical: true,
        },
        output,
    )
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn filtered_boxed<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    sparse_source_rows: Option<&[bool]>,
) -> Result<Vec<u8>, PilError> {
    if source_size.0 != destination_size.0
        && source_size.1 != destination_size.1
        && !super::coefficients::horizontal_first(source_size, destination_size)
    {
        let dimensions = CheckedDims::new_allow_empty(
            source_size.0,
            source_size.1,
            checked_channels::<CHANNELS>()?,
        )?;
        let transposed =
            transpose_samples::<CHANNELS>(source, source_size.1, source_size.0, dimensions);
        let (left, top, right, bottom) = bounds;
        let output = filtered_boxed::<CHANNELS>(
            &transposed,
            (source_size.1, source_size.0),
            (destination_size.1, destination_size.0),
            filter,
            (top, left, bottom, right),
            None,
        )?;
        let output_dims = CheckedDims::new(
            destination_size.0,
            destination_size.1,
            checked_channels::<CHANNELS>()?,
        )?;
        return Ok(transpose_samples::<CHANNELS>(
            &output,
            destination_size.0,
            destination_size.1,
            output_dims,
        ));
    }
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    let (left, top, right, bottom) = bounds;
    let horizontal =
        precompute_coeffs_boxed_for_filter(destination_width, source_width, left, right, filter);
    let mut vertical =
        precompute_coeffs_boxed_for_filter(destination_height, source_height, top, bottom, filter);
    let (left, top, right, bottom) = (left as f32, top as f32, right as f32, bottom as f32);
    let need_horizontal =
        destination_width != source_width || left != 0.0 || right != destination_width as f32;
    let need_vertical =
        destination_height != source_height || top != 0.0 || bottom != destination_height as f32;
    let (first_source_row, intermediate_rows) = if need_vertical {
        compact_resize_coeffs_to_source_span(&mut vertical, source_height)
            .unwrap_or((0, source_height))
    } else {
        (0, source_height)
    };
    let row_stride = source_width as usize * CHANNELS;
    let source_start = first_source_row as usize * row_stride;
    let source_len = intermediate_rows as usize * row_stride;
    let source_span = &source[source_start..source_start + source_len];
    let sparse_source_rows = sparse_source_rows.filter(|rows| rows.len() == source_height as usize);
    filtered_with_coefficients::<CHANNELS>(ResizePlan {
        source: source_span,
        sparse_source_rows,
        source_row_offset: first_source_row,
        source_size: (source_width, intermediate_rows),
        destination_size,
        horizontal: &horizontal,
        vertical: &vertical,
        need_horizontal,
        need_vertical,
    })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn filtered_with_coefficients<const CHANNELS: usize>(
    plan: ResizePlan<'_>,
) -> Result<Vec<u8>, PilError> {
    let output_dims = CheckedDims::new(
        plan.destination_size.0,
        plan.destination_size.1,
        checked_channels::<CHANNELS>()?,
    )?;
    let mut output = output_dims.alloc_buffer();
    filtered_with_coefficients_into::<CHANNELS>(plan, &mut output)?;
    Ok(output)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn filtered_with_coefficients_into<const CHANNELS: usize>(
    plan: ResizePlan<'_>,
    output: &mut [u8],
) -> Result<(), PilError> {
    let (source_width, source_height) = plan.source_size;
    let (destination_width, destination_height) = plan.destination_size;
    let channels = checked_channels::<CHANNELS>()?;
    let output_dims = CheckedDims::new(destination_width, destination_height, channels)?;
    let intermediate_dims = CheckedDims::new(destination_width, source_height, channels)?;
    if output.len() != output_dims.total_bytes() {
        return Err(PilError::InternalError(
            "byte-sample Resize output window has an invalid shape".into(),
        ));
    }
    let mut intermediate = intermediate_dims.alloc_buffer();
    if plan.need_horizontal {
        for source_y in 0..source_height as usize {
            let source_start = source_y * source_width as usize * CHANNELS;
            let output_start = source_y * destination_width as usize * CHANNELS;
            let output_row = &mut intermediate
                [output_start..output_start + destination_width as usize * CHANNELS];
            let source_row_is_nonzero = plan.sparse_source_rows.map_or(true, |rows| {
                sparse_source_row_is_nonzero(rows, plan.source_row_offset, source_y)
            });
            if source_row_is_nonzero {
                horizontal_row::<CHANNELS>(
                    &plan.source[source_start..source_start + source_width as usize * CHANNELS],
                    plan.horizontal,
                    destination_width,
                    output_row,
                );
            } else {
                output_row.fill(0);
            }
        }
    } else {
        intermediate.copy_from_slice(plan.source);
    }
    if !plan.need_vertical {
        output.copy_from_slice(&intermediate);
    } else if let Some(sparse_source_rows) = plan.sparse_source_rows {
        vertical_row_major::<CHANNELS>(
            &intermediate,
            destination_width as usize,
            destination_height as usize,
            plan.vertical,
            Some((sparse_source_rows, plan.source_row_offset)),
            output,
        );
    } else if should_transpose_vertical(source_height, destination_width) {
        let transposed = transpose_samples::<CHANNELS>(
            &intermediate,
            source_height,
            destination_width,
            intermediate_dims,
        );
        vertical_transposed::<CHANNELS>(
            &transposed,
            source_height as usize,
            destination_width as usize,
            destination_height as usize,
            plan.vertical,
            output,
        );
    } else {
        vertical_row_major::<CHANNELS>(
            &intermediate,
            destination_width as usize,
            destination_height as usize,
            plan.vertical,
            None,
            output,
        );
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn horizontal_row<const CHANNELS: usize>(
    source: &[u8],
    coefficients: &FilterCoeffs,
    output_width: u32,
    output: &mut [u8],
) {
    if CHANNELS == 3 {
        crate::ops::pil_resize::horizontal_pass_rgb_row(source, coefficients, output_width, output);
        return;
    }
    for output_x in 0..output_width as usize {
        let weights = weights(coefficients, output_x);
        if weights.is_empty() {
            continue;
        }
        let first_source = coefficients.xmin[output_x] as usize * CHANNELS;
        for channel in 0..CHANNELS {
            let mut sum = 0_i64;
            for (tap, &weight) in weights.iter().enumerate() {
                sum += i64::from(source[first_source + tap * CHANNELS + channel]) * weight;
            }
            output[output_x * CHANNELS + channel] = fixed_point_to_u8(sum);
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_row_major<const CHANNELS: usize>(
    intermediate: &[u8],
    output_width: usize,
    output_height: usize,
    coefficients: &FilterCoeffs,
    sparse_source_rows: Option<(&[bool], u32)>,
    output: &mut [u8],
) {
    let row_stride = output_width * CHANNELS;
    for output_y in 0..output_height {
        let output_start = output_y * row_stride;
        let output_row = &mut output[output_start..output_start + row_stride];
        if CHANNELS == 3 {
            if sparse_source_rows.is_some_and(|(rows, offset)| {
                !vertical_span_has_nonzero_source(coefficients, offset, rows, output_y)
            }) {
                output_row.fill(0);
            } else {
                crate::ops::pil_resize::vertical_pass_rgb_row(
                    intermediate,
                    output_width as u32,
                    coefficients,
                    output_y,
                    output_row,
                );
            }
            continue;
        }
        let weights = weights(coefficients, output_y);
        if weights.is_empty() {
            output_row.fill(0);
            continue;
        }
        let first_source = coefficients.xmin[output_y] as usize * row_stride;
        for output_x in 0..output_width {
            for channel in 0..CHANNELS {
                let mut sum = 0_i64;
                for (tap, &weight) in weights.iter().enumerate() {
                    let source = first_source + tap * row_stride + output_x * CHANNELS + channel;
                    sum += i64::from(intermediate[source]) * weight;
                }
                output_row[output_x * CHANNELS + channel] = fixed_point_to_u8(sum);
            }
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn sparse_source_row_is_nonzero(
    nonzero_source_rows: &[bool],
    first_source_row: u32,
    relative_source_row: usize,
) -> bool {
    let Ok(first_source_row) = usize::try_from(first_source_row) else {
        return true;
    };
    let Some(source_row) = first_source_row.checked_add(relative_source_row) else {
        return true;
    };
    nonzero_source_rows.get(source_row).copied().unwrap_or(true)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_span_has_nonzero_source(
    coefficients: &FilterCoeffs,
    first_source_row: u32,
    nonzero_source_rows: &[bool],
    output_y: usize,
) -> bool {
    let Some(&first_relative_row) = coefficients.xmin.get(output_y) else {
        return true;
    };
    let Some(&tap_count) = coefficients.count.get(output_y) else {
        return true;
    };
    let Ok(first_relative_row) = usize::try_from(first_relative_row) else {
        return true;
    };
    let Ok(first_source_row) = usize::try_from(first_source_row) else {
        return true;
    };
    let Some(first_absolute_row) = first_source_row.checked_add(first_relative_row) else {
        return true;
    };
    for tap in 0..tap_count {
        let Some(source_y) = first_absolute_row.checked_add(tap) else {
            return true;
        };
        let Some(&is_nonzero) = nonzero_source_rows.get(source_y) else {
            return true;
        };
        if is_nonzero {
            return true;
        }
    }
    false
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_transposed<const CHANNELS: usize>(
    intermediate: &[u8],
    source_height: usize,
    output_width: usize,
    output_height: usize,
    coefficients: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width * CHANNELS;
    for output_y in 0..output_height {
        let weights = weights(coefficients, output_y);
        let output_start = output_y * output_stride;
        let output_row = &mut output[output_start..output_start + output_stride];
        if weights.is_empty() {
            output_row.fill(0);
            continue;
        }
        let first_source_y = coefficients.xmin[output_y] as usize;
        for output_x in 0..output_width {
            let source_column = output_x * source_height + first_source_y;
            for channel in 0..CHANNELS {
                let mut sum = 0_i64;
                for (tap, &weight) in weights.iter().enumerate() {
                    let source = (source_column + tap) * CHANNELS + channel;
                    sum += i64::from(intermediate[source]) * weight;
                }
                output_row[output_x * CHANNELS + channel] = fixed_point_to_u8(sum);
            }
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn transpose_samples<const CHANNELS: usize>(
    source: &[u8],
    source_height: u32,
    output_width: u32,
    dimensions: CheckedDims,
) -> Vec<u8> {
    let mut transposed = dimensions.alloc_buffer();
    if CHANNELS == 3 {
        const TILE_WIDTH: usize = 32;
        const TILE_HEIGHT: usize = 32;
        let source_height = source_height as usize;
        let output_width = output_width as usize;
        let source_stride = output_width * CHANNELS;
        let destination_stride = source_height * CHANNELS;
        for tile_y in (0..source_height).step_by(TILE_HEIGHT) {
            let end_y = (tile_y + TILE_HEIGHT).min(source_height);
            for tile_x in (0..output_width).step_by(TILE_WIDTH) {
                let end_x = (tile_x + TILE_WIDTH).min(output_width);
                for source_y in tile_y..end_y {
                    let source_row = source_y * source_stride;
                    for output_x in tile_x..end_x {
                        let source_start = source_row + output_x * CHANNELS;
                        let destination_start = output_x * destination_stride + source_y * CHANNELS;
                        transposed[destination_start..destination_start + CHANNELS]
                            .copy_from_slice(&source[source_start..source_start + CHANNELS]);
                    }
                }
            }
        }
        return transposed;
    }
    for source_y in 0..source_height as usize {
        let source_row = source_y * output_width as usize * CHANNELS;
        for output_x in 0..output_width as usize {
            let source_start = source_row + output_x * CHANNELS;
            let destination_start = (output_x * source_height as usize + source_y) * CHANNELS;
            transposed[destination_start..destination_start + CHANNELS]
                .copy_from_slice(&source[source_start..source_start + CHANNELS]);
        }
    }
    transposed
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn weights(coefficients: &FilterCoeffs, index: usize) -> &[i64] {
    let start = coefficients.offsets[index];
    &coefficients.weights[start..start + coefficients.count[index]]
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn should_transpose_vertical(source_rows: u32, output_width: u32) -> bool {
    source_rows > 1
        && output_width > 1
        && (source_rows as usize).saturating_mul(output_width as usize) >= 512 * 512
}
