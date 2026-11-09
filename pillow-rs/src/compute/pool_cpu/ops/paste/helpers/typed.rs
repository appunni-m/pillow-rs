use crate::error::PilError;

use super::plan::{NativeF32Plan, NativeU16Plan};

/// Executes an internal native u16 Paste extension without mode conversion.
pub(crate) fn execute_u16<const CHANNELS: usize>(
    plan: NativeU16Plan<'_>,
    source: &[u16],
) -> Result<Vec<u16>, PilError> {
    let destination_samples = sample_count(plan.destination_size)?
        .checked_mul(CHANNELS)
        .ok_or_else(|| overflow("destination sample count"))?;
    let source_samples = sample_count(plan.source_size)?
        .checked_mul(CHANNELS)
        .ok_or_else(|| overflow("source sample count"))?;
    if plan.destination.len() != destination_samples || source.len() != source_samples {
        return Err(PilError::InternalError(
            "native u16 Paste storage did not match dimensions".into(),
        ));
    }
    let mut output = plan.destination.to_vec();
    if plan.region.is_empty() {
        return Ok(output);
    }
    let mask_bytes = plan.mask.as_deref().map(|mask| mask.as_bytes());
    let mask_layout = plan.mask_layout;
    let source_width = usize::try_from(plan.source_size.0).map_err(|_| overflow("source width"))?;
    let destination_width =
        usize::try_from(plan.destination_size.0).map_err(|_| overflow("destination width"))?;
    let region_width = usize::try_from(plan.source_size.0).map_err(|_| overflow("mask width"))?;

    for row in 0..usize::try_from(plan.region.height).map_err(|_| overflow("copy height"))? {
        let source_y =
            usize::try_from(plan.region.source_top).map_err(|_| overflow("source y"))? + row;
        let destination_y = usize::try_from(plan.region.destination_top)
            .map_err(|_| overflow("destination y"))?
            + row;
        for column in 0..usize::try_from(plan.region.width).map_err(|_| overflow("copy width"))? {
            let source_x = usize::try_from(plan.region.source_left)
                .map_err(|_| overflow("source x"))?
                + column;
            let destination_x = usize::try_from(plan.region.destination_left)
                .map_err(|_| overflow("destination x"))?
                + column;
            let source_index = (source_y * source_width + source_x) * CHANNELS;
            let destination_index = (destination_y * destination_width + destination_x) * CHANNELS;
            let Some((mask, layout)) = mask_bytes.zip(mask_layout) else {
                output[destination_index..destination_index + CHANNELS]
                    .copy_from_slice(&source[source_index..source_index + CHANNELS]);
                continue;
            };
            let mask_index =
                (source_y * region_width + source_x) * layout.channels + layout.value_index;
            let value = mask[mask_index];
            if value == 0 {
                continue;
            }
            if value == 255 || layout.binary {
                output[destination_index..destination_index + CHANNELS]
                    .copy_from_slice(&source[source_index..source_index + CHANNELS]);
                continue;
            }
            blend_u16::<CHANNELS>(
                &source[source_index..source_index + CHANNELS],
                &mut output[destination_index..destination_index + CHANNELS],
                value,
            );
        }
    }
    Ok(output)
}

/// Executes an internal native f32 Paste extension without mode conversion.
pub(crate) fn execute_f32<const CHANNELS: usize>(
    plan: NativeF32Plan<'_>,
    source: &[f32],
) -> Result<Vec<f32>, PilError> {
    let destination_samples = sample_count(plan.destination_size)?
        .checked_mul(CHANNELS)
        .ok_or_else(|| overflow("destination sample count"))?;
    let source_samples = sample_count(plan.source_size)?
        .checked_mul(CHANNELS)
        .ok_or_else(|| overflow("source sample count"))?;
    if plan.destination.len() != destination_samples || source.len() != source_samples {
        return Err(PilError::InternalError(
            "native f32 Paste storage did not match dimensions".into(),
        ));
    }
    let mut output = plan.destination.to_vec();
    if plan.region.is_empty() {
        return Ok(output);
    }
    let mask_bytes = plan.mask.as_deref().map(|mask| mask.as_bytes());
    let mask_layout = plan.mask_layout;
    let source_width = usize::try_from(plan.source_size.0).map_err(|_| overflow("source width"))?;
    let destination_width =
        usize::try_from(plan.destination_size.0).map_err(|_| overflow("destination width"))?;
    let region_width = usize::try_from(plan.source_size.0).map_err(|_| overflow("mask width"))?;

    for row in 0..usize::try_from(plan.region.height).map_err(|_| overflow("copy height"))? {
        let source_y =
            usize::try_from(plan.region.source_top).map_err(|_| overflow("source y"))? + row;
        let destination_y = usize::try_from(plan.region.destination_top)
            .map_err(|_| overflow("destination y"))?
            + row;
        for column in 0..usize::try_from(plan.region.width).map_err(|_| overflow("copy width"))? {
            let source_x = usize::try_from(plan.region.source_left)
                .map_err(|_| overflow("source x"))?
                + column;
            let destination_x = usize::try_from(plan.region.destination_left)
                .map_err(|_| overflow("destination x"))?
                + column;
            let source_index = (source_y * source_width + source_x) * CHANNELS;
            let destination_index = (destination_y * destination_width + destination_x) * CHANNELS;
            let Some((mask, layout)) = mask_bytes.zip(mask_layout) else {
                output[destination_index..destination_index + CHANNELS]
                    .copy_from_slice(&source[source_index..source_index + CHANNELS]);
                continue;
            };
            let mask_index =
                (source_y * region_width + source_x) * layout.channels + layout.value_index;
            let value = mask[mask_index];
            if value == 0 {
                continue;
            }
            if value == 255 || layout.binary {
                output[destination_index..destination_index + CHANNELS]
                    .copy_from_slice(&source[source_index..source_index + CHANNELS]);
                continue;
            }
            blend_f32::<CHANNELS>(
                &source[source_index..source_index + CHANNELS],
                &mut output[destination_index..destination_index + CHANNELS],
                value,
            );
        }
    }
    Ok(output)
}

#[inline(always)]
fn blend_u16<const CHANNELS: usize>(source: &[u16], destination: &mut [u16], mask: u8) {
    let weight = u32::from(mask);
    let inverse = 255 - weight;
    for band in 0..CHANNELS {
        destination[band] =
            ((u32::from(source[band]) * weight + u32::from(destination[band]) * inverse + 127)
                / 255) as u16;
    }
}

#[inline(always)]
fn blend_f32<const CHANNELS: usize>(source: &[f32], destination: &mut [f32], mask: u8) {
    let weight = f32::from(mask) / 255.0;
    let inverse = 1.0 - weight;
    for band in 0..CHANNELS {
        destination[band] = source[band] * weight + destination[band] * inverse;
    }
}

fn sample_count(dimensions: (u32, u32)) -> Result<usize, PilError> {
    usize::try_from(dimensions.0)
        .ok()
        .and_then(|width| {
            usize::try_from(dimensions.1)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| overflow("sample count"))
}

fn overflow(part: &str) -> PilError {
    PilError::OverflowError(format!("native Paste {part} overflowed"))
}
