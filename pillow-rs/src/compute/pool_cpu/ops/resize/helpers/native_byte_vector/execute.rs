//! Native-byte Resize orchestration for an already selected architecture.

use super::super::byte_samples::transpose_samples;
use super::{bounded_i32, boxed, convolution, plan, telemetry};

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::resize::helpers::coefficients::{
    byte_coefficients_fit_i32, byte_coefficients_fit_source, horizontal_first,
};
use crate::error::PilError;
use crate::ops::pil_resize::precompute_coeffs;
use crate::pipeline::ResampleFilter;

pub(crate) fn execute_boxed<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    execute_boxed_with_row_facts::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>(
        source,
        source_size,
        destination_size,
        bounds,
        filter,
        None,
    )
}

pub(crate) fn execute_boxed_with_sparse_rows<
    const CHANNELS: usize,
    const PREMULTIPLY_STRAIGHT_ALPHA: bool,
>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
    sparse_source_rows: &[bool],
) -> Result<Vec<u8>, PilError> {
    execute_boxed_with_row_facts::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>(
        source,
        source_size,
        destination_size,
        bounds,
        filter,
        Some(sparse_source_rows),
    )
}

fn execute_boxed_with_row_facts<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
    sparse_source_rows: Option<&[bool]>,
) -> Result<Vec<u8>, PilError> {
    if !needs_vertical_first(source_size, destination_size)
        || matches!(filter, ResampleFilter::Nearest)
    {
        return boxed::execute::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>(
            source,
            source_size,
            destination_size,
            bounds,
            filter,
            sparse_source_rows,
        );
    }
    validate_sparse_rows(sparse_source_rows, source_size.1)?;
    let transposed_source = transpose_source::<CHANNELS>(
        source,
        source_size,
        "boxed native-byte Resize received invalid source buffer",
    )?;
    let transposed_sparse_rows = sparse_source_rows
        .map(|_| nonzero_rows::<CHANNELS>(&transposed_source, source_size.1, source_size.0))
        .transpose()?;
    let (left, top, right, bottom) = bounds;
    let transposed_output = boxed::execute::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>(
        &transposed_source,
        (source_size.1, source_size.0),
        (destination_size.1, destination_size.0),
        (top, left, bottom, right),
        filter,
        transposed_sparse_rows.as_deref(),
    )?;
    let output_dims = CheckedDims::new(
        destination_size.0,
        destination_size.1,
        checked_channel_count::<CHANNELS>()?,
    )?;
    Ok(transpose_samples::<CHANNELS>(
        &transposed_output,
        destination_size.0,
        destination_size.1,
        output_dims,
    ))
}

fn validate_sparse_rows(sparse_rows: Option<&[bool]>, height: u32) -> Result<(), PilError> {
    if sparse_rows.is_some_and(|rows| usize::try_from(height).ok() != Some(rows.len())) {
        Err(PilError::InternalError(
            "boxed native-byte Resize received invalid sparse row facts".into(),
        ))
    } else {
        Ok(())
    }
}

fn checked_channel_count<const CHANNELS: usize>() -> Result<u8, PilError> {
    u8::try_from(CHANNELS)
        .ok()
        .filter(|channels| (1..=4).contains(channels))
        .ok_or_else(|| {
            PilError::DimensionError("native-byte Resize requires one to four channels".into())
        })
}

fn transpose_source<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    invalid_source: &'static str,
) -> Result<Vec<u8>, PilError> {
    let dimensions = CheckedDims::new(
        source_size.0,
        source_size.1,
        checked_channel_count::<CHANNELS>()?,
    )?;
    if source.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(invalid_source.into()));
    }
    Ok(transpose_samples::<CHANNELS>(
        source,
        source_size.1,
        source_size.0,
        dimensions,
    ))
}

fn nonzero_rows<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<bool>, PilError> {
    let dimensions = CheckedDims::new(width, height, checked_channel_count::<CHANNELS>()?)?;
    if source.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "transposed native-byte Resize has an invalid source buffer".into(),
        ));
    }
    let row_stride = dimensions.row_stride();
    Ok(source
        .chunks_exact(row_stride)
        .map(|row| row.iter().any(|&sample| sample != 0))
        .collect())
}

fn needs_vertical_first(source: (u32, u32), destination: (u32, u32)) -> bool {
    source.0 != destination.0 && source.1 != destination.1 && !horizontal_first(source, destination)
}

fn execute_vertical_first<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    sparse_source_rows: Option<&[bool]>,
    output_dims: CheckedDims,
    output: &mut [u8],
) -> Result<(), PilError> {
    let transposed_source = transpose_source::<CHANNELS>(
        source,
        source_size,
        "filtered native-byte Resize received an invalid source buffer",
    )?;
    let transposed_sparse_rows = sparse_source_rows
        .map(|_| nonzero_rows::<CHANNELS>(&transposed_source, source_size.1, source_size.0))
        .transpose()?;
    let transposed_output_dims = CheckedDims::new(
        destination_size.1,
        destination_size.0,
        checked_channel_count::<CHANNELS>()?,
    )?;
    let mut transposed_output = transposed_output_dims.alloc_buffer();
    execute_into_with_row_facts::<CHANNELS, PREMULTIPLIED_ALPHA, USE_BOUNDED_I32>(
        &transposed_source,
        (source_size.1, source_size.0),
        (destination_size.1, destination_size.0),
        filter,
        transposed_sparse_rows.as_deref(),
        transposed_output_dims,
        &mut transposed_output,
    )?;
    let restored_output = transpose_samples::<CHANNELS>(
        &transposed_output,
        destination_size.0,
        destination_size.1,
        output_dims,
    );
    output.copy_from_slice(&restored_output);
    Ok(())
}

pub(crate) fn execute<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    execute_with_row_facts::<CHANNELS, PREMULTIPLIED_ALPHA, USE_BOUNDED_I32>(
        source,
        source_size,
        destination_size,
        filter,
        None,
    )
}

pub(crate) fn execute_with_sparse_rows<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    sparse_source_rows: &[bool],
) -> Result<Vec<u8>, PilError> {
    execute_with_row_facts::<CHANNELS, PREMULTIPLIED_ALPHA, USE_BOUNDED_I32>(
        source,
        source_size,
        destination_size,
        filter,
        Some(sparse_source_rows),
    )
}

fn execute_with_row_facts<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    sparse_source_rows: Option<&[bool]>,
) -> Result<Vec<u8>, PilError> {
    let output_dims = CheckedDims::new(destination_size.0, destination_size.1, CHANNELS as u8)?;
    let mut output = output_dims.alloc_buffer();
    execute_into_with_row_facts::<CHANNELS, PREMULTIPLIED_ALPHA, USE_BOUNDED_I32>(
        source,
        source_size,
        destination_size,
        filter,
        sparse_source_rows,
        output_dims,
        &mut output,
    )?;
    Ok(output)
}

pub(crate) fn execute_into<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    output: &mut [u8],
) -> Result<bool, PilError> {
    if matches!(filter, ResampleFilter::Nearest)
        || source_size.0 == 0
        || source_size.1 == 0
        || destination_size.0 == 0
        || destination_size.1 == 0
    {
        return Ok(false);
    }
    let output_dims = CheckedDims::new(destination_size.0, destination_size.1, CHANNELS as u8)?;
    execute_into_with_row_facts::<CHANNELS, PREMULTIPLIED_ALPHA, USE_BOUNDED_I32>(
        source,
        source_size,
        destination_size,
        filter,
        None,
        output_dims,
        output,
    )?;
    Ok(true)
}

fn execute_into_with_row_facts<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    sparse_source_rows: Option<&[bool]>,
    output_dims: CheckedDims,
    output: &mut [u8],
) -> Result<(), PilError> {
    validate_channels::<CHANNELS, PREMULTIPLIED_ALPHA, USE_BOUNDED_I32>()?;
    if matches!(filter, ResampleFilter::Nearest) {
        return Err(PilError::InternalError(
            "filtered native-byte Resize received the nearest filter".into(),
        ));
    }
    let source_dims = CheckedDims::new(source_size.0, source_size.1, CHANNELS as u8)?;
    let intermediate_dims = CheckedDims::new(destination_size.0, source_size.1, CHANNELS as u8)?;
    if source.len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "filtered native-byte Resize received an invalid source buffer".into(),
        ));
    }
    if output.len() != output_dims.total_bytes() {
        return Err(PilError::InternalError(
            "filtered native-byte Resize received an invalid output buffer".into(),
        ));
    }
    if sparse_source_rows
        .is_some_and(|rows| usize::try_from(source_size.1).ok() != Some(rows.len()))
    {
        return Err(PilError::InternalError(
            "native-byte Resize received invalid sparse row facts".into(),
        ));
    }
    if needs_vertical_first(source_size, destination_size) {
        return execute_vertical_first::<CHANNELS, PREMULTIPLIED_ALPHA, USE_BOUNDED_I32>(
            source,
            source_size,
            destination_size,
            filter,
            sparse_source_rows,
            output_dims,
            output,
        );
    }

    let horizontal = precompute_coeffs(destination_size.0, source_size.0, filter);
    let vertical = precompute_coeffs(destination_size.1, source_size.1, filter);
    let source_width = usize::try_from(source_size.0)
        .map_err(|_| PilError::DimensionError("Resize source width is too large".into()))?;
    let source_height = usize::try_from(source_size.1)
        .map_err(|_| PilError::DimensionError("Resize source height is too large".into()))?;
    let output_width = usize::try_from(destination_size.0)
        .map_err(|_| PilError::DimensionError("Resize width is too large".into()))?;
    let output_height = usize::try_from(destination_size.1)
        .map_err(|_| PilError::DimensionError("Resize height is too large".into()))?;
    validate_coefficients(&horizontal, &vertical, source_width, source_height)?;
    let plan = plan::build_resize_horizontal_plan::<CHANNELS>(&horizontal, output_width)
        .ok_or_else(invalid_kernel_plan)?;
    let source_stride = source_width
        .checked_mul(CHANNELS)
        .ok_or_else(invalid_kernel_plan)?;
    let intermediate_stride = output_width
        .checked_mul(CHANNELS)
        .ok_or_else(invalid_kernel_plan)?;
    let mut intermediate = intermediate_dims.alloc_buffer();
    let mut premultiplied_row = if PREMULTIPLIED_ALPHA {
        vec![0; source_stride]
    } else {
        Vec::new()
    };

    let (horizontal_blocks, horizontal_tail) =
        horizontal_pass::<CHANNELS, PREMULTIPLIED_ALPHA, USE_BOUNDED_I32>(
            source,
            source_height,
            source_stride,
            output_width,
            intermediate_stride,
            &horizontal,
            &plan,
            &mut premultiplied_row,
            &mut intermediate,
            sparse_source_rows,
        )
        .ok_or_else(invalid_kernel_plan)?;
    let (vertical_blocks, vertical_tail) =
        vertical_pass::<CHANNELS, PREMULTIPLIED_ALPHA, USE_BOUNDED_I32>(
            &intermediate,
            output_width,
            source_height,
            output_height,
            &vertical,
            output,
            sparse_source_rows,
        )
        .ok_or_else(invalid_kernel_plan)?;
    telemetry::record_vector_work(
        horizontal_blocks,
        vertical_blocks,
        horizontal_tail,
        vertical_tail,
    );
    Ok(())
}

fn validate_channels<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>() -> Result<(), PilError> {
    let has_valid_alpha_layout = !PREMULTIPLIED_ALPHA || CHANNELS == 2 || CHANNELS == 4;
    let has_valid_bounded_i32_layout = !USE_BOUNDED_I32
        || matches!(
            (CHANNELS, PREMULTIPLIED_ALPHA),
            (2, true) | (3, false) | (4, false) | (4, true)
        );
    if (1..=4).contains(&CHANNELS) && has_valid_alpha_layout && has_valid_bounded_i32_layout {
        Ok(())
    } else {
        Err(PilError::DimensionError(
            "native-byte Resize requires one to four channels and valid alpha layout".into(),
        ))
    }
}

fn validate_coefficients(
    horizontal: &crate::ops::pil_resize::FilterCoeffs,
    vertical: &crate::ops::pil_resize::FilterCoeffs,
    source_width: usize,
    source_height: usize,
) -> Result<(), PilError> {
    if byte_coefficients_fit_source(horizontal, source_width)
        && byte_coefficients_fit_source(vertical, source_height)
        && byte_coefficients_fit_i32(horizontal)
        && byte_coefficients_fit_i32(vertical)
    {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "selected native-byte Resize kernel received unsupported coefficients".into(),
        ))
    }
}

fn horizontal_pass<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>(
    source: &[u8],
    source_height: usize,
    source_stride: usize,
    output_width: usize,
    output_stride: usize,
    coefficients: &crate::ops::pil_resize::FilterCoeffs,
    plan: &plan::ResizeHorizontalPlan,
    premultiplied_row: &mut [u8],
    intermediate: &mut [u8],
    sparse_source_rows: Option<&[bool]>,
) -> Option<(u64, u64)> {
    let mut vector_blocks = 0_u64;
    let mut scalar_tail = 0_u64;
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let source_row = source.get(source_start..source_start.checked_add(source_stride)?)?;
        let output_start = source_y.checked_mul(output_stride)?;
        let output_row =
            intermediate.get_mut(output_start..output_start.checked_add(output_stride)?)?;
        if sparse_source_rows.is_some_and(|rows| !rows[source_y]) {
            output_row.fill(0);
            continue;
        }
        let (blocks, tail) = convolution::resize_horizontal_convolution_row::<
            CHANNELS,
            PREMULTIPLIED_ALPHA,
            USE_BOUNDED_I32,
        >(
            source_row,
            coefficients,
            plan,
            output_width,
            output_row,
            premultiplied_row,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Some((vector_blocks, scalar_tail))
}

fn vertical_pass<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    output_height: usize,
    coefficients: &crate::ops::pil_resize::FilterCoeffs,
    output: &mut [u8],
    sparse_source_rows: Option<&[bool]>,
) -> Option<(u64, u64)> {
    let mut vector_blocks = 0_u64;
    let mut scalar_tail = 0_u64;
    for output_y in 0..output_height {
        let output_start = output_y.checked_mul(output_width)?.checked_mul(CHANNELS)?;
        let row_len = output_width.checked_mul(CHANNELS)?;
        let output_row = output.get_mut(output_start..output_start.checked_add(row_len)?)?;
        if let Some(rows) = sparse_source_rows
            && !super::active_rows::vertical_row_touches_source(
                coefficients,
                source_height,
                0,
                rows,
                output_y,
            )?
        {
            output_row.fill(0);
            continue;
        }
        let (blocks, tail) = if USE_BOUNDED_I32 {
            bounded_i32::resize_vertical_bounded_i32_vector_row::<CHANNELS, PREMULTIPLIED_ALPHA>(
                intermediate,
                output_width,
                source_height,
                coefficients,
                output_y,
                output_row,
            )?
        } else {
            convolution::resize_vertical_vector_row::<CHANNELS, PREMULTIPLIED_ALPHA>(
                intermediate,
                output_width,
                source_height,
                coefficients,
                output_y,
                output_row,
            )?
        };
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Some((vector_blocks, scalar_tail))
}

fn invalid_kernel_plan() -> PilError {
    PilError::InternalError("selected native-byte Resize kernel rejected its plan".into())
}
