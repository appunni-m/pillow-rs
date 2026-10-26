//! Native-domain extrema formatting for scalar image modes.

use crate::error::PilError;
use crate::image::FormattedExtrema;
use crate::raster::DynamicImage;

pub(super) fn integer(image: &DynamicImage) -> Result<FormattedExtrema, PilError> {
    let (minimum, maximum) = image
        .as_bytes()
        .chunks_exact(4)
        .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .fold(None, |extrema: Option<(i32, i32)>, value| {
            Some(match extrema {
                Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                None => (value, value),
            })
        })
        .ok_or_else(|| PilError::InternalError("I image has no scalar samples".into()))?;
    Ok(FormattedExtrema::Integer((minimum, maximum)))
}

pub(super) fn luma16(image: &DynamicImage) -> Result<FormattedExtrema, PilError> {
    let luma = image.to_luma16();
    let values = luma.as_raw().iter().copied().map(i32::from);
    let (minimum, maximum) = values
        .fold(None, |extrema: Option<(i32, i32)>, value| {
            Some(match extrema {
                Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                None => (value, value),
            })
        })
        .ok_or_else(|| PilError::InternalError("I;16 image has no samples".into()))?;
    Ok(FormattedExtrema::Integer((minimum, maximum)))
}

pub(super) fn float(image: &DynamicImage) -> Result<FormattedExtrema, PilError> {
    let (minimum, maximum) = image
        .as_bytes()
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) as f64)
        .fold(None, |extrema: Option<(f64, f64)>, value| {
            Some(match extrema {
                Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                None => (value, value),
            })
        })
        .ok_or_else(|| PilError::InternalError("F image has no scalar samples".into()))?;
    Ok(FormattedExtrema::Float((minimum, maximum)))
}
