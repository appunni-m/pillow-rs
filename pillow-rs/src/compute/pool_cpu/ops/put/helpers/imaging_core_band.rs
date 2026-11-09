//! Native byte-band mutation for Pillow's callback-visible ImagingCore API.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const WRONG_MODE: &str = "image has wrong mode";
const BAND_OUT_OF_RANGE: &str = "band index out of range";
const IMAGES_DO_NOT_MATCH: &str = "images do not match";

pub(crate) const fn is_byte_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::One
            | Mode::ImageLuma8
            | Mode::ImageLumaA8
            | Mode::ImageLaPremultiplied8
            | Mode::ImagePalette8
            | Mode::ImagePaletteAlpha8
            | Mode::ImageRgb8
            | Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageCmyk8
            | Mode::ImageYCbCr8
            | Mode::ImageLab8
            | Mode::ImageHsv8
    )
}

pub(crate) fn validate_band_index(band: i32, bands: usize) -> Result<usize, PilError> {
    usize::try_from(band)
        .ok()
        .filter(|&index| index < bands)
        .ok_or_else(|| PilError::ValueError(BAND_OUT_OF_RANGE.into()))
}

pub(crate) fn validate_put_band_source(source_mode: Mode) -> Result<(), PilError> {
    if source_mode.band_count() != 1 {
        return Err(PilError::ValueError(WRONG_MODE.into()));
    }
    Ok(())
}

pub(crate) fn validate_put_band_layout(
    destination_mode: Mode,
    source_mode: Mode,
    destination_size: (u32, u32),
    source_size: (u32, u32),
) -> Result<(), PilError> {
    let compatible = if destination_mode.band_count() == 1 {
        source_mode == destination_mode
    } else {
        is_byte_mode(destination_mode) && is_byte_mode(source_mode)
    };
    if destination_size != source_size || !compatible {
        return Err(PilError::ValueError(IMAGES_DO_NOT_MATCH.into()));
    }
    Ok(())
}

pub(crate) fn fill_band(image: &mut DynamicImage, band: usize, value: u8) -> Result<(), PilError> {
    let mode = image.mode();
    if !is_byte_mode(mode) || band >= mode.band_count() {
        return Err(PilError::InternalError(
            "FillBand received an unprepared mode or band".into(),
        ));
    }
    let bytes_per_pixel = mode.band_count();
    let byte_len = image.as_bytes().len();
    let pixels = checked_pixel_count(image.width(), image.height())?;
    let expected_len = pixels
        .checked_mul(bytes_per_pixel)
        .ok_or_else(|| PilError::DimensionError("band mutation byte count overflow".into()))?;
    if byte_len != expected_len {
        return Err(PilError::InternalError(
            "FillBand byte storage did not match its mode".into(),
        ));
    }
    let hidden_lanes = image.take_hidden_pixel_lanes();
    let Some(bytes) = image.as_bytes_mut() else {
        let _ = image.replace_hidden_pixel_lanes(hidden_lanes);
        return Err(PilError::InternalError(
            "FillBand mode has no mutable byte storage".into(),
        ));
    };
    for pixel in bytes.chunks_exact_mut(bytes_per_pixel) {
        pixel[band] = value;
    }
    if !image.replace_hidden_pixel_lanes(hidden_lanes) {
        return Err(PilError::InternalError(
            "FillBand could not restore physical pixel lanes".into(),
        ));
    }
    Ok(())
}

pub(crate) fn put_band(
    destination: &mut DynamicImage,
    source: &DynamicImage,
    band: usize,
) -> Result<(), PilError> {
    let destination_mode = destination.mode();
    if destination_mode.band_count() == 1 {
        if source.mode() != destination_mode
            || source.width() != destination.width()
            || source.height() != destination.height()
        {
            return Err(PilError::InternalError(
                "PutBand received a mismatched single-band source".into(),
            ));
        }
        *destination = source.clone();
        return Ok(());
    }
    if !is_byte_mode(destination_mode) || !is_byte_mode(source.mode()) {
        return Err(PilError::InternalError(
            "PutBand received an unprepared mode".into(),
        ));
    }
    let pixels = checked_pixel_count(destination.width(), destination.height())?;
    if source.as_bytes().len() != pixels || band >= destination_mode.band_count() {
        return Err(PilError::InternalError(
            "PutBand input did not match its prepared storage".into(),
        ));
    }
    let bytes_per_pixel = destination_mode.band_count();
    let expected_len = pixels
        .checked_mul(bytes_per_pixel)
        .ok_or_else(|| PilError::DimensionError("band mutation byte count overflow".into()))?;
    if destination.as_bytes().len() != expected_len {
        return Err(PilError::InternalError(
            "PutBand destination storage did not match its mode".into(),
        ));
    }
    let hidden_lanes = destination.take_hidden_pixel_lanes();
    let Some(destination_bytes) = destination.as_bytes_mut() else {
        let _ = destination.replace_hidden_pixel_lanes(hidden_lanes);
        return Err(PilError::InternalError(
            "PutBand destination has no mutable byte storage".into(),
        ));
    };
    for (pixel, &sample) in destination_bytes
        .chunks_exact_mut(bytes_per_pixel)
        .zip(source.as_bytes())
    {
        pixel[band] = sample;
    }
    if !destination.replace_hidden_pixel_lanes(hidden_lanes) {
        return Err(PilError::InternalError(
            "PutBand could not restore physical pixel lanes".into(),
        ));
    }
    Ok(())
}

fn checked_pixel_count(width: u32, height: u32) -> Result<usize, PilError> {
    let width = usize::try_from(width)
        .map_err(|_| PilError::DimensionError("band mutation pixel count overflow".into()))?;
    let height = usize::try_from(height)
        .map_err(|_| PilError::DimensionError("band mutation pixel count overflow".into()))?;
    width
        .checked_mul(height)
        .ok_or_else(|| PilError::DimensionError("band mutation pixel count overflow".into()))
}
