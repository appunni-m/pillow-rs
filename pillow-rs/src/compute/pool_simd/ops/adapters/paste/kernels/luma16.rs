//! I;16 Paste kernels that blend only Pillow's selected stored byte.

use super::super::{NativePasteMaskLayout, NativePastePlan};
use crate::compute::pool_simd::ops::adapters::simd_div255_u16x8;
use wide::u16x8;

/// Blend the first stored byte of eight I;16 samples with an 8-bit mask.
/// Pillow preserves the other byte; `byte_shift` selects which stored byte is
/// first for the image's byte order. The byte products fit in u16 lanes.
#[inline]
fn native_paste_luma16_blend_vector8(
    source: [u16; 8],
    destination: [u16; 8],
    mask: [u8; 8],
    source_byte_shifts: [u32; 8],
    destination_byte_shifts: [u32; 8],
    premultiplied_mask: bool,
) -> [u16; 8] {
    let source_bytes = std::array::from_fn(|lane| {
        u16::from(((source[lane] >> source_byte_shifts[lane]) & 0xff) as u8)
    });
    let destination_bytes = std::array::from_fn(|lane| {
        u16::from(((destination[lane] >> destination_byte_shifts[lane]) & 0xff) as u8)
    });
    let source_lanes = u16x8::new(source_bytes);
    let destination_lanes = u16x8::new(destination_bytes);
    let mask_lanes = u16x8::new(mask.map(u16::from));
    let blended_lanes = if premultiplied_mask {
        let scaled = destination_lanes * (u16x8::splat(255) - mask_lanes) + u16x8::splat(128);
        simd_div255_u16x8(scaled) + source_lanes
    } else {
        let weighted = source_lanes * mask_lanes
            + destination_lanes * (u16x8::splat(255) - mask_lanes)
            + u16x8::splat(127);
        simd_div255_u16x8(weighted)
    };
    let blended_bytes = blended_lanes.to_array().map(|value| value as u8);
    std::array::from_fn(|lane| {
        let shift = destination_byte_shifts[lane];
        let byte_mask = 0xffu16 << shift;
        (destination[lane] & !byte_mask) | (u16::from(blended_bytes[lane]) << shift)
    })
}

#[inline]
fn native_paste_luma16_mask_block(
    mask: &[u8],
    mask_row_stride: usize,
    source_y: usize,
    source_left: usize,
    start: usize,
    active: usize,
    mask_layout: NativePasteMaskLayout,
) -> [u8; 8] {
    std::array::from_fn(|lane| {
        if lane >= active {
            return 0;
        }
        mask[source_y * mask_row_stride
            + (source_left + start + lane) * mask_layout.channels
            + mask_layout.value_index]
    })
}

/// Apply Paste to a native `ImageLuma16` plane. Pillow's masked I;16 path
/// blends only the first byte in mode storage order and preserves the other;
/// an unmasked paste copies complete u16 samples.
/// Each group of up to eight samples uses the same vector blend, including a
/// padded final group; no scalar pixel arithmetic is used for tails.
pub(in crate::compute::pool_simd::ops::adapters::paste) fn native_paste_luma16_apply(
    destination: &mut [u16],
    source: &[u16],
    mask: Option<&[u8]>,
    destination_width: usize,
    destination_height: usize,
    plan: NativePastePlan,
) -> bool {
    if plan.layout.mode != "I;16" {
        return false;
    }
    let Some(destination_len) = destination_width.checked_mul(destination_height) else {
        return false;
    };
    let Some(source_len) = plan.source_width.checked_mul(plan.source_height) else {
        return false;
    };
    if destination.len() != destination_len || source.len() != source_len {
        return false;
    }

    let mask_layout = match (plan.mask, mask) {
        (Some(layout), Some(mask)) => {
            let Some(mask_row_stride) = plan.source_width.checked_mul(layout.channels) else {
                return false;
            };
            let Some(mask_len) = mask_row_stride.checked_mul(plan.source_height) else {
                return false;
            };
            if mask.len() != mask_len {
                return false;
            }
            Some((layout, mask_row_stride))
        }
        (None, None) => None,
        _ => return false,
    };

    let region = plan.region;
    if region.source_left.saturating_add(region.width) > plan.source_width
        || region.source_top.saturating_add(region.height) > plan.source_height
        || region.destination_left.saturating_add(region.width) > destination_width
        || region.destination_top.saturating_add(region.height) > destination_height
    {
        return false;
    }

    let mut vector_blocks = 0u64;
    if let Some((mask_layout, mask_row_stride)) = mask_layout.filter(|(layout, _)| layout.binary) {
        let mask = mask.expect("a binary mask layout always has mask bytes");
        for row in 0..region.height {
            let source_row_start = (region.source_top + row) * plan.source_width;
            let destination_row_start = (region.destination_top + row) * destination_width;
            for start in (0..region.width).step_by(8) {
                let active = (region.width - start).min(8);
                let mask_block = native_paste_luma16_mask_block(
                    mask,
                    mask_row_stride,
                    region.source_top + row,
                    region.source_left,
                    start,
                    active,
                    mask_layout,
                );
                for (lane, value) in mask_block.iter().copied().enumerate().take(active) {
                    if value != 0 {
                        destination
                            [destination_row_start + region.destination_left + start + lane] =
                            source[source_row_start + region.source_left + start + lane];
                    }
                }
            }
        }
    } else if mask_layout.is_none() {
        for row in 0..region.height {
            let source_row_start =
                (region.source_top + row) * plan.source_width + region.source_left;
            let destination_row_start =
                (region.destination_top + row) * destination_width + region.destination_left;
            let source_row = &source[source_row_start..source_row_start + region.width];
            let destination_row =
                &mut destination[destination_row_start..destination_row_start + region.width];
            for start in (0..region.width).step_by(8) {
                let active = (region.width - start).min(8);
                let mut source_block = [0u16; 8];
                source_block[..active].copy_from_slice(&source_row[start..start + active]);
                let output = u16x8::new(source_block).to_array();
                destination_row[start..start + active].copy_from_slice(&output[..active]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    } else if let Some((mask_layout, mask_row_stride)) = mask_layout {
        let mask = mask.expect("a masked layout always has mask bytes");
        for row in 0..region.height {
            let source_row_start = (region.source_top + row) * plan.source_width;
            let destination_row_start = (region.destination_top + row) * destination_width;
            for start in (0..region.width).step_by(8) {
                let active = (region.width - start).min(8);
                let mut source_block = [0u16; 8];
                let mut destination_block = [0u16; 8];
                let mut source_byte_shifts = [0u32; 8];
                let mut destination_byte_shifts = [0u32; 8];
                for lane in 0..active {
                    let source_byte_offset = region.source_left + start + lane;
                    let destination_byte_offset = region.destination_left + start + lane;
                    source_block[lane] = source[source_row_start + source_byte_offset / 2];
                    destination_block[lane] =
                        destination[destination_row_start + destination_byte_offset / 2];
                    source_byte_shifts[lane] = if source_byte_offset % 2 == 0 {
                        plan.luma16_mask_byte_shift
                    } else {
                        8 - plan.luma16_mask_byte_shift
                    };
                    destination_byte_shifts[lane] = if destination_byte_offset % 2 == 0 {
                        plan.luma16_mask_byte_shift
                    } else {
                        8 - plan.luma16_mask_byte_shift
                    };
                }
                let mask_block = native_paste_luma16_mask_block(
                    mask,
                    mask_row_stride,
                    region.source_top + row,
                    region.source_left,
                    start,
                    active,
                    mask_layout,
                );
                let output = native_paste_luma16_blend_vector8(
                    source_block,
                    destination_block,
                    mask_block,
                    source_byte_shifts,
                    destination_byte_shifts,
                    mask_layout.premultiplied,
                );
                for lane in 0..active {
                    let destination_byte_offset = region.destination_left + start + lane;
                    let destination_index = destination_row_start + destination_byte_offset / 2;
                    let shift = destination_byte_shifts[lane];
                    let byte_mask = 0xffu16 << shift;
                    let output_byte = (output[lane] >> shift) & 0xff;
                    destination[destination_index] =
                        (destination[destination_index] & !byte_mask) | (output_byte << shift);
                }
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    }

    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    crate::compute::record_pipeline_operation_path(if plan.mask.is_some() {
        "vector"
    } else {
        "native-copy"
    });
    true
}
