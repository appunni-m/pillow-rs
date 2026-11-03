use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{GenericImageView, ImageBuffer, Pixel};

/// Read one compact carrier's omitted physical bytes before a logical write.
pub(crate) fn hidden_lanes_for_pixel(
    hidden_lanes: &Option<HiddenPixelLanes>,
    pixel_index: usize,
    lane_count: usize,
    fallback: [u8; 2],
) -> Result<[u8; 2], PilError> {
    if !(1..=2).contains(&lane_count) {
        return Err(PilError::InternalError(
            "hidden pixel lane count was invalid".into(),
        ));
    }
    let Some(hidden_lanes) = hidden_lanes else {
        return Ok(fallback);
    };
    let Some(values) = (hidden_lanes.lane_count() == lane_count)
        .then(|| hidden_lanes.for_pixel(pixel_index))
        .flatten()
    else {
        return Err(PilError::InternalError(
            "hidden pixel lane payload did not match its image".into(),
        ));
    };
    let mut output = fallback;
    output[..lane_count].copy_from_slice(values);
    Ok(output)
}

/// Update omitted bytes for a compact carrier, materializing untouched lanes
/// from the operation's mode-specific fallback only when needed.
pub(crate) fn update_hidden_lanes<P, F>(
    image: &ImageBuffer<P, Vec<u8>>,
    hidden_lanes: &mut Option<HiddenPixelLanes>,
    pixel_index: usize,
    lane_count: usize,
    values: [u8; 2],
    default: [u8; 2],
    fallback: &F,
) -> Result<(), PilError>
where
    P: Pixel<Subpixel = u8>,
    F: Fn(&P) -> [u8; 2],
{
    if !(1..=2).contains(&lane_count) {
        return Err(PilError::InternalError(
            "hidden pixel lane count was invalid".into(),
        ));
    }
    if let Some(hidden_lanes) = hidden_lanes {
        if hidden_lanes.lane_count() != lane_count
            || hidden_lanes.set_pixel(pixel_index, values).is_none()
        {
            return Err(PilError::InternalError(
                "hidden pixel lane payload did not match its image".into(),
            ));
        }
        return Ok(());
    }
    if values[..lane_count] == default[..lane_count] {
        return Ok(());
    }

    let pixel_count = usize::try_from(image.width())
        .ok()
        .and_then(|width| {
            usize::try_from(image.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::DimensionError("hidden pixel lane extent overflow".into()))?;
    let storage_size = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::DimensionError("hidden pixel lane size overflow".into()))?;
    let mut storage = Vec::with_capacity(storage_size);
    for pixel in image.pixels() {
        storage.extend_from_slice(&fallback(pixel)[..lane_count]);
    }
    let mut lanes =
        HiddenPixelLanes::per_pixel(lane_count, pixel_count, storage).ok_or_else(|| {
            PilError::InternalError("hidden pixel lane storage did not match its image".into())
        })?;
    if lanes.set_pixel(pixel_index, values).is_none() {
        return Err(PilError::InternalError(
            "hidden pixel lane index did not match its image".into(),
        ));
    }
    *hidden_lanes = Some(lanes);
    Ok(())
}
