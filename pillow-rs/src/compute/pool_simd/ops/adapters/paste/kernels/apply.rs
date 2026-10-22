//! Paste row dispatch and native output materialization.

use super::super::NativePastePlan;
use super::{
    native_paste_blend_vector8, native_paste_blend_vector16, native_paste_cmyk_l_masked_row,
    native_paste_l_masked_row, native_paste_la_masked_row, native_paste_mask_block,
    native_paste_mask_block_active, native_paste_pa_masked_row, native_paste_preblend_vector8,
    native_paste_preblend_vector16, native_paste_rgb_l_masked_row, native_paste_rgba_l_masked_row,
};
use crate::compute::pool_simd::ops::adapters::copy_native_bytes;

#[cfg(feature = "parallel")]
const SIMD_PASTE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_apply(
    destination: &mut [u8],
    source: &[u8],
    mask: Option<&[u8]>,
    destination_width: usize,
    destination_height: usize,
    plan: NativePastePlan,
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
    if destination.len() != destination_bytes || source.len() != source_bytes {
        return false;
    }

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

    if plan.layout.mode == "CMYK"
        && plan.layout.channels == 4
        && region.width != 0
        && region.height != 0
        && let Some((mask_layout, mask_row_stride)) = mask_layout
        && mask_layout.channels == 1
        && mask_layout.value_index == 0
        && !mask_layout.premultiplied
        && !mask_layout.binary
    {
        let Some(destination_region_start) =
            region.destination_top.checked_mul(destination_row_stride)
        else {
            return false;
        };
        let Some(destination_region_len) = region.height.checked_mul(destination_row_stride) else {
            return false;
        };
        let Some(destination_region_end) =
            destination_region_start.checked_add(destination_region_len)
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
            let Some(destination_slice) =
                destination_row.get_mut(destination_left..destination_right)
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

        if vector_blocks != 0 {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        }
        if scalar_tail != 0 {
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        }
        crate::compute::record_pipeline_operation_path(if used_neon {
            "neon-cmyk-l-blend"
        } else {
            "vector"
        });
        return true;
    }

    if plan.layout.mode == "PA"
        && plan.layout.channels == 2
        && region.width != 0
        && region.height != 0
        && let Some((mask_layout, mask_row_stride)) = mask_layout
        && mask_layout.channels == 1
        && mask_layout.value_index == 0
        && !mask_layout.premultiplied
        && !mask_layout.binary
    {
        let Some(destination_region_start) =
            region.destination_top.checked_mul(destination_row_stride)
        else {
            return false;
        };
        let Some(destination_region_len) = region.height.checked_mul(destination_row_stride) else {
            return false;
        };
        let Some(destination_region_end) =
            destination_region_start.checked_add(destination_region_len)
        else {
            return false;
        };
        let Some(destination_rows) =
            destination.get_mut(destination_region_start..destination_region_end)
        else {
            return false;
        };
        let mask = mask.expect("a validated L mask layout always has mask bytes");
        let Some(destination_left) = region.destination_left.checked_mul(2) else {
            return false;
        };
        let Some(destination_right) = region
            .destination_left
            .checked_add(region.width)
            .and_then(|right| right.checked_mul(2))
        else {
            return false;
        };
        let apply_pa_row = |row_index: usize, destination_row: &mut [u8]| {
            let source_y = region.source_top + row_index;
            let source_row_start = source_y * source_row_stride + region.source_left * 2;
            let source_row = &source[source_row_start..source_row_start + region_row_bytes];
            let mask_row_start = source_y * mask_row_stride + region.source_left;
            let mask_row = &mask[mask_row_start..mask_row_start + region.width];
            native_paste_pa_masked_row(
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
            super::super::super::simd_rows_mut_serial!(
                destination_rows,
                destination_row_stride,
                region.height,
                |_row_start, _row_end, row, destination_row| {
                    apply_pa_row(row as usize, destination_row);
                }
            );
        } else {
            for (row_index, destination_row) in destination_rows
                .chunks_exact_mut(destination_row_stride)
                .enumerate()
            {
                apply_pa_row(row_index, destination_row);
            }
        }

        #[cfg(not(feature = "parallel"))]
        for (row_index, destination_row) in destination_rows
            .chunks_exact_mut(destination_row_stride)
            .enumerate()
        {
            apply_pa_row(row_index, destination_row);
        }

        let vector_len16 = region_row_bytes / 16 * 16;
        let vector_len8 = region_row_bytes / 8 * 8;
        let vector_blocks_per_row = (vector_len16 / 16 + (vector_len8 - vector_len16) / 8)
            + usize::from(allow_short_masked_tail && vector_len8 < region_row_bytes);
        let vector_blocks = vector_blocks_per_row.saturating_mul(region.height) as u64;
        let scalar_tail = if allow_short_masked_tail {
            0
        } else {
            (region_row_bytes - vector_len8).saturating_mul(region.height) as u64
        };
        if vector_blocks != 0 {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        }
        if scalar_tail != 0 {
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        }
        crate::compute::record_pipeline_operation_path("vector");
        return true;
    }

    if plan.layout.mode == "RGBA"
        && plan.layout.channels == 4
        && region.width != 0
        && region.height != 0
        && let Some((mask_layout, mask_row_stride)) = mask_layout
        && mask_layout.channels == 1
        && mask_layout.value_index == 0
        && !mask_layout.premultiplied
        && !mask_layout.binary
    {
        let Some(destination_region_start) =
            region.destination_top.checked_mul(destination_row_stride)
        else {
            return false;
        };
        let Some(destination_region_len) = region.height.checked_mul(destination_row_stride) else {
            return false;
        };
        let Some(destination_region_end) =
            destination_region_start.checked_add(destination_region_len)
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
            let Some(destination_slice) =
                destination_row.get_mut(destination_left..destination_right)
            else {
                return false;
            };
            let (blocks, tail, row_used_neon) = native_paste_rgba_l_masked_row(
                source_row,
                destination_slice,
                &mask[mask_start..mask_end],
                allow_short_masked_tail,
            );
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
            used_neon |= row_used_neon;
        }

        if vector_blocks != 0 {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        }
        if scalar_tail != 0 {
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        }
        crate::compute::record_pipeline_operation_path(if used_neon {
            "neon-rgba-l-blend"
        } else {
            "rgba-l-mask-vector"
        });
        return true;
    }

    if plan.layout.mode == "LA"
        && plan.layout.channels == 2
        && region.width != 0
        && region.height != 0
        && let Some((mask_layout, mask_row_stride)) = mask_layout
        && mask_layout.channels == 1
        && mask_layout.value_index == 0
        && !mask_layout.premultiplied
        && !mask_layout.binary
    {
        let Some(destination_region_start) =
            region.destination_top.checked_mul(destination_row_stride)
        else {
            return false;
        };
        let Some(destination_region_len) = region.height.checked_mul(destination_row_stride) else {
            return false;
        };
        let Some(destination_region_end) =
            destination_region_start.checked_add(destination_region_len)
        else {
            return false;
        };
        let Some(destination_rows) =
            destination.get_mut(destination_region_start..destination_region_end)
        else {
            return false;
        };
        let mask = mask.expect("a validated L mask layout always has mask bytes");
        let Some(destination_left) = region.destination_left.checked_mul(2) else {
            return false;
        };
        let Some(destination_right) = region
            .destination_left
            .checked_add(region.width)
            .and_then(|right| right.checked_mul(2))
        else {
            return false;
        };
        let apply_la_row = |row_index: usize, destination_row: &mut [u8]| {
            let source_y = region.source_top + row_index;
            let source_row_start = source_y * source_row_stride + region.source_left * 2;
            let source_row = &source[source_row_start..source_row_start + region_row_bytes];
            let mask_row_start = source_y * mask_row_stride + region.source_left;
            let mask_row = &mask[mask_row_start..mask_row_start + region.width];
            native_paste_la_masked_row(
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
            super::super::super::simd_rows_mut_serial!(
                destination_rows,
                destination_row_stride,
                region.height,
                |_row_start, _row_end, row, destination_row| {
                    used_neon |= apply_la_row(row as usize, destination_row);
                }
            );
        } else {
            for (row_index, destination_row) in destination_rows
                .chunks_exact_mut(destination_row_stride)
                .enumerate()
            {
                used_neon |= apply_la_row(row_index, destination_row);
            }
        }

        #[cfg(not(feature = "parallel"))]
        for (row_index, destination_row) in destination_rows
            .chunks_exact_mut(destination_row_stride)
            .enumerate()
        {
            used_neon |= apply_la_row(row_index, destination_row);
        }

        let vector_len16 = region_row_bytes / 16 * 16;
        let vector_len8 = region_row_bytes / 8 * 8;
        let vector_blocks_per_row = (vector_len16 / 16 + (vector_len8 - vector_len16) / 8)
            + usize::from(allow_short_masked_tail && vector_len8 < region_row_bytes);
        let vector_blocks = vector_blocks_per_row.saturating_mul(region.height) as u64;
        let scalar_tail = if allow_short_masked_tail {
            0
        } else {
            (region_row_bytes - vector_len8).saturating_mul(region.height) as u64
        };
        if vector_blocks != 0 {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        }
        if scalar_tail != 0 {
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        }
        crate::compute::record_pipeline_operation_path(if used_neon {
            "neon-la-l-blend"
        } else {
            "vector"
        });
        return true;
    }

    if matches!(plan.layout.mode, "RGB" | "HSV")
        && plan.layout.channels == 3
        && region.width != 0
        && region.height != 0
        && let Some((mask_layout, mask_row_stride)) = mask_layout
        && mask_layout.channels == 1
        && mask_layout.value_index == 0
        && !mask_layout.premultiplied
        && !mask_layout.binary
    {
        let Some(destination_region_start) =
            region.destination_top.checked_mul(destination_row_stride)
        else {
            return false;
        };
        let Some(destination_region_len) = region.height.checked_mul(destination_row_stride) else {
            return false;
        };
        let Some(destination_region_end) =
            destination_region_start.checked_add(destination_region_len)
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
        let apply_rgb_row = |row_index: usize, destination_row: &mut [u8]| {
            let source_y = region.source_top + row_index;
            let source_row_start = source_y * source_row_stride + region.source_left * 3;
            let source_row = &source[source_row_start..source_row_start + region_row_bytes];
            let mask_row_start = source_y * mask_row_stride + region.source_left;
            let mask_row = &mask[mask_row_start..mask_row_start + region.width];
            native_paste_rgb_l_masked_row(
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
            super::super::super::simd_rows_mut_serial!(
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

        let vector_len16 = region_row_bytes / 16 * 16;
        let vector_len8 = region_row_bytes / 8 * 8;
        let vector_blocks_per_row = (vector_len16 / 16 + (vector_len8 - vector_len16) / 8)
            + usize::from(allow_short_masked_tail && vector_len8 < region_row_bytes);
        let vector_blocks = vector_blocks_per_row.saturating_mul(region.height) as u64;
        let scalar_tail = if allow_short_masked_tail {
            0
        } else {
            (region_row_bytes - vector_len8).saturating_mul(region.height) as u64
        };
        if vector_blocks != 0 {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        }
        if scalar_tail != 0 {
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        }
        crate::compute::record_pipeline_operation_path("vector");
        return true;
    }

    if plan.layout.mode == "L"
        && plan.layout.channels == 1
        && region.width != 0
        && region.height != 0
        && let Some((mask_layout, mask_row_stride)) = mask_layout
        && mask_layout.channels == 1
        && mask_layout.value_index == 0
        && !mask_layout.premultiplied
        && !mask_layout.binary
    {
        let Some(destination_region_start) =
            region.destination_top.checked_mul(destination_row_stride)
        else {
            return false;
        };
        let Some(destination_region_len) = region.height.checked_mul(destination_row_stride) else {
            return false;
        };
        let Some(destination_region_end) =
            destination_region_start.checked_add(destination_region_len)
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
            super::super::super::simd_rows_mut_serial!(
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
        crate::compute::record_pipeline_operation_path(if used_neon {
            "neon-blend"
        } else {
            "vector"
        });
        return true;
    }

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
        match mask_layout {
            None => {
                let Some((blocks, tail)) = copy_native_bytes(source_row, destination_row) else {
                    return false;
                };
                vector_blocks = vector_blocks.saturating_add(blocks);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
            Some((mask_layout, mask_row_stride)) => {
                let mask = mask.expect("a mask layout always has mask bytes");
                let vector_len = region_row_bytes / 16 * 16;
                for start in (0..vector_len).step_by(16) {
                    let source_block = <[u8; 16]>::try_from(&source_row[start..start + 16])
                        .expect("native Paste blend block has 16 bytes");
                    let destination_block =
                        <[u8; 16]>::try_from(&destination_row[start..start + 16])
                            .expect("native Paste destination block has 16 bytes");
                    let mask_block = native_paste_mask_block::<16>(
                        mask,
                        mask_row_stride,
                        region.source_top + row,
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
                    vector_blocks = vector_blocks.saturating_add(1);
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
                        region.source_top + row,
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
                    vector_blocks = vector_blocks.saturating_add(1);
                }
                let tail = region_row_bytes - vector_len8;
                if tail != 0 && allow_short_masked_tail {
                    // A partial row still uses the same vector blend. Pad
                    // only the inactive lanes so tiny composite rows (for
                    // example a 3-byte P row) never fall back to scalar pixel
                    // arithmetic or read past the source/mask row.
                    let mut source_block = [0u8; 8];
                    let mut destination_block = [0u8; 8];
                    source_block[..tail].copy_from_slice(&source_row[vector_len8..]);
                    destination_block[..tail].copy_from_slice(&destination_row[vector_len8..]);
                    let mask_block = native_paste_mask_block_active::<8>(
                        mask,
                        mask_row_stride,
                        region.source_top + row,
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
                    vector_blocks = vector_blocks.saturating_add(1);
                    scalar_tail = scalar_tail.saturating_add(tail as u64);
                } else {
                    for index in vector_len8..region_row_bytes {
                        let source_pixel = index / plan.layout.channels;
                        let mask_value = mask[(region.source_top + row) * mask_row_stride
                            + (region.source_left + source_pixel) * mask_layout.channels
                            + mask_layout.value_index];
                        let source_value = source_row[index];
                        let destination_value = destination_row[index];
                        destination_row[index] = if mask_layout.premultiplied {
                            let scaled =
                                u32::from(destination_value) * u32::from(255 - mask_value) + 128;
                            ((((scaled >> 8) + scaled) >> 8) + u32::from(source_value)) as u8
                        } else {
                            let mask = u16::from(mask_value);
                            ((u16::from(source_value) * mask
                                + u16::from(destination_value) * (255 - mask)
                                + 127)
                                / 255) as u8
                        };
                        scalar_tail = scalar_tail.saturating_add(1);
                    }
                }
            }
        }
    }

    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path(if plan.mask.is_some() {
        "vector"
    } else {
        "native-copy"
    });
    true
}
