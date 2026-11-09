//! Eight-lane typed I;16 Paste strategy preserving Pillow's byte-lane behavior.

use crate::compute::pool_cpu::ops::paste::helpers::plan::Luma16Plan;
use crate::error::PilError;
use wide::u16x8;

/// Executes a destination-mode vectorized I;16 Paste plan.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "prepared extents validate every sample and mask access"
)]
pub(crate) fn execute(plan: Luma16Plan<'_>, source: &[u16]) -> Result<Vec<u16>, PilError> {
    let source_width = usize::try_from(plan.source_size.0).map_err(|_| overflow("source width"))?;
    let source_height =
        usize::try_from(plan.source_size.1).map_err(|_| overflow("source height"))?;
    let destination_width =
        usize::try_from(plan.destination_size.0).map_err(|_| overflow("destination width"))?;
    let destination_height =
        usize::try_from(plan.destination_size.1).map_err(|_| overflow("destination height"))?;
    if source.len() != sample_count(source_width, source_height)?
        || plan.destination.len() != sample_count(destination_width, destination_height)?
    {
        return Err(PilError::InternalError(
            "I;16 Paste storage did not match dimensions".into(),
        ));
    }
    let mut output = plan.destination.to_vec();
    let mut copy_vector_blocks = 0_u64;
    match (plan.mask.as_deref(), plan.mask_layout) {
        (None, None) => {
            copy_vector_blocks =
                copy_region(&plan, source, &mut output, source_width, destination_width);
        }
        (Some(mask), Some(layout)) if layout.binary => {
            copy_binary_region(
                &plan,
                source,
                mask.as_bytes(),
                &mut output,
                source_width,
                destination_width,
                layout,
            );
        }
        (Some(mask), Some(layout)) => blend_fractional_region(
            &plan,
            source,
            mask.as_bytes(),
            &mut output,
            source_width,
            destination_width,
            layout,
        )?,
        _ => {
            return Err(PilError::InternalError(
                "I;16 Paste mask strategy was incomplete".into(),
            ));
        }
    }
    if plan.mask.is_some() {
        if plan.mask_layout.is_some_and(|layout| !layout.binary) {
            let blocks = (plan.region.width as usize)
                .div_ceil(8)
                .saturating_mul(plan.region.height as usize);
            record_vector_blocks(u64::try_from(blocks).unwrap_or(u64::MAX));
        }
        crate::compute::record_pipeline_operation_path("vector");
    } else {
        record_vector_blocks(copy_vector_blocks);
        crate::compute::record_pipeline_operation_path("native-copy");
    }
    Ok(output)
}

fn copy_region(
    plan: &Luma16Plan<'_>,
    source: &[u16],
    output: &mut [u16],
    source_width: usize,
    destination_width: usize,
) -> u64 {
    let source_high_byte_first = super::i16::high_byte_first(plan.source_mode);
    let destination_high_byte_first = super::i16::high_byte_first(plan.mode);
    let mut vector_blocks = 0u64;
    for row in 0..plan.region.height as usize {
        let source_start = (plan.region.source_top as usize + row) * source_width
            + plan.region.source_left as usize;
        let destination_start = (plan.region.destination_top as usize + row) * destination_width
            + plan.region.destination_left as usize;
        let width = plan.region.width as usize;
        let source_row = &source[source_start..source_start + width];
        let destination_row = &mut output[destination_start..destination_start + width];
        if source_high_byte_first != destination_high_byte_first {
            for start in (0..width).step_by(8) {
                let active = (width - start).min(8);
                let mut block = [0_u16; 8];
                block[..active].copy_from_slice(&source_row[start..start + active]);
                let values = u16x8::new(block);
                let swapped = ((values << 8u32) | (values >> 8u32)).to_array();
                destination_row[start..start + active].copy_from_slice(&swapped[..active]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
            continue;
        }
        for start in (0..width).step_by(8) {
            let active = (width - start).min(8);
            let mut block = [0u16; 8];
            block[..active].copy_from_slice(&source_row[start..start + active]);
            let copied = u16x8::new(block).to_array();
            destination_row[start..start + active].copy_from_slice(&copied[..active]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    vector_blocks
}

fn record_vector_blocks(blocks: u64) {
    if blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
    }
}

fn copy_binary_region(
    plan: &Luma16Plan<'_>,
    source: &[u16],
    mask: &[u8],
    output: &mut [u16],
    source_width: usize,
    destination_width: usize,
    layout: super::layout::MaskLayout,
) {
    let mask_width = plan.source_size.0 as usize;
    let source_high_byte_first = super::i16::high_byte_first(plan.source_mode);
    let destination_high_byte_first = super::i16::high_byte_first(plan.mode);
    for row in 0..plan.region.height as usize {
        let source_y = plan.region.source_top as usize + row;
        let destination_y = plan.region.destination_top as usize + row;
        for column in 0..plan.region.width as usize {
            let source_x = plan.region.source_left as usize + column;
            let mask_index =
                (source_y * mask_width + source_x) * layout.channels + layout.value_index;
            if mask[mask_index] != 0 {
                let source_index = source_y * source_width + source_x;
                let destination_index = destination_y * destination_width
                    + plan.region.destination_left as usize
                    + column;
                output[destination_index] = super::i16::sample_in_destination_order(
                    source[source_index],
                    source_high_byte_first,
                    destination_high_byte_first,
                );
            }
        }
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "prepared extents validate byte-lane source, destination, and mask offsets"
)]
fn blend_fractional_region(
    plan: &Luma16Plan<'_>,
    source: &[u16],
    mask: &[u8],
    output: &mut [u16],
    source_width: usize,
    destination_width: usize,
    layout: super::layout::MaskLayout,
) -> Result<(), PilError> {
    let mask_width = source_width;
    let source_left = plan.region.source_left as usize;
    let destination_left = plan.region.destination_left as usize;
    let source_high_byte_first = super::i16::high_byte_first(plan.source_mode);
    let destination_high_byte_first = super::i16::high_byte_first(plan.mode);
    for row in 0..plan.region.height as usize {
        let source_y = plan.region.source_top as usize + row;
        let destination_y = plan.region.destination_top as usize + row;
        for start in (0..plan.region.width as usize).step_by(8) {
            let active = (plan.region.width as usize - start).min(8);
            let mut source_bytes = [0u8; 8];
            let mut destination_bytes = [0u8; 8];
            let mut masks = [0u8; 8];
            let mut destination_samples = [0usize; 8];
            let mut destination_shifts = [0u32; 8];
            for lane in 0..active {
                let source_byte_x = source_left + start + lane;
                let destination_byte_x = destination_left + start + lane;
                let source_index = source_y * source_width + source_byte_x / 2;
                let destination_index = destination_y * destination_width + destination_byte_x / 2;
                let source_shift = byte_shift(source_high_byte_first, source_byte_x);
                let destination_shift = byte_shift(destination_high_byte_first, destination_byte_x);
                source_bytes[lane] = ((source[source_index] >> source_shift) & 0xff) as u8;
                destination_bytes[lane] =
                    ((output[destination_index] >> destination_shift) & 0xff) as u8;
                let mask_index =
                    (source_y * mask_width + source_byte_x) * layout.channels + layout.value_index;
                masks[lane] = mask[mask_index];
                destination_samples[lane] = destination_index;
                destination_shifts[lane] = destination_shift;
            }
            let blended = blend_group(source_bytes, destination_bytes, masks, layout.premultiplied);
            write_byte_lanes(
                output,
                blended,
                destination_samples,
                destination_shifts,
                active,
            );
        }
    }
    Ok(())
}

fn blend_group(
    source: [u8; 8],
    destination: [u8; 8],
    mask: [u8; 8],
    premultiplied: bool,
) -> [u8; 8] {
    let source = u16x8::new(source.map(u16::from));
    let destination = u16x8::new(destination.map(u16::from));
    let weight = u16x8::new(mask.map(u16::from));
    let inverse = u16x8::splat(255) - weight;
    let values = if premultiplied {
        let scaled = destination * inverse + u16x8::splat(128);
        div255(scaled) + source
    } else {
        div255(source * weight + destination * inverse + u16x8::splat(127))
    };
    values.to_array().map(|value| value as u8)
}

fn write_byte_lanes(
    output: &mut [u16],
    blended: [u8; 8],
    samples: [usize; 8],
    shifts: [u32; 8],
    active: usize,
) {
    for lane in 0..active {
        let shift = shifts[lane];
        let byte_mask = 0xffu16 << shift;
        let current = output[samples[lane]];
        output[samples[lane]] = (current & !byte_mask) | (u16::from(blended[lane]) << shift);
    }
}

fn div255(value: u16x8) -> u16x8 {
    let incremented = value + u16x8::splat(1);
    (incremented + (incremented >> 8u32)) >> 8u32
}

fn byte_shift(high_byte_first: bool, byte_offset: usize) -> u32 {
    match (high_byte_first, byte_offset % 2) {
        (false, 0) | (true, 1) => 0,
        (true, 0) | (false, 1) => 8,
        _ => unreachable!("byte offset parity has two values"),
    }
}

fn sample_count(width: usize, height: usize) -> Result<usize, PilError> {
    width
        .checked_mul(height)
        .ok_or_else(|| overflow("sample count"))
}

fn overflow(part: &str) -> PilError {
    PilError::OverflowError(format!("I;16 Paste vector {part} overflowed"))
}
