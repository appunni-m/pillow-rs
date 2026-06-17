//! AVIF decoder — ISOBMFF container parsing with feature-gated decoding.
//!
//! AVIF (AV1 Image File Format) uses an ISOBMFF (ISO Base Media File Format)
//! container. This decoder:
//!
//! 1. Parses the top-level ISOBMFF boxes to identify AVIF structure.
//! 2. Detects the `ftyp` box for AVIF identification.
//! 3. Locates the `mdat` box containing the AV1 bitstream.
//! 4. Reads image dimensions from the `av1C` or `ispe` box in `moov`/`meta`.
//!
//! Feature-gated: full parsing requires the `avif` feature. Without it the
//! decoder only confirms the format via `ftyp` brand check.
//!
//! References:
//!   - AVIF Specification: https://aomediacodec.github.io/av1-avif/
//!   - ISOBMFF (ISO 14496-12)

use crate::types::DecodedImage;
#[cfg(feature = "avif")]
use crate::types::ColorType;

/// Maximum reasonable image dimension to avoid OOM on corrupted data
#[cfg(feature = "avif")]
const MAX_DIM: u32 = 16384;

/// Decode an AVIF image from raw bytes.
///
/// When the `avif` feature is enabled, this parses the ISOBMFF container to
/// extract dimensions and returns a correctly-sized placeholder. Without the
/// feature it returns `None`.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    #[cfg(not(feature = "avif"))]
    {
        // Without the `avif` feature, only confirm AVIF format
        let _ = data;
        None
    }

    #[cfg(feature = "avif")]
    {
        decode_avif(data)
    }
}

/// Full AVIF decoding (only compiled when `feature = "avif"`).
#[cfg(feature = "avif")]
fn decode_avif(data: &[u8]) -> Option<DecodedImage> {
    if data.len() < 12 {
        return None;
    }

    let mut offset = 0usize;
    let mut found_ftyp = false;
    let mut is_avif = false;
    let mut width: u32 = 0;
    let mut height: u32 = 0;

    while offset + 8 <= data.len() {
        let box_size = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;

        let box_type = &data[offset + 4..offset + 8];

        if box_size < 8 {
            break;
        }

        let actual_size = if box_size == 1 {
            // Extended size (64-bit)
            if offset + 16 > data.len() {
                break;
            }
            let hi = u64::from_be_bytes([
                data[offset + 8], data[offset + 9], data[offset + 10],
                data[offset + 11], data[offset + 12], data[offset + 13],
                data[offset + 14], data[offset + 15],
            ]);
            hi as usize
        } else {
            box_size
        };

        if actual_size < 8 || offset + actual_size > data.len() {
            break;
        }

        match box_type {
            b"ftyp" => {
                found_ftyp = true;
                if let Some(box_data) = data.get(offset + 8..offset + actual_size) {
                    if box_data.len() >= 8 {
                        let remaining = &box_data[8..];
                        for brand in remaining.chunks(4) {
                            if brand == b"avif" || brand == b"avis" {
                                is_avif = true;
                                break;
                            }
                        }
                    }
                }
            }
            b"meta" => {
                // meta is a full box: skip version(1) + flags(3) before sub-boxes
                if let Some(box_data) = data.get(offset + 12..offset + actual_size) {
                    if let Some((w, h)) = parse_avif_dimensions(box_data) {
                        width = w;
                        height = h;
                    }
                }
            }
            b"moov" => {
                // moov is a regular box
                if let Some(box_data) = data.get(offset + 8..offset + actual_size) {
                    if let Some((w, h)) = parse_avif_dimensions(box_data) {
                        width = w;
                        height = h;
                    }
                }
            }
            b"mdat" => {
                // Media data box — full AV1 bitstream decode not yet implemented
            }
            _ => {}
        }

        offset += actual_size;
    }

    if !found_ftyp || !is_avif {
        return None;
    }

    if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
        return None;
    }

    // Return a correctly-sized placeholder (purple-tinted checkerboard RGBA)
    let num_pixels = (width as u64 * height as u64) as usize;
    let mut pixels = vec![0u8; num_pixels.saturating_mul(4)];
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) as usize) * 4;
            if (x / 8 + y / 8) % 2 == 0 {
                pixels[idx] = 118;
                pixels[idx + 1] = 98;
                pixels[idx + 2] = 128;
            } else {
                pixels[idx] = 190;
                pixels[idx + 1] = 170;
                pixels[idx + 2] = 200;
            }
            pixels[idx + 3] = 255;
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Parse AVIF dimensions from a `meta` or `moov` box recursively.
///
/// Looks for the `ispe` (Image Spatial Extent) property inside the
/// item property chain to extract width and height.
///
/// Note: `meta` and `ispe` are ISOBMFF "full boxes" — they start with
/// a 4-byte version+flags header after the 8-byte box header. Other
/// boxes like `iprp`/`ipco` are regular boxes with sub-boxes directly.
#[cfg(feature = "avif")]
fn parse_avif_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let mut offset = 0usize;

    while offset + 8 <= data.len() {
        let box_size = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;

        let box_type = &data[offset + 4..offset + 8];

        if box_size < 8 || offset + box_size > data.len() {
            break;
        }

        match box_type {
            b"ispe" => {
                // ispe is a full box: version(1) + flags(3) + width(4) + height(4)
                let payload = data.get(offset + 8..offset + box_size)?;
                // Payload = version(1) + flags(3) + width(4) + height(4) = 12 bytes
                if payload.len() >= 12 {
                    let w = u32::from_be_bytes([
                        payload[4], payload[5],
                        payload[6], payload[7],
                    ]);
                    let h = u32::from_be_bytes([
                        payload[8], payload[9],
                        payload[10], payload[11],
                    ]);
                    if w > 0 && h > 0 && w <= MAX_DIM && h <= MAX_DIM {
                        return Some((w, h));
                    }
                }
                return None;
            }
            b"meta" => {
                // meta is a full box: skip version(1) + flags(3) before sub-boxes
                if let Some(inner) = data.get(offset + 12..offset + box_size) {
                    if let Some(dims) = parse_avif_dimensions(inner) {
                        return Some(dims);
                    }
                }
            }
            b"iprp" | b"ipco" | b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" => {
                // Regular boxes: recurse directly into contents
                if let Some(inner) = data.get(offset + 8..offset + box_size) {
                    if let Some(dims) = parse_avif_dimensions(inner) {
                        return Some(dims);
                    }
                }
            }
            _ => {}
        }

        offset += box_size;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal ISOBMFF box.
    fn make_box(box_type: &[u8; 4], contents: &[u8]) -> Vec<u8> {
        let mut box_data = Vec::new();
        let total_size = 8 + contents.len();
        box_data.extend_from_slice(&(total_size as u32).to_be_bytes());
        box_data.extend_from_slice(box_type);
        box_data.extend_from_slice(contents);
        box_data
    }

    /// Build a minimal AVIF file with ftyp and ispe for dimensions.
    fn build_avif(width: u32, height: u32) -> Vec<u8> {
        let mut avif = Vec::new();

        // ftyp box
        let mut ftyp_data = Vec::new();
        ftyp_data.extend_from_slice(b"avif");
        ftyp_data.extend_from_slice(&[0u8; 4]);
        ftyp_data.extend_from_slice(b"avif");
        avif.extend_from_slice(&make_box(b"ftyp", &ftyp_data));

        // meta → iprp → ipco → ispe
        let mut ispe_data = vec![0u8; 4];
        ispe_data.extend_from_slice(&width.to_be_bytes());
        ispe_data.extend_from_slice(&height.to_be_bytes());
        let ispe_box = make_box(b"ispe", &ispe_data);
        let ipco_box = make_box(b"ipco", &ispe_box);
        let iprp_box = make_box(b"iprp", &ipco_box);

        let mut meta_data = vec![0u8; 4];
        meta_data.extend_from_slice(&iprp_box);
        avif.extend_from_slice(&make_box(b"meta", &meta_data));

        // mdat placeholder
        avif.extend_from_slice(&make_box(b"mdat", &[0u8; 16]));
        avif
    }

    #[cfg(feature = "avif")]
    fn build_avis_brand(width: u32, height: u32) -> Vec<u8> {
        let mut data = Vec::new();

        let mut ftyp_data = Vec::new();
        ftyp_data.extend_from_slice(b"avis");
        ftyp_data.extend_from_slice(&[0u8; 4]);
        ftyp_data.extend_from_slice(b"avis");
        data.extend_from_slice(&make_box(b"ftyp", &ftyp_data));

        let mut ispe_data = vec![0u8; 4];
        ispe_data.extend_from_slice(&width.to_be_bytes());
        ispe_data.extend_from_slice(&height.to_be_bytes());
        let ispe_box = make_box(b"ispe", &ispe_data);
        let ipco_box = make_box(b"ipco", &ispe_box);
        let iprp_box = make_box(b"iprp", &ipco_box);

        let mut meta_data = vec![0u8; 4];
        meta_data.extend_from_slice(&iprp_box);
        data.extend_from_slice(&make_box(b"meta", &meta_data));

        data
    }

    #[test]
    fn test_not_avif() {
        assert!(decode(b"not avif").is_none());
    }

    #[test]
    fn test_avif_too_small() {
        assert!(decode(b"\x00\x00\x00\x08ftypavif").is_none());
    }

    #[test]
    fn test_avif_no_ftyp() {
        let data = make_box(b"mdat", &[0u8; 8]);
        assert!(decode(&data).is_none());
    }

    #[test]
    fn test_avif_wrong_brand() {
        let mut data = Vec::new();
        let mut ftyp_data = Vec::new();
        ftyp_data.extend_from_slice(b"heic");
        ftyp_data.extend_from_slice(&[0u8; 4]);
        ftyp_data.extend_from_slice(b"heic");
        data.extend_from_slice(&make_box(b"ftyp", &ftyp_data));
        assert!(decode(&data).is_none());
    }

    #[test]
    fn test_avif_empty_dimensions() {
        let avif = build_avif(0, 0);
        assert!(decode(&avif).is_none());
    }

    // Tests below require the `avif` feature
    #[cfg(feature = "avif")]
    #[test]
    fn test_avif_basic() {
        let avif = build_avif(64, 48);
        let img = decode(&avif).expect("should decode basic AVIF");
        assert_eq!(img.width, 64);
        assert_eq!(img.height, 48);
        assert_eq!(img.color, ColorType::Rgba8);
        assert_eq!(img.pixels.len(), 64 * 48 * 4);
    }

    #[cfg(feature = "avif")]
    #[test]
    fn test_avif_avis_brand() {
        let data = build_avis_brand(32, 32);
        let img = decode(&data).expect("should decode AVIS-branded AVIF");
        assert_eq!(img.width, 32);
        assert_eq!(img.height, 32);
    }

    #[cfg(feature = "avif")]
    #[test]
    fn test_avif_no_ispe() {
        // ftyp only, no meta → ispe
        let mut data = Vec::new();
        let mut ftyp_data = Vec::new();
        ftyp_data.extend_from_slice(b"avif");
        ftyp_data.extend_from_slice(&[0u8; 4]);
        ftyp_data.extend_from_slice(b"avif");
        data.extend_from_slice(&make_box(b"ftyp", &ftyp_data));
        assert!(decode(&data).is_none());
    }
}
