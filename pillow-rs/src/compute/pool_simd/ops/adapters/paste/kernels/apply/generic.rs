//! Generic native byte copy and masked blend fallback.

use super::{
    NativePasteApplyContext, native_paste_blend_vector8, native_paste_blend_vector16,
    native_paste_mask_block, native_paste_mask_block_active, native_paste_preblend_vector8,
    native_paste_preblend_vector16,
};
use crate::compute::pool_simd::ops::adapters::copy_native_bytes;

pub(super) fn apply(context: &mut NativePasteApplyContext<'_>) -> bool {
    let metrics = match context.mask_layout {
        None => copy_rows(context),
        Some((mask_layout, mask_row_stride)) => {
            blend_masked_rows(context, mask_layout, mask_row_stride)
        }
    };
    let Some((vector_blocks, scalar_tail)) = metrics else {
        return false;
    };
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path(if context.plan.mask.is_some() {
        "vector"
    } else {
        "native-copy"
    });
    true
}

fn copy_rows(context: &mut NativePasteApplyContext<'_>) -> Option<(u64, u64)> {
    let plan = context.plan;
    let region = plan.region;
    let destination_row_stride = context.destination_row_stride;
    let source_row_stride = context.source_row_stride;
    let region_row_bytes = context.region_row_bytes;
    let source = context.source;
    let destination = &mut *context.destination;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for row in 0..region.height {
        let source_row_start = (region.source_top + row) * source_row_stride
            + region.source_left * plan.layout.channels;
        let destination_row_start = (region.destination_top + row) * destination_row_stride
            + region.destination_left * plan.layout.channels;
        let source_row = &source[source_row_start..source_row_start + region_row_bytes];
        let destination_row =
            &mut destination[destination_row_start..destination_row_start + region_row_bytes];
        let Some((blocks, tail)) = copy_native_bytes(source_row, destination_row) else {
            return None;
        };
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Some((vector_blocks, scalar_tail))
}

#[inline]
fn blend_masked_row(
    source_row: &[u8],
    destination_row: &mut [u8],
    mask: &[u8],
    mask_row_stride: usize,
    source_y: usize,
    region: super::super::super::NativePasteRegion,
    plan: super::super::super::NativePastePlan,
    mask_layout: super::super::super::NativePasteMaskLayout,
    region_row_bytes: usize,
    allow_short_masked_tail: bool,
    vector_blocks: &mut u64,
    scalar_tail: &mut u64,
) {
    let vector_len = region_row_bytes / 16 * 16;
    for start in (0..vector_len).step_by(16) {
        let source_block = <[u8; 16]>::try_from(&source_row[start..start + 16])
            .expect("native Paste blend block has 16 bytes");
        let destination_block = <[u8; 16]>::try_from(&destination_row[start..start + 16])
            .expect("native Paste destination block has 16 bytes");
        let mask_block = native_paste_mask_block::<16>(
            mask,
            mask_row_stride,
            source_y,
            region.source_left,
            start,
            plan.layout.channels,
            mask_layout,
        );
        let blended = if mask_layout.premultiplied {
            native_paste_preblend_vector16(source_block, destination_block, mask_block)
        } else {
            native_paste_blend_vector16(source_block, destination_block, mask_block)
        };
        destination_row[start..start + 16].copy_from_slice(&blended);
        *vector_blocks = vector_blocks.saturating_add(1);
    }

    let vector_len8 = region_row_bytes / 8 * 8;
    for start in (vector_len..vector_len8).step_by(8) {
        let source_block = <[u8; 8]>::try_from(&source_row[start..start + 8])
            .expect("native Paste blend block has 8 bytes");
        let destination_block = <[u8; 8]>::try_from(&destination_row[start..start + 8])
            .expect("native Paste destination block has 8 bytes");
        let mask_block = native_paste_mask_block::<8>(
            mask,
            mask_row_stride,
            source_y,
            region.source_left,
            start,
            plan.layout.channels,
            mask_layout,
        );
        let blended = if mask_layout.premultiplied {
            native_paste_preblend_vector8(source_block, destination_block, mask_block)
        } else {
            native_paste_blend_vector8(source_block, destination_block, mask_block)
        };
        destination_row[start..start + 8].copy_from_slice(&blended);
        *vector_blocks = vector_blocks.saturating_add(1);
    }

    let tail = region_row_bytes - vector_len8;
    if tail != 0 && allow_short_masked_tail {
        // Pad inactive lanes so narrow rows keep the same vector blend and do
        // not read beyond the source or mask row.
        let mut source_block = [0u8; 8];
        let mut destination_block = [0u8; 8];
        source_block[..tail].copy_from_slice(&source_row[vector_len8..]);
        destination_block[..tail].copy_from_slice(&destination_row[vector_len8..]);
        let mask_block = native_paste_mask_block_active::<8>(
            mask,
            mask_row_stride,
            source_y,
            region.source_left,
            vector_len8,
            region_row_bytes,
            plan.layout.channels,
            mask_layout,
        );
        let blended = if mask_layout.premultiplied {
            native_paste_preblend_vector8(source_block, destination_block, mask_block)
        } else {
            native_paste_blend_vector8(source_block, destination_block, mask_block)
        };
        destination_row[vector_len8..].copy_from_slice(&blended[..tail]);
        *vector_blocks = vector_blocks.saturating_add(1);
        *scalar_tail = scalar_tail.saturating_add(tail as u64);
    } else {
        for index in vector_len8..region_row_bytes {
            let source_pixel = index / plan.layout.channels;
            let mask_value = mask[source_y * mask_row_stride
                + (region.source_left + source_pixel) * mask_layout.channels
                + mask_layout.value_index];
            let source_value = source_row[index];
            let destination_value = destination_row[index];
            destination_row[index] = if mask_layout.premultiplied {
                let scaled = u32::from(destination_value) * u32::from(255 - mask_value) + 128;
                ((((scaled >> 8) + scaled) >> 8) + u32::from(source_value)) as u8
            } else {
                let mask = u16::from(mask_value);
                ((u16::from(source_value) * mask
                    + u16::from(destination_value) * (255 - mask)
                    + 127)
                    / 255) as u8
            };
            *scalar_tail = scalar_tail.saturating_add(1);
        }
    }
}

fn blend_masked_rows(
    context: &mut NativePasteApplyContext<'_>,
    mask_layout: super::super::super::NativePasteMaskLayout,
    mask_row_stride: usize,
) -> Option<(u64, u64)> {
    let plan = context.plan;
    let region = plan.region;
    let destination_row_stride = context.destination_row_stride;
    let source_row_stride = context.source_row_stride;
    let region_row_bytes = context.region_row_bytes;
    let allow_short_masked_tail = context.allow_short_masked_tail;
    let source = context.source;
    let mask = context.mask.expect("a mask layout always has mask bytes");
    let destination = &mut *context.destination;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for row in 0..region.height {
        let source_y = region.source_top + row;
        let source_row_start =
            source_y * source_row_stride + region.source_left * plan.layout.channels;
        let destination_row_start = (region.destination_top + row) * destination_row_stride
            + region.destination_left * plan.layout.channels;
        let source_row = &source[source_row_start..source_row_start + region_row_bytes];
        let destination_row =
            &mut destination[destination_row_start..destination_row_start + region_row_bytes];
        blend_masked_row(
            source_row,
            destination_row,
            mask,
            mask_row_stride,
            source_y,
            region,
            plan,
            mask_layout,
            region_row_bytes,
            allow_short_masked_tail,
            &mut vector_blocks,
            &mut scalar_tail,
        );
    }
    Some((vector_blocks, scalar_tail))
}
