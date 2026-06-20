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

use pillow_rs_image::DynamicImage;
use pillow_rs_image::GrayAlphaImage;
use pillow_rs_image::GrayImage;
use pillow_rs_image::RgbImage;
use pillow_rs_image::RgbaImage;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

/// AS PER DESIGN — DO NOT REMOVE:
/// The ONE canonical function for raw-bytes → DynamicImage conversion.
/// Every call site that creates a DynamicImage from a byte buffer MUST use this.
///
/// Validates:
/// 1. Dimensions via CheckedDims (overflow + DoS protection)
/// 2. Buffer size matches declared dimensions
///
/// # Errors
/// - `DimensionError` if dimensions are invalid
/// - `ValueError` if buffer is too small for the declared dimensions
pub fn raw_bytes_to_image(
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
        1 => DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, pixel_data)
                .expect("CheckedDims guarantees buf.len() == w*h*ch for ch=1"),
        ),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, pixel_data)
                .expect("CheckedDims guarantees buf.len() == w*h*ch for ch=2"),
        ),
        3 => DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, pixel_data)
                .expect("CheckedDims guarantees buf.len() == w*h*ch for ch=3"),
        ),
        4 => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, pixel_data)
                .expect("CheckedDims guarantees buf.len() == w*h*ch for ch=4"),
        ),
        _ => {
            // CheckedDims validates channels ∈ [1,4], so this is unreachable.
            unreachable!("CheckedDims guarantees channels ∈ [1,4]");
        }
    })
}

/// Create a DynamicImage from a buffer that was pre-allocated via
/// CheckedDims::alloc_buffer(). No additional validation needed.
///
/// AS PER DESIGN: Use this when you already have a CheckedDims and a buffer
/// you allocated from it. The sizes are guaranteed to match.
pub fn raw_bytes_to_image_trusted(dims: CheckedDims, data: Vec<u8>) -> DynamicImage {
    debug_assert_eq!(
        data.len(),
        dims.total_bytes(),
        "Trusted buffer size mismatch: expected {}, got {}",
        dims.total_bytes(),
        data.len()
    );
    match dims.channels {
        1 => DynamicImage::ImageLuma8(
            GrayImage::from_raw(dims.width, dims.height, data)
                .expect("CheckedDims guarantees correct size for ch=1"),
        ),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(dims.width, dims.height, data)
                .expect("CheckedDims guarantees correct size for ch=2"),
        ),
        3 => DynamicImage::ImageRgb8(
            RgbImage::from_raw(dims.width, dims.height, data)
                .expect("CheckedDims guarantees correct size for ch=3"),
        ),
        4 => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(dims.width, dims.height, data)
                .expect("CheckedDims guarantees correct size for ch=4"),
        ),
        _ => unreachable!("CheckedDims guarantees channels ∈ [1,4]"),
    }
}

// AS PER DESIGN — DO NOT REMOVE: Tests validate correctness.
#[cfg(test)]
mod tests {
    use super::*;

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
        let img = raw_bytes_to_image_trusted(dims, data);
        assert_eq!(img.width(), 10);
        assert_eq!(img.height(), 20);
    }
}
