//! Validated byte-region routing for native SIMD Paste modes.

mod cmyk_l;
mod generic;
mod l;
mod la;
mod pa;
mod rgb;
mod rgba;

use super::super::{NativePasteMaskLayout, NativePastePlan, NativePasteSourceLayout};
use super::{
    native_paste_binary_copy_vector16, native_paste_blend_vector8, native_paste_blend_vector16,
    native_paste_cmyk_l_masked_row, native_paste_l_masked_row, native_paste_la_masked_row,
    native_paste_mask_block, native_paste_mask_block_active, native_paste_pa_masked_row,
    native_paste_preblend_vector8, native_paste_preblend_vector16, native_paste_rgb_l_masked_row,
    native_paste_rgba_l_masked_row,
};
#[cfg(feature = "parallel")]
const SIMD_PASTE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

struct NativePasteApplyContext<'a> {
    destination: &'a mut [u8],
    source: &'a [u8],
    mask: Option<&'a [u8]>,
    destination_row_stride: usize,
    source_row_stride: usize,
    region_row_bytes: usize,
    mask_layout: Option<(NativePasteMaskLayout, usize)>,
    plan: NativePastePlan,
    allow_short_masked_tail: bool,
}

fn record_vector_and_scalar_work(vector_blocks: u64, scalar_tail: u64) {
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
}

fn broadcast_source_row(source: &[u8], row_bytes: usize, pixel_bytes: usize) -> Option<Vec<u8>> {
    if pixel_bytes == 0 || source.len() != pixel_bytes || !row_bytes.is_multiple_of(pixel_bytes) {
        return None;
    }
    let mut row = vec![0; row_bytes];
    for pixel in row.chunks_exact_mut(pixel_bytes) {
        pixel.copy_from_slice(source);
    }
    Some(row)
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_apply(
    destination: &mut [u8],
    source: &[u8],
    mask: Option<&[u8]>,
    destination_width: usize,
    destination_height: usize,
    plan: NativePastePlan,
    allow_short_masked_tail: bool,
) -> bool {
    native_paste_apply_with_source_layout(
        destination,
        source,
        mask,
        destination_width,
        destination_height,
        plan,
        NativePasteSourceLayout::Full,
        allow_short_masked_tail,
    )
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_apply_with_source_layout(
    destination: &mut [u8],
    source: &[u8],
    mask: Option<&[u8]>,
    destination_width: usize,
    destination_height: usize,
    plan: NativePastePlan,
    source_layout: NativePasteSourceLayout,
    allow_short_masked_tail: bool,
) -> bool {
    let Some(destination_row_stride) = destination_width.checked_mul(plan.layout.channels) else {
        return false;
    };
    let Some(source_row_stride) = plan.source_width.checked_mul(plan.layout.channels) else {
        return false;
    };
    let Some(destination_bytes) = destination_row_stride.checked_mul(destination_height) else {
        return false;
    };
    let Some(source_bytes) = source_row_stride.checked_mul(plan.source_height) else {
        return false;
    };
    let expected_source_bytes = if matches!(source_layout, NativePasteSourceLayout::Broadcast) {
        plan.layout.channels
    } else {
        source_bytes
    };
    if destination.len() != destination_bytes || source.len() != expected_source_bytes {
        return false;
    }

    // Bitmap passes one solid source pixel with a full-size L mask. Expand it
    // only to a single scanline, then reuse that row for every destination row.
    let broadcast_row = if matches!(source_layout, NativePasteSourceLayout::Broadcast)
        && plan.region.width != 0
        && plan.region.height != 0
    {
        let Some(row) = broadcast_source_row(source, source_row_stride, plan.layout.channels)
        else {
            return false;
        };
        Some(row)
    } else {
        None
    };
    let source = broadcast_row.as_deref().unwrap_or(source);
    let source_row_stride = if matches!(source_layout, NativePasteSourceLayout::Broadcast) {
        0
    } else {
        source_row_stride
    };

    let mask_layout = match (plan.mask, mask) {
        (Some(layout), Some(mask)) => {
            let Some(mask_row_stride) = plan.source_width.checked_mul(layout.channels) else {
                return false;
            };
            let Some(mask_bytes) = mask_row_stride.checked_mul(plan.source_height) else {
                return false;
            };
            if mask.len() != mask_bytes {
                return false;
            }
            Some((layout, mask_row_stride))
        }
        (None, None) => None,
        _ => return false,
    };

    let region = plan.region;
    let Some(region_row_bytes) = region.width.checked_mul(plan.layout.channels) else {
        return false;
    };
    if region.source_left.saturating_add(region.width) > plan.source_width
        || region.source_top.saturating_add(region.height) > plan.source_height
        || region.destination_left.saturating_add(region.width) > destination_width
        || region.destination_top.saturating_add(region.height) > destination_height
    {
        return false;
    }

    let mut context = NativePasteApplyContext {
        destination,
        source,
        mask,
        destination_row_stride,
        source_row_stride,
        region_row_bytes,
        mask_layout,
        plan,
        allow_short_masked_tail,
    };
    dispatch_modes(&mut context)
}

fn dispatch_modes(context: &mut NativePasteApplyContext<'_>) -> bool {
    let plan = context.plan;
    let region = plan.region;
    if region.width != 0
        && region.height != 0
        && let Some((mask_layout, _)) = context.mask_layout
        && mask_layout.channels == 1
        && mask_layout.value_index == 0
        && !mask_layout.premultiplied
        && !mask_layout.binary
    {
        match plan.layout.mode {
            "CMYK" if plan.layout.channels == 4 => return cmyk_l::apply(context),
            "PA" if plan.layout.channels == 2 => return pa::apply(context),
            "RGBA" if plan.layout.channels == 4 => return rgba::apply(context),
            "LA" if plan.layout.channels == 2 => return la::apply(context),
            "RGB" | "HSV" if plan.layout.channels == 3 => return rgb::apply(context),
            "L" if plan.layout.channels == 1 => return l::apply(context),
            _ => {}
        }
    }
    generic::apply(context)
}
