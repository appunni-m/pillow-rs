//! SIMD Transpose family admission and mode/orientation selection.

#[cfg(target_arch = "x86_64")]
mod kernels;

use super::{
    CheckedDims, copy_native_bytes, mirror_native_row, transpose_gathered_row,
    transpose_native_tiled,
};
#[cfg(feature = "parallel")]
use super::{
    transpose_native_collect, transpose_native_collect_admitted, transpose_native_odd_collect,
    transpose_native_odd_collect_admitted,
};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, TransposeMethod};
use crate::raster::DynamicImage;

#[inline]
#[cfg(target_arch = "x86_64")]
pub(super) fn rgba_avx2(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    method: &TransposeMethod,
) -> Option<(u64, u64)> {
    if channels != 4
        || !matches!(
            method,
            TransposeMethod::Rotate90
                | TransposeMethod::Rotate270
                | TransposeMethod::Transpose
                | TransposeMethod::Transverse
        )
    {
        return None;
    }

    let reverse_x = matches!(
        method,
        TransposeMethod::Rotate90 | TransposeMethod::Transverse
    );
    let reverse_y = matches!(
        method,
        TransposeMethod::Rotate270 | TransposeMethod::Transverse
    );
    kernels::transpose_rgba(source, output, width, height, reverse_x, reverse_y)
}

#[cfg(feature = "parallel")]
fn try_parallel_collect(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    total_bytes: usize,
    method: &TransposeMethod,
    output_dims: &CheckedDims,
) -> Option<(Vec<u8>, u64, u64)> {
    let reverse_x = matches!(
        method,
        TransposeMethod::Rotate90 | TransposeMethod::Transverse
    );
    let reverse_y = matches!(
        method,
        TransposeMethod::Rotate270 | TransposeMethod::Transverse
    );
    if transpose_native_collect_admitted(width, height, channels, total_bytes, method) {
        let (output, vector_blocks) = match channels {
            3 => transpose_native_collect::<3, 48>(
                source,
                width,
                height,
                reverse_x,
                reverse_y,
                output_dims,
            ),
            4 => transpose_native_collect::<4, 64>(
                source,
                width,
                height,
                reverse_x,
                reverse_y,
                output_dims,
            ),
            _ => unreachable!("collection requires three or four native bytes"),
        }?;
        return Some((output, vector_blocks, 0));
    }
    if transpose_native_odd_collect_admitted(width, height, channels, total_bytes, method) {
        return transpose_native_odd_collect(
            source,
            width,
            height,
            reverse_x,
            reverse_y,
            output_dims,
        );
    }
    None
}

fn transpose_same_dimensions(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    method: &TransposeMethod,
) -> Option<(u64, u64)> {
    let row_bytes = width.checked_mul(channels)?;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..height {
        let (source_row, mirror) = match method {
            TransposeMethod::FlipLeftRight => (y, true),
            TransposeMethod::FlipTopBottom => (height - 1 - y, false),
            TransposeMethod::Rotate180 => (height - 1 - y, true),
            _ => return None,
        };
        let source_start = source_row.checked_mul(row_bytes)?;
        let output_start = y.checked_mul(row_bytes)?;
        let source_row = source.get(source_start..source_start + row_bytes)?;
        let output_row = output.get_mut(output_start..output_start + row_bytes)?;
        let (blocks, tail) = if mirror {
            mirror_native_row(source_row, output_row, width, channels)?
        } else {
            copy_native_bytes(source_row, output_row)?
        };
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Some((vector_blocks, scalar_tail))
}

fn transpose_tiled_axis_swap(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    method: &TransposeMethod,
) -> Option<(u64, u64)> {
    let tile = match channels {
        1 => 16,
        2 => 8,
        3 | 4 => 4,
        _ => return None,
    };
    if width < tile || height < tile {
        return None;
    }
    let reverse_x = matches!(
        method,
        TransposeMethod::Rotate90 | TransposeMethod::Transverse
    );
    let reverse_y = matches!(
        method,
        TransposeMethod::Rotate270 | TransposeMethod::Transverse
    );
    Some(match channels {
        1 => transpose_native_tiled::<1, 16>(source, output, width, height, reverse_x, reverse_y),
        2 => transpose_native_tiled::<2, 8>(source, output, width, height, reverse_x, reverse_y),
        3 => transpose_native_tiled::<3, 4>(source, output, width, height, reverse_x, reverse_y),
        4 => transpose_native_tiled::<4, 4>(source, output, width, height, reverse_x, reverse_y),
        _ => return None,
    })
}

fn transpose_gathered_axis_swap(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    out_width: usize,
    out_height: usize,
    channels: usize,
    pixels: usize,
    method: &TransposeMethod,
) -> Option<(u64, u64)> {
    let source_row_bytes = width.checked_mul(channels)?;
    let output_row_bytes = out_width.checked_mul(channels)?;
    #[cfg(not(feature = "parallel"))]
    let _ = pixels;
    #[cfg(feature = "parallel")]
    if pixels >= 256 * 1024 {
        super::simd_rows_mut_serial!(
            output,
            output_row_bytes,
            out_height,
            |_row_start, _row_end, output_row, output_row_slice| {
                let (source_x, reverse_source_y) =
                    gathered_source_column(method, width, output_row as usize);
                let _ = transpose_gathered_row(
                    source,
                    output_row_slice,
                    source_row_bytes,
                    height,
                    channels,
                    source_x,
                    reverse_source_y,
                );
            }
        );
        return Some((
            (out_height as u64).saturating_mul((output_row_bytes / 16) as u64),
            (out_height as u64).saturating_mul((output_row_bytes % 16) as u64),
        ));
    }

    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_row in 0..out_height {
        let (source_x, reverse_source_y) = gathered_source_column(method, width, output_row);
        let output_start = output_row.checked_mul(output_row_bytes)?;
        let (blocks, tail) = transpose_gathered_row(
            source,
            output.get_mut(output_start..output_start + output_row_bytes)?,
            source_row_bytes,
            height,
            channels,
            source_x,
            reverse_source_y,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Some((vector_blocks, scalar_tail))
}

#[inline(always)]
fn gathered_source_column(
    method: &TransposeMethod,
    width: usize,
    output_row: usize,
) -> (usize, bool) {
    match method {
        TransposeMethod::Rotate90 => (width - 1 - output_row, false),
        TransposeMethod::Rotate270 | TransposeMethod::Transpose => {
            (output_row, matches!(method, TransposeMethod::Rotate270))
        }
        TransposeMethod::Transverse => (width - 1 - output_row, true),
        _ => unreachable!("same-dimension transpose handled above"),
    }
}

pub(super) fn native_transpose_bytes(
    bytes: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    method: TransposeMethod,
) -> Option<(Vec<u8>, u32, u32, u64, u64, &'static str)> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let pixels = width.checked_mul(height)?;
    let total_bytes = pixels.checked_mul(channels)?;
    let source = bytes.get(..total_bytes)?;
    let (out_width, out_height) = match method {
        TransposeMethod::FlipLeftRight
        | TransposeMethod::FlipTopBottom
        | TransposeMethod::Rotate180 => (width, height),
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => (height, width),
    };
    let output_dims = CheckedDims::new_allow_empty(
        u32::try_from(out_width).ok()?,
        u32::try_from(out_height).ok()?,
        u8::try_from(channels).ok()?,
    )
    .ok()?;
    #[cfg(feature = "parallel")]
    if let Some((output, vector_blocks, scalar_tail)) = try_parallel_collect(
        source,
        width,
        height,
        channels,
        total_bytes,
        &method,
        &output_dims,
    ) {
        return Some((
            output,
            out_width as u32,
            out_height as u32,
            vector_blocks,
            scalar_tail,
            "native-copy",
        ));
    }

    let allocation_started = crate::compute::pipeline_phase_timestamp();
    let mut output = output_dims.alloc_buffer();
    crate::compute::record_transpose_output_allocation_timing(allocation_started);
    #[cfg(target_arch = "x86_64")]
    {
        let avx2_started = crate::compute::pipeline_phase_timestamp();
        if let Some((blocks, tail)) =
            rgba_avx2(source, &mut output, width, height, channels, &method)
        {
            crate::compute::record_transpose_avx2_executor_timing(avx2_started);
            return Some((
                output,
                out_width as u32,
                out_height as u32,
                blocks,
                tail,
                "avx2-transpose",
            ));
        }
    }

    let (vector_blocks, scalar_tail) = match method {
        TransposeMethod::FlipLeftRight
        | TransposeMethod::FlipTopBottom
        | TransposeMethod::Rotate180 => {
            transpose_same_dimensions(source, &mut output, width, height, channels, &method)?
        }
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => {
            if let Some(counts) =
                transpose_tiled_axis_swap(source, &mut output, width, height, channels, &method)
            {
                counts
            } else {
                transpose_gathered_axis_swap(
                    source,
                    &mut output,
                    width,
                    height,
                    out_width,
                    out_height,
                    channels,
                    pixels,
                    &method,
                )?
            }
        }
    };
    Some((
        output,
        out_width as u32,
        out_height as u32,
        vector_blocks,
        scalar_tail,
        "native-copy",
    ))
}

/// Execute Pillow's native transpose family adapter and record its selected kernel path.
pub(crate) fn simd_transpose(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Transpose { method } = op else {
        return Err(PilError::ValueError("expected Transpose op".into()));
    };
    if let Some((result, vector_blocks, scalar_tail, kernel_path)) =
        super::native_transpose(img, mode, method.clone())
    {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path(kernel_path);
        return Ok(result);
    }
    Err(super::simd_unsupported("Transpose"))
}
