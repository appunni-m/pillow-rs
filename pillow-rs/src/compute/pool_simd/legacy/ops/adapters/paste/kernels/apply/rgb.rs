//! RGB masked Paste specialization.

use super::NativePasteApplyContext;
#[cfg(feature = "parallel")]
use super::SIMD_PASTE_PARALLEL_PIXEL_THRESHOLD;
use super::native_paste_rgb_l_masked_row;

#[inline]
fn record_rgb_work(region_row_bytes: usize, region_height: usize, allow_short_masked_tail: bool) {
    let vector_len16 = region_row_bytes / 16 * 16;
    let vector_len8 = region_row_bytes / 8 * 8;
    let vector_blocks_per_row = (vector_len16 / 16 + (vector_len8 - vector_len16) / 8)
        + usize::from(allow_short_masked_tail && vector_len8 < region_row_bytes);
    let vector_blocks = vector_blocks_per_row.saturating_mul(region_height) as u64;
    let scalar_tail = if allow_short_masked_tail {
        0
    } else {
        (region_row_bytes - vector_len8).saturating_mul(region_height) as u64
    };
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
}

pub(super) fn apply(context: &mut NativePasteApplyContext<'_>) -> bool {
    let plan = context.plan;
    let region = plan.region;
    let (_, mask_row_stride) = context
        .mask_layout
        .expect("mode dispatch admitted a supported L mask");
    let destination_row_stride = context.destination_row_stride;
    let source_row_stride = context.source_row_stride;
    let region_row_bytes = context.region_row_bytes;
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
    let Some(destination_left) = region.destination_left.checked_mul(3) else {
        return false;
    };
    let Some(destination_right) = region
        .destination_left
        .checked_add(region.width)
        .and_then(|right| right.checked_mul(3))
    else {
        return false;
    };
    let mut used_neon = false;
    let mut apply_rgb_row = |row_index: usize, destination_row: &mut [u8]| {
        let source_y = region.source_top + row_index;
        let source_row_start = source_y * source_row_stride + region.source_left * 3;
        let source_row = &source[source_row_start..source_row_start + region_row_bytes];
        let mask_row_start = source_y * mask_row_stride + region.source_left;
        let mask_row = &mask[mask_row_start..mask_row_start + region.width];
        used_neon |= native_paste_rgb_l_masked_row(
            source_row,
            &mut destination_row[destination_left..destination_right],
            mask_row,
            allow_short_masked_tail,
        );
    };

    #[cfg(feature = "parallel")]
    if region.height > 1
        && region.width.saturating_mul(region.height) >= SIMD_PASTE_PARALLEL_PIXEL_THRESHOLD
    {
        super::super::super::super::simd_rows_mut_serial!(
            destination_rows,
            destination_row_stride,
            region.height,
            |_row_start, _row_end, row, destination_row| {
                apply_rgb_row(row as usize, destination_row);
            }
        );
    } else {
        for (row_index, destination_row) in destination_rows
            .chunks_exact_mut(destination_row_stride)
            .enumerate()
        {
            apply_rgb_row(row_index, destination_row);
        }
    }

    #[cfg(not(feature = "parallel"))]
    for (row_index, destination_row) in destination_rows
        .chunks_exact_mut(destination_row_stride)
        .enumerate()
    {
        apply_rgb_row(row_index, destination_row);
    }

    record_rgb_work(region_row_bytes, region.height, allow_short_masked_tail);
    crate::compute::record_pipeline_operation_path(if used_neon {
        "neon-rgb-l-blend"
    } else {
        "vector"
    });
    true
}
