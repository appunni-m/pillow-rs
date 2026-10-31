use crate::error::PilError;

use super::plan::Luma16Plan;

/// Executes Pillow's typed I;16 Paste behavior without changing mode identity.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "validated image extents bound every row, pixel, and byte-lane offset"
)]
pub(crate) fn execute(plan: Luma16Plan<'_>, source: &[u16]) -> Result<Vec<u16>, PilError> {
    let (source_width, source_height) = plan.source_size;
    let (destination_width, _) = plan.destination_size;
    let expected_source = sample_count(source_width, source_height)?;
    let expected_destination = sample_count(plan.destination_size.0, plan.destination_size.1)?;
    if source.len() != expected_source || plan.destination.len() != expected_destination {
        return Err(PilError::InternalError("I;16 Paste storage did not match dimensions".into()));
    }
    let mut output = plan.destination.to_vec();
    if plan.region.is_empty() {
        return Ok(output);
    }
    let mask_bytes = plan.mask.as_deref().map(|mask| mask.as_bytes());
    let mask_layout = plan.mask_layout;
    let high_byte_first = high_byte_first(plan.mode);
    let region_width = usize::try_from(plan.region.width)
        .map_err(|_| overflow("region width"))?;
    let source_left = usize::try_from(plan.region.source_left)
        .map_err(|_| overflow("source x"))?;
    let destination_left = usize::try_from(plan.region.destination_left)
        .map_err(|_| overflow("destination x"))?;
    let source_top = usize::try_from(plan.region.source_top)
        .map_err(|_| overflow("source y"))?;
    let destination_top = usize::try_from(plan.region.destination_top)
        .map_err(|_| overflow("destination y"))?;
    let copy_width = usize::try_from(plan.region.width)
        .map_err(|_| overflow("copy width"))?;
    let copy_height = usize::try_from(plan.region.height)
        .map_err(|_| overflow("copy height"))?;
    let source_width = usize::try_from(source_width).map_err(|_| overflow("source width"))?;
    let destination_width = usize::try_from(destination_width)
        .map_err(|_| overflow("destination width"))?;
    let mask_width = source_width;

    for row in 0..copy_height {
        let source_y = source_top + row;
        let destination_y = destination_top + row;
        for column in 0..copy_width {
            let source_x = source_left + column;
            let destination_x = destination_left + column;
            let Some((bytes, layout)) = mask_bytes.zip(mask_layout) else {
                output[destination_y * destination_width + destination_x] =
                    source[source_y * source_width + source_x];
                continue;
            };
            let mask_index =
                (source_y * mask_width + source_x) * layout.channels + layout.value_index;
            let value = bytes[mask_index];
            if layout.binary {
                if value != 0 {
                    output[destination_y * destination_width + destination_x] =
                        source[source_y * source_width + source_x];
                }
                continue;
            }

            // Pillow's I;16 Paste walks the raw image8 pointer one byte per
            // mask pixel. Preserve the untouched byte and the explicit mode
            // byte order instead of widening the mask to a u16 blend.
            if value == 0 && !layout.premultiplied {
                continue;
            }
            let source_byte_x = source_x / 2;
            let destination_byte_x = destination_x / 2;
            let source_sample = source[source_y * source_width + source_byte_x];
            let destination_index = destination_y * destination_width + destination_byte_x;
            let destination_sample = output[destination_index];
            let source_shift = byte_shift(high_byte_first, source_x);
            let destination_shift = byte_shift(high_byte_first, destination_x);
            let source_byte = ((source_sample >> source_shift) & 0xff) as u8;
            let destination_byte = ((destination_sample >> destination_shift) & 0xff) as u8;
            let blended = blend_byte(source_byte, destination_byte, value, layout.premultiplied);
            let byte_mask = 0xffu16 << destination_shift;
            output[destination_index] =
                (destination_sample & !byte_mask) | (u16::from(blended) << destination_shift);
        }
    }
    Ok(output)
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "the mask is a byte and the byte blend intermediates are bounded"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "Pillow stores the blended byte and selected byte lane only"
)]
fn blend_byte(source: u8, destination: u8, mask: u8, premultiplied: bool) -> u8 {
    let inverse = 255 - u32::from(mask);
    if premultiplied {
        let scaled = u32::from(destination) * inverse + 128;
        ((((scaled >> 8) + scaled) >> 8) + u32::from(source)) as u8
    } else {
        ((u32::from(source) * u32::from(mask)
            + u32::from(destination) * inverse
            + 127)
            / 255) as u8
    }
}

fn high_byte_first(mode: crate::raster::Mode) -> bool {
    match mode {
        crate::raster::Mode::ImageI16B => true,
        crate::raster::Mode::ImageI16L => false,
        crate::raster::Mode::ImageI16 | crate::raster::Mode::ImageI16N => {
            cfg!(target_endian = "big")
        }
        _ => false,
    }
}

fn byte_shift(high_byte_first: bool, byte_offset: usize) -> u32 {
    match (high_byte_first, byte_offset % 2) {
        (false, 0) | (true, 1) => 0,
        (true, 0) | (false, 1) => 8,
        _ => unreachable!("byte offset parity has two values"),
    }
}

fn sample_count(width: u32, height: u32) -> Result<usize, PilError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| overflow("sample count"))
}

fn overflow(part: &str) -> PilError {
    PilError::OverflowError(format!("I;16 Paste {part} overflowed"))
}
