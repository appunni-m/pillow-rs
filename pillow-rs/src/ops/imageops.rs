//! ImageOps — high-level image operations (module-level functions).
//! Mirroring PIL.ImageOps: autocontrast, equalize, invert, flip, mirror,
//! posterize, solarize, expand, scale, contain, cover, fit, pad, grayscale.

use crate::error::PilError;
use crate::image::Image;
use crate::ops::resize::parse_resample;
use crate::pipeline::PipelineOp;

/// Normalize image contrast. Clips the darkest and lightest `cutoff` percent.
pub fn autocontrast(image: &Image, cutoff: f64) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Autocontrast { cutoff }))
}

/// Equalize the image histogram.
pub fn equalize(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Equalize))
}

/// Invert all pixel values (negative).
pub fn invert(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Invert))
}

/// ImageOps.invert: raises NotImplementedError for P-mode (unlike ImageChops.invert).
pub fn invert_ops(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "P" {
        return Err(PilError::NotImplementedError(
            "mode P support coming soon".into(),
        ));
    }
    invert(image)
}

/// Flip image vertically.
pub fn flip(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Flip))
}

/// Mirror image horizontally (same as FLIP_LEFT_RIGHT).
pub fn mirror(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Mirror))
}

/// Reduce number of bits per color channel.
pub fn posterize(image: &Image, bits: u8) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(
        image,
        PipelineOp::Posterize {
            bits: bits.clamp(1, 8),
        },
    ))
}

/// Invert all pixel values above threshold.
pub fn solarize(image: &Image, threshold: u8) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Solarize { threshold }))
}

/// Convert to grayscale using PIL-compatible BT.601 formula.
pub fn grayscale(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Grayscale))
}

/// Colorize grayscale image using black/white color mapping.
pub fn colorize(
    image: &Image,
    black: (u8, u8, u8),
    white: (u8, u8, u8),
) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode != "L" {
        // PIL raises AssertionError for non-L modes
        return Err(PilError::AssertionError(String::new()));
    }
    Ok(Image::push_op(image, PipelineOp::Colorize { black, white }))
}

/// Add a border around the image.
pub fn expand(image: &Image, border: u32, fill: (u8, u8, u8, u8)) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Expand { border, fill }))
}

/// Resize image to fit within (w, h) while preserving aspect ratio.
pub fn contain(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(image, PipelineOp::Contain { w, h, filter }))
}

/// Resize image to completely cover (w, h), cropping overflow.
pub fn cover(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(image, PipelineOp::Cover { w, h, filter }))
}

/// Resize and crop to fit within (w, h) with centering.
pub fn fit(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<&str>,
    bleed: f64,
    centering: (f64, f64),
) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(
        image,
        PipelineOp::Fit {
            w,
            h,
            filter,
            bleed,
            centering,
        },
    ))
}

/// Resize and pad to exactly (w, h) with optional color fill.
pub fn pad(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<&str>,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(
        image,
        PipelineOp::Pad {
            w,
            h,
            filter,
            color,
            centering,
        },
    ))
}

/// Scale image by a factor.
pub fn scale(image: &Image, factor: f64, filter: Option<&str>) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(image, PipelineOp::Scale { factor, filter }))
}

/// Crop border pixels from the image.
pub fn crop(image: &Image, border: u32) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::CropBorder { border }))
}

/// Extract Orientation tag (0x0112) from raw EXIF bytes. Returns None if not found.
///
/// Scans TIFF IFD0 entries looking for tag 0x0112. Handles both TIFF and
/// Exif-JPEG (starts with "Exif\0\0") formats. Supports little-endian (II)
/// and big-endian (MM) byte orders.
pub fn exif_get_orientation(raw: &[u8]) -> Option<u32> {
    if raw.is_empty() || raw.len() < 8 {
        return None;
    }

    // Skip EXIF header if present (Exif-JPEG format)
    let data = if raw.starts_with(b"Exif\x00\x00") && raw.len() > 6 {
        &raw[6..]
    } else {
        raw
    };

    if data.len() < 8 {
        return None;
    }

    // Determine byte order
    let le = match &data[..2] {
        b"II" => true,  // Little-endian
        b"MM" => false, // Big-endian
        _ => return None,
    };

    // Check TIFF magic number (42)
    let magic = if le {
        u16::from_le_bytes([data[2], data[3]])
    } else {
        u16::from_be_bytes([data[2], data[3]])
    };
    if magic != 42 {
        return None;
    }

    // Get IFD0 offset
    let ifd_offset = if le {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]])
    } else {
        u32::from_be_bytes([data[4], data[5], data[6], data[7]])
    } as usize;

    if ifd_offset + 2 > data.len() {
        return None;
    }

    // Number of IFD entries
    let num_entries = if le {
        u16::from_le_bytes([data[ifd_offset], data[ifd_offset + 1]])
    } else {
        u16::from_be_bytes([data[ifd_offset], data[ifd_offset + 1]])
    } as usize;

    // Scan IFD entries for Orientation tag (0x0112)
    for i in 0..num_entries {
        let entry_start = ifd_offset + 2 + i * 12;
        if entry_start + 12 > data.len() {
            break;
        }
        let tag = if le {
            u16::from_le_bytes([data[entry_start], data[entry_start + 1]])
        } else {
            u16::from_be_bytes([data[entry_start], data[entry_start + 1]])
        };
        if tag == 0x0112 {
            // Orientation value is at entry_start + 8, SHORT type (2 bytes)
            let value = if le {
                u16::from_le_bytes([data[entry_start + 8], data[entry_start + 9]])
            } else {
                u16::from_be_bytes([data[entry_start + 8], data[entry_start + 9]])
            };
            if (1..=8).contains(&value) {
                return Some(value as u32);
            }
            return None;
        }
    }

    None
}

/// Remove Orientation tag from EXIF bytes by zeroing its tag field.
///
/// Returns the modified EXIF bytes. If no orientation tag is found,
/// or the data is invalid, returns a copy of the original bytes.
pub fn exif_remove_orientation(raw: &[u8]) -> Vec<u8> {
    if raw.len() < 14 {
        return raw.to_vec();
    }

    let header_len = if raw.starts_with(b"Exif\x00\x00") {
        6
    } else {
        0
    };

    if raw.len() - header_len < 8 {
        return raw.to_vec();
    }

    let data = &raw[header_len..];

    let le = match &data[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return raw.to_vec(),
    };

    let magic = if le {
        u16::from_le_bytes([data[2], data[3]])
    } else {
        u16::from_be_bytes([data[2], data[3]])
    };
    if magic != 42 {
        return raw.to_vec();
    }

    let ifd_offset = if le {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]])
    } else {
        u32::from_be_bytes([data[4], data[5], data[6], data[7]])
    } as usize;

    let abs_ifd = header_len + ifd_offset;
    if abs_ifd + 2 > raw.len() {
        return raw.to_vec();
    }

    let num_entries = if le {
        u16::from_le_bytes([raw[abs_ifd], raw[abs_ifd + 1]])
    } else {
        u16::from_be_bytes([raw[abs_ifd], raw[abs_ifd + 1]])
    } as usize;

    let mut result = raw.to_vec();

    for i in 0..num_entries {
        let entry_start = abs_ifd + 2 + i * 12;
        if entry_start + 12 > result.len() {
            break;
        }
        let tag = if le {
            u16::from_le_bytes([result[entry_start], result[entry_start + 1]])
        } else {
            u16::from_be_bytes([result[entry_start], result[entry_start + 1]])
        };
        if tag == 0x0112 {
            // Orientation
            // Zero out the tag to indicate "no tag"
            result[entry_start] = 0;
            result[entry_start + 1] = 0;
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exif_get_orientation_empty() {
        assert_eq!(exif_get_orientation(&[]), None);
        assert_eq!(exif_get_orientation(b"abc"), None);
    }

    #[test]
    fn test_exif_get_orientation_no_exif() {
        // Valid TIFF header but no orientation tag
        // II (little-endian), magic=42, IFD0 at offset 8, 0 entries
        let raw = b"II\x2a\x00\x08\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(exif_get_orientation(raw), None);
    }

    #[test]
    fn test_exif_get_orientation_with_tag_le() {
        // Little-endian TIFF with orientation=6 (ROTATE_270)
        // II, magic=42 (2a 00), IFD0 offset=8, 1 entry
        // Entry: tag=0112, type=SHORT(03), count=1, value=6
        let mut raw = Vec::new();
        raw.extend_from_slice(b"II\x2a\x00"); // Byte order + magic
        raw.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset
        raw.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
        raw.extend_from_slice(&0x0112u16.to_le_bytes()); // tag = Orientation
        raw.extend_from_slice(&3u16.to_le_bytes()); // type = SHORT
        raw.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        raw.extend_from_slice(&6u16.to_le_bytes()); // value = 6
        raw.extend_from_slice(&[0u8; 2]); // padding

        assert_eq!(exif_get_orientation(&raw), Some(6));
    }

    #[test]
    fn test_exif_get_orientation_with_tag_be() {
        // Big-endian TIFF with orientation=3 (ROTATE_180)
        let mut raw = Vec::new();
        raw.extend_from_slice(b"MM\x00\x2a"); // Byte order + magic
        raw.extend_from_slice(&8u32.to_be_bytes()); // IFD0 offset
        raw.extend_from_slice(&1u16.to_be_bytes()); // 1 entry
        raw.extend_from_slice(&0x0112u16.to_be_bytes()); // tag
        raw.extend_from_slice(&3u16.to_be_bytes()); // type
        raw.extend_from_slice(&1u32.to_be_bytes()); // count
        raw.extend_from_slice(&3u16.to_be_bytes()); // value = 3
        raw.extend_from_slice(&[0u8; 2]); // padding

        assert_eq!(exif_get_orientation(&raw), Some(3));
    }

    #[test]
    fn test_exif_get_orientation_exif_jpeg() {
        // Exif-JPEG format: starts with "Exif\0\0", then TIFF
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II\x2a\x00");
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&8u16.to_le_bytes()); // value = 8
        tiff.extend_from_slice(&[0u8; 2]);

        let mut raw = Vec::new();
        raw.extend_from_slice(b"Exif\x00\x00");
        raw.extend_from_slice(&tiff);

        assert_eq!(exif_get_orientation(&raw), Some(8));
    }

    #[test]
    fn test_exif_remove_orientation() {
        // Create TIFF with orientation=6
        let mut raw = Vec::new();
        raw.extend_from_slice(b"II\x2a\x00");
        raw.extend_from_slice(&8u32.to_le_bytes());
        raw.extend_from_slice(&1u16.to_le_bytes());
        // orientation tag entry (12 bytes)
        let orientation_start = raw.len();
        raw.extend_from_slice(&0x0112u16.to_le_bytes());
        raw.extend_from_slice(&3u16.to_le_bytes());
        raw.extend_from_slice(&1u32.to_le_bytes());
        raw.extend_from_slice(&6u16.to_le_bytes());
        raw.extend_from_slice(&[0u8; 2]);

        assert_eq!(exif_get_orientation(&raw), Some(6));

        let cleaned = exif_remove_orientation(&raw);
        // After removal, orientation tag should be zeroed
        assert_eq!(cleaned[orientation_start], 0);
        assert_eq!(cleaned[orientation_start + 1], 0);
        // exif_get_orientation should now return None
        assert_eq!(exif_get_orientation(&cleaned), None);
    }

    #[test]
    fn test_exif_remove_orientation_no_orientation() {
        // TIFF with 0 entries — no orientation to remove
        let raw = b"II\x2a\x00\x08\x00\x00\x00\x00\x00\x00\x00";
        let result = exif_remove_orientation(raw);
        assert_eq!(result, raw);
    }

    #[test]
    fn test_exif_remove_orientation_too_short() {
        let raw = b"Exif\x00\x00";
        let result = exif_remove_orientation(raw);
        assert_eq!(result, raw);
    }
}
