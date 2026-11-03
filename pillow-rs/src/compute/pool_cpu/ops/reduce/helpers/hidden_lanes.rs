//! Omitted physical-byte behavior for Pillow Reduce.
//!
//! Pillow's `Reduce.c` writes zero to physical bytes outside the logical bands
//! for two- and three-band outputs. A 1x1 factor is instead an image copy and
//! retains its source bytes. The exact source audit is in
//! `docs/CPU_REFACTOR_INVENTORY.md`.

use crate::error::PilError;
use crate::raster::{DynamicImage, buffer::HiddenPixelLanes};

/// Preserves copied lanes or installs Pillow's zeroed reduction padding.
pub(crate) fn finish(
    source: &DynamicImage,
    output: &mut DynamicImage,
    factors: (u32, u32),
) -> Result<(), PilError> {
    if output.mode() != source.mode() {
        return Err(PilError::InternalError(
            "Reduce output changed mode while composing physical lanes".into(),
        ));
    }
    let Some(lane_count) = source.mode().hidden_pixel_lane_count() else {
        return Ok(());
    };
    if factors.0 < 2 && factors.1 < 2 {
        if output.hidden_pixel_lanes() == source.hidden_pixel_lanes() {
            return Ok(());
        }
        if !output.remap_hidden_pixel_lanes_from(source, |index| index) {
            return Err(invalid_lane_output());
        }
        return Ok(());
    }

    let pixel_count = usize::try_from(output.width())
        .ok()
        .and_then(|width| {
            usize::try_from(output.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::DimensionError("Reduce lane output extent overflow".into()))?;
    let lanes = HiddenPixelLanes::constant(lane_count, pixel_count, [0; 2])
        .ok_or_else(invalid_lane_output)?;
    if !output.replace_hidden_pixel_lanes(Some(lanes)) {
        return Err(invalid_lane_output());
    }
    Ok(())
}

fn invalid_lane_output() -> PilError {
    PilError::InternalError("Reduce physical lane output was invalid".into())
}
