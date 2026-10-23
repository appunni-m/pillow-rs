//! CMYK L masked Paste specialization.

use super::NativePasteApplyContext;
use super::native_paste_cmyk_l_masked_row;

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
    let Some(source_x) = region.source_left.checked_mul(4) else {
        return false;
    };
    let Some(destination_left) = region.destination_left.checked_mul(4) else {
        return false;
    };
    let Some(destination_right) = destination_left.checked_add(region_row_bytes) else {
        return false;
    };
    let Some(mask_x_end) = region.source_left.checked_add(region.width) else {
        return false;
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let mut used_neon = false;

    for (row_index, destination_row) in destination_rows
        .chunks_exact_mut(destination_row_stride)
        .enumerate()
    {
        let Some(source_y) = region.source_top.checked_add(row_index) else {
            return false;
        };
        let Some(source_start) = source_y
            .checked_mul(source_row_stride)
            .and_then(|row| row.checked_add(source_x))
        else {
            return false;
        };
        let Some(source_end) = source_start.checked_add(region_row_bytes) else {
            return false;
        };
        let Some(source_row) = source.get(source_start..source_end) else {
            return false;
        };
        let Some(mask_start) = source_y
            .checked_mul(mask_row_stride)
            .and_then(|row| row.checked_add(region.source_left))
        else {
            return false;
        };
        let Some(mask_end) = mask_start.checked_add(region.width) else {
            return false;
        };
        if mask_x_end > mask_row_stride || mask_end > mask.len() {
            return false;
        }
        let Some(destination_slice) = destination_row.get_mut(destination_left..destination_right)
        else {
            return false;
        };
        let (blocks, tail, row_used_neon) = native_paste_cmyk_l_masked_row(
            source_row,
            destination_slice,
            &mask[mask_start..mask_end],
            allow_short_masked_tail,
        );
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
        used_neon |= row_used_neon;
    }

    super::record_vector_and_scalar_work(vector_blocks, scalar_tail);
    crate::compute::record_pipeline_operation_path(if used_neon {
        "neon-cmyk-l-blend"
    } else {
        "vector"
    });
    return true;
}
