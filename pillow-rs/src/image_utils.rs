// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   This is the ONE canonical function for converting raw bytes to a
//   DynamicImage. The same logic previously existed in three separate files
//   (image.rs:1646, geometry.rs:100, filter.rs:30). Those copies are banned.
//
//   CI enforces: no match-on-channels + from_raw pattern outside this file
//   (see scripts/check_no_duplicate_raw_bytes.sh).
//
//   Use this function whenever you need to reconstruct a DynamicImage from
//   raw pixel data. It validates dimensions via CheckedDims before allocation.
// ============================================================================

use crate::raster::DynamicImage;
use crate::raster::GrayAlphaImage;
use crate::raster::GrayImage;
use crate::raster::RgbImage;
use crate::raster::RgbaImage;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

/// Converts raw pixel bytes into a [`DynamicImage`] after allocation checks.
///
/// `channels` is the number of stored bytes per pixel and must be `1`, `2`,
/// `3`, or `4`. `data` may be longer than needed; only the validated
/// `width * height * channels` prefix is consumed.
///
/// # Errors
///
/// Returns [`PilError::DimensionError`] if dimensions or byte counts are
/// invalid, and [`PilError::ValueError`] if `data` is too short for the declared
/// shape.
pub(crate) fn raw_bytes_to_image(
    width: u32,
    height: u32,
    mut data: Vec<u8>,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    if !(1..=4).contains(&channels) {
        return Err(PilError::ValueError(format!(
            "raw_bytes_to_image: unsupported channel count {channels}"
        )));
    }

    let dims = CheckedDims::new(width, height, channels as u8)?;

    if data.len() < dims.total_bytes() {
        return Err(PilError::ValueError(format!(
            "raw_bytes_to_image: expected {} bytes for {}×{}×{}, got {}",
            dims.total_bytes(),
            dims.width,
            dims.height,
            dims.channels,
            data.len()
        )));
    }

    data.truncate(dims.total_bytes());

    // Channels 1-4 are the only valid counts. The match arms correspond
    // exactly to the image crate's DynamicImage variants.
    Ok(match channels {
        1 => {
            DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, data).ok_or_else(|| {
                PilError::InternalError("raw_bytes_to_image: L buffer shape mismatch".to_string())
            })?)
        }
        2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(width, height, data).ok_or_else(
            || PilError::InternalError("raw_bytes_to_image: LA buffer shape mismatch".to_string()),
        )?),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, data).ok_or_else(|| {
            PilError::InternalError("raw_bytes_to_image: RGB buffer shape mismatch".to_string())
        })?),
        4 => {
            DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, data).ok_or_else(|| {
                PilError::InternalError(
                    "raw_bytes_to_image: RGBA buffer shape mismatch".to_string(),
                )
            })?)
        }
        _ => unreachable!("channel count was validated above"),
    })
}
