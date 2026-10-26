//! L masked Paste specialization.

use super::NativePasteApplyContext;
#[cfg(feature = "parallel")]
use super::SIMD_PASTE_PARALLEL_PIXEL_THRESHOLD;
use super::native_paste_l_masked_row;

pub(super) fn apply(context: &mut NativePasteApplyContext<'_>) -> bool {
    let plan = context.plan;
    let region = plan.region;
    let (_, mask_row_stride) = context
        .mask_layout
        .expect("mode dispatch admitted a supported L mask");
    let destination_row_stride = context.destination_row_stride;
    let source_row_stride = context.source_row_stride;
    let allow_short_masked_tail = context.allow_short_masked_tail;
    let source = context.source;
    let mask = context.mask;
    let destination = &mut *context.destination;

    let Some(destination_region_start) = region.destination_top.checked_mul(destination_row_stride)
    else {
        return false;
    };
    let Some(destination_region_len) = region.height.checked_mul(destination_row_stride) else {
        return false;
    };
    let Some(destination_region_end) = destination_region_start.checked_add(destination_region_len)
    else {
        return false;
    };
    let Some(destination_rows) =
        destination.get_mut(destination_region_start..destination_region_end)
    else {
        return false;
    };
    let mask = mask.expect("a validated L mask layout always has mask bytes");
    let destination_left = region.destination_left;
    let Some(destination_right) = destination_left.checked_add(region.width) else {
        return false;
    };
    let apply_luma_row = |row_index: usize, destination_row: &mut [u8]| {
        let source_y = region.source_top + row_index;
        let source_row_start = source_y * source_row_stride + region.source_left;
        let source_row = &source[source_row_start..source_row_start + region.width];
        let mask_row_start = source_y * mask_row_stride + region.source_left;
        let mask_row = &mask[mask_row_start..mask_row_start + region.width];
        native_paste_l_masked_row(
            source_row,
            &mut destination_row[destination_left..destination_right],
            mask_row,
            allow_short_masked_tail,
        )
    };

    let mut used_neon = false;
    #[cfg(feature = "parallel")]
    if region.height > 1
        && region.width.saturating_mul(region.height) >= SIMD_PASTE_PARALLEL_PIXEL_THRESHOLD
    {
        super::super::super::super::simd_rows_mut_serial!(
            destination_rows,
            destination_row_stride,
            region.height,
            |_row_start, _row_end, row, destination_row| {
                used_neon |= apply_luma_row(row as usize, destination_row);
            }
        );
    } else {
        for (row_index, destination_row) in destination_rows
            .chunks_exact_mut(destination_row_stride)
            .enumerate()
        {
            used_neon |= apply_luma_row(row_index, destination_row);
        }
    }

    #[cfg(not(feature = "parallel"))]
    for (row_index, destination_row) in destination_rows
        .chunks_exact_mut(destination_row_stride)
        .enumerate()
    {
        used_neon |= apply_luma_row(row_index, destination_row);
    }

    let (vector_blocks, scalar_tail) = if used_neon {
        (
            (region.width / 16).saturating_mul(region.height) as u64,
            (region.width % 16).saturating_mul(region.height) as u64,
        )
    } else {
        let vector_len16 = region.width / 16 * 16;
        let vector_len8 = region.width / 8 * 8;
        let vector_blocks_per_row = (vector_len16 / 16 + (vector_len8 - vector_len16) / 8)
            + usize::from(allow_short_masked_tail && vector_len8 < region.width);
        let vector_blocks = vector_blocks_per_row.saturating_mul(region.height) as u64;
        let scalar_tail = if allow_short_masked_tail {
            0
        } else {
            (region.width - vector_len8).saturating_mul(region.height) as u64
        };
        (vector_blocks, scalar_tail)
    };
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path(if used_neon { "neon-blend" } else { "vector" });
    true
}
