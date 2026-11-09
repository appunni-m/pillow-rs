use crate::error::PilError;
use crate::raster::Mode;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{ImageBuffer, Pixel};

/// Returns the canonical omitted bytes for a logical pixel when no producer
/// state was retained for its compact carrier.
pub(crate) fn default_hidden_pixel_lanes(mode: Mode, first_sample: u8) -> Option<[u8; 2]> {
    match mode {
        // LA/La repeat luma; PA repeats its palette index in the omitted slots.
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            Some([first_sample, first_sample])
        }
        // Canonical X value; Convert strategies retain source-specific values separately.
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            Some([u8::MAX, 0])
        }
        _ => None,
    }
}

/// Materializes canonical omitted bytes from a compact logical raster.
pub(crate) fn default_hidden_lanes_for_bytes(
    mode: Mode,
    logical_bytes: &[u8],
    pixel_count: usize,
) -> Result<Option<HiddenPixelLanes>, PilError> {
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(None);
    };
    if pixel_count == 0 {
        return Ok(None);
    }
    let fallback = default_hidden_pixel_lanes(mode, 0)
        .ok_or_else(|| PilError::InternalError("mode omitted its physical-lane fallback".into()))?;
    let carrier_width = mode.byte_carrier_pixel_width().ok_or_else(|| {
        PilError::InternalError("mode omitted its byte-carrier pixel width".into())
    })?;
    let logical_size = pixel_count
        .checked_mul(carrier_width)
        .ok_or_else(|| PilError::InternalError("logical pixel extent overflow".into()))?;
    let logical_bytes = logical_bytes.get(..logical_size).ok_or_else(|| {
        PilError::InternalError("logical pixel bytes did not match the mode extent".into())
    })?;
    if lane_count == 1 {
        return HiddenPixelLanes::constant(lane_count, pixel_count, fallback)
            .map(Some)
            .ok_or_else(|| PilError::InternalError("physical-lane extent was invalid".into()));
    }
    let lane_size = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::InternalError("physical-lane extent overflow".into()))?;
    let mut values = Vec::with_capacity(lane_size);
    for pixel in logical_bytes.chunks_exact(carrier_width) {
        let lanes = default_hidden_pixel_lanes(mode, pixel[0]).ok_or_else(|| {
            PilError::InternalError("mode omitted its physical-lane fallback".into())
        })?;
        values.extend_from_slice(&lanes[..lane_count]);
    }
    HiddenPixelLanes::per_pixel(lane_count, pixel_count, values)
        .map(Some)
        .ok_or_else(|| PilError::InternalError("physical-lane extent was invalid".into()))
}

/// Read one compact carrier's omitted physical bytes before a logical write.
pub(crate) fn hidden_lanes_for_pixel(
    hidden_lanes: &Option<HiddenPixelLanes>,
    pixel_index: usize,
    lane_count: usize,
    fallback: [u8; 2],
) -> Result<[u8; 2], PilError> {
    hidden_lanes_for_pixel_ref(hidden_lanes.as_ref(), pixel_index, lane_count, fallback)
}

/// Read omitted physical bytes from a borrowed sidecar or a mode fallback.
pub(crate) fn hidden_lanes_for_pixel_ref(
    hidden_lanes: Option<&HiddenPixelLanes>,
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
