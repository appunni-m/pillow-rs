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

use image_slash_star::DynamicImage;
use image_slash_star::GrayAlphaImage;
use image_slash_star::GrayImage;
use image_slash_star::RgbImage;
use image_slash_star::RgbaImage;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

/// Converts raw pixel bytes into a [`DynamicImage`] after allocation checks.
///
/// `channels` is the number of stored bytes per pixel and must be `1`, `2`,
/// `3`, or `4`. `data` may be longer than needed; only the validated
/// `width * height * channels` prefix is copied into the image.
///
/// # Errors
///
/// Returns [`PilError::DimensionError`] if dimensions or byte counts are
/// invalid, and [`PilError::ValueError`] if `data` is too short for the declared
/// shape.
#[allow(dead_code)]
pub(crate) fn raw_bytes_to_image(
    width: u32,
    height: u32,
    channels: u8,
    data: &[u8],
) -> Result<DynamicImage, PilError> {
    let dims = CheckedDims::new(width, height, channels)?;

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

    let pixel_data = data[..dims.total_bytes()].to_vec();

    // AS PER DESIGN: Channels 1-4 are the only valid counts. The match
    // arms correspond exactly to the image crate's DynamicImage variants.
    // Channels is validated to be 1-4 by CheckedDims (zero rejected above).
    Ok(match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, pixel_data).ok_or_else(
            || PilError::InternalError("raw_bytes_to_image: L buffer shape mismatch".to_string()),
        )?),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, pixel_data).ok_or_else(|| {
                PilError::InternalError("raw_bytes_to_image: LA buffer shape mismatch".to_string())
            })?,
        ),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, pixel_data).ok_or_else(
            || PilError::InternalError("raw_bytes_to_image: RGB buffer shape mismatch".to_string()),
        )?),
        4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, pixel_data).ok_or_else(
            || {
                PilError::InternalError(
                    "raw_bytes_to_image: RGBA buffer shape mismatch".to_string(),
                )
            },
        )?),
        _ => {
            // CheckedDims validates channels ∈ [1,4], so this is unreachable.
            unreachable!("CheckedDims guarantees channels ∈ [1,4]");
        }
    })
}

/// Builds a [`DynamicImage`] from a buffer allocated for `dims`.
///
/// Use this only when `data` came from [`CheckedDims::alloc_buffer`] or an
/// equivalent checked path. Debug builds assert that the byte length still
/// matches [`CheckedDims::total_bytes`].
#[allow(dead_code)]
pub(crate) fn raw_bytes_to_image_trusted(
    dims: CheckedDims,
    data: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    if data.len() != dims.total_bytes() {
        return Err(PilError::InternalError(format!(
            "trusted buffer size mismatch: expected {}, got {}",
            dims.total_bytes(),
            data.len()
        )));
    }
    match dims.channels {
        1 => GrayImage::from_raw(dims.width, dims.height, data)
            .map(DynamicImage::ImageLuma8)
            .ok_or_else(|| {
                PilError::InternalError(
                    "raw_bytes_to_image_trusted: L buffer shape mismatch".to_string(),
                )
            }),
        2 => GrayAlphaImage::from_raw(dims.width, dims.height, data)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(|| {
                PilError::InternalError(
                    "raw_bytes_to_image_trusted: LA buffer shape mismatch".to_string(),
                )
            }),
        3 => RgbImage::from_raw(dims.width, dims.height, data)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| {
                PilError::InternalError(
                    "raw_bytes_to_image_trusted: RGB buffer shape mismatch".to_string(),
                )
            }),
        4 => RgbaImage::from_raw(dims.width, dims.height, data)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| {
                PilError::InternalError(
                    "raw_bytes_to_image_trusted: RGBA buffer shape mismatch".to_string(),
                )
            }),
        _ => unreachable!("CheckedDims guarantees channels ∈ [1,4]"),
    }
}

// AS PER DESIGN — DO NOT REMOVE: Tests validate correctness.
#[cfg(test)]
mod tests {
    use super::raw_bytes_to_image;
    use super::raw_bytes_to_image_trusted;
    use crate::CheckedDims;

    #[test]
    fn valid_rgba_image() {
        let data = vec![128u8; 100 * 100 * 4];
        let img = raw_bytes_to_image(100, 100, 4, &data).unwrap();
        assert_eq!(img.width(), 100);
        assert_eq!(img.height(), 100);
    }

    #[test]
    fn buffer_too_small() {
        let data = vec![0u8; 10];
        assert!(raw_bytes_to_image(100, 100, 4, &data).is_err());
    }

    #[test]
    fn zero_dimension_rejected() {
        let data = vec![0u8; 100];
        assert!(raw_bytes_to_image(0, 100, 3, &data).is_err());
    }

    #[test]
    fn trusted_path_matches_validated() {
        let dims = CheckedDims::new(10, 20, 3).unwrap();
        let data = dims.alloc_buffer();
        let img = raw_bytes_to_image_trusted(dims, data).unwrap();
        assert_eq!(img.width(), 10);
        assert_eq!(img.height(), 20);
    }
}
