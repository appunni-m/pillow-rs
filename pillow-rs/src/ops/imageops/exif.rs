use super::*;

/// Applies the EXIF orientation transform used by `ImageOps.exif_transpose`.
///
/// The returned image is `Some` for a transformed image, or for a copy when
/// `in_place` is false and no valid orientation is present. In-place
/// replacement is performed by the binding after this function returns.
pub fn exif_transpose(image: &Image, in_place: bool) -> Result<Option<Image>, PilError> {
    let raw_exif = image.getexif();
    // Pillow maps EXIF's eight orientation values onto these named transpose
    // operations; the rotation directions are intentionally asymmetric.
    let method = match exif_get_orientation(&raw_exif).unwrap_or(1) {
        2 => Some("FLIP_LEFT_RIGHT"),
        3 => Some("ROTATE_180"),
        4 => Some("FLIP_TOP_BOTTOM"),
        5 => Some("TRANSPOSE"),
        6 => Some("ROTATE_270"),
        7 => Some("TRANSVERSE"),
        8 => Some("ROTATE_90"),
        _ => None,
    };

    match method {
        Some(method) => {
            let exif = exif_remove_orientation(&raw_exif);
            image
                .transpose(method)?
                .with_exif_metadata(Some(exif))
                .map(Some)
        }
        None if in_place => Ok(None),
        None => Ok(Some(image.copy())),
    }
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
    let data = if raw.starts_with(b"Exif\x00\x00") {
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

/// Remove IFD0's Orientation entry after the pixels have been transposed.
///
/// Pillow's `ImageOps.exif_transpose` serializes the retained Exif mapping
/// again, which removes tag `0x0112` while retaining unrelated IFD0 entries.
/// The fixture surface currently uses inline SHORT values; preserve the
/// remainder of the TIFF payload byte-for-byte so opaque metadata remains
/// untouched.
pub fn exif_remove_orientation(raw: &[u8]) -> Vec<u8> {
    let Some(ifd) = parse_exif_ifd(raw) else {
        return raw.to_vec();
    };
    let Some(entries) = ifd.data.get(ifd.entries_start..ifd.entries_end) else {
        // Pillow rebuilds a valid Exif mapping when an incomplete advertised
        // entry follows complete IFD records. Preserve complete records and
        // serialize the next-IFD pointer as zero.
        let available_entries = ifd.data.len().saturating_sub(ifd.entries_start) / 12;
        let partial_end = ifd
            .entries_start
            .saturating_add(available_entries.saturating_mul(12));
        let Some(partial_entries) = ifd.data.get(ifd.entries_start..partial_end) else {
            return raw.to_vec();
        };
        let retained = retain_exif_entries(partial_entries, ifd.byte_order);
        return serialize_exif_ifd(ifd, &retained, &[0; 4], false);
    };

    let retained = retain_exif_entries(entries, ifd.byte_order);
    if retained.len() == ifd.entry_count {
        return raw.to_vec();
    }
    serialize_exif_ifd(ifd, &retained, &ifd.data[ifd.entries_end..], true)
}

#[derive(Clone, Copy)]
struct ExifIfd<'a> {
    raw: &'a [u8],
    data: &'a [u8],
    prefix_len: usize,
    byte_order: TiffByteOrder,
    ifd_offset: usize,
    entry_count: usize,
    entries_start: usize,
    entries_end: usize,
}

#[derive(Clone, Copy)]
enum TiffByteOrder {
    Little,
    Big,
}

impl TiffByteOrder {
    fn read_u16(self, bytes: &[u8]) -> u16 {
        let bytes = [bytes[0], bytes[1]];
        match self {
            Self::Little => u16::from_le_bytes(bytes),
            Self::Big => u16::from_be_bytes(bytes),
        }
    }

    fn read_u32(self, bytes: &[u8]) -> u32 {
        let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        }
    }

    fn push_u16(self, output: &mut Vec<u8>, value: u16) {
        let bytes = match self {
            Self::Little => value.to_le_bytes(),
            Self::Big => value.to_be_bytes(),
        };
        output.extend_from_slice(&bytes);
    }
}

fn parse_exif_ifd(raw: &[u8]) -> Option<ExifIfd<'_>> {
    let prefix_len = if raw.starts_with(b"Exif\x00\x00") {
        6
    } else {
        0
    };
    let data = raw.get(prefix_len..)?;
    if data.len() < 8 {
        return None;
    }
    let byte_order = match &data[..2] {
        b"II" => TiffByteOrder::Little,
        b"MM" => TiffByteOrder::Big,
        _ => return None,
    };
    if byte_order.read_u16(&data[2..4]) != 42 {
        return None;
    }
    let ifd_offset = usize::try_from(byte_order.read_u32(&data[4..8])).ok()?;
    let entries_start = ifd_offset.checked_add(2)?;
    let count_bytes = data.get(ifd_offset..entries_start)?;
    let entry_count = usize::from(byte_order.read_u16(count_bytes));
    let entries_end = entries_start.checked_add(entry_count.checked_mul(12)?)?;
    Some(ExifIfd {
        raw,
        data,
        prefix_len,
        byte_order,
        ifd_offset,
        entry_count,
        entries_start,
        entries_end,
    })
}

fn retain_exif_entries(entries: &[u8], byte_order: TiffByteOrder) -> Vec<Vec<u8>> {
    entries
        .chunks_exact(12)
        .filter(|entry| byte_order.read_u16(&entry[..2]) != 0x0112)
        .map(|entry| normalize_image_width_entry(entry, byte_order))
        .collect()
}

fn normalize_image_width_entry(entry: &[u8], byte_order: TiffByteOrder) -> Vec<u8> {
    let mut entry = entry.to_vec();
    let is_inline_short_width = byte_order.read_u16(&entry[..2]) == 0x0100
        && byte_order.read_u16(&entry[2..4]) == 3
        && byte_order.read_u32(&entry[4..8]) == 1;
    if is_inline_short_width {
        let value = u32::from(byte_order.read_u16(&entry[8..10]));
        entry[2..4].copy_from_slice(&match byte_order {
            TiffByteOrder::Little => 4u16.to_le_bytes(),
            TiffByteOrder::Big => 4u16.to_be_bytes(),
        });
        let value_bytes = match byte_order {
            TiffByteOrder::Little => value.to_le_bytes(),
            TiffByteOrder::Big => value.to_be_bytes(),
        };
        entry[8..12].copy_from_slice(&value_bytes);
    }
    entry
}

fn serialize_exif_ifd(
    ifd: ExifIfd<'_>,
    retained: &[Vec<u8>],
    tail: &[u8],
    pad_short_tail: bool,
) -> Vec<u8> {
    let capacity = if pad_short_tail {
        ifd.raw.len().saturating_sub(12)
    } else {
        ifd.prefix_len
            .saturating_add(ifd.ifd_offset)
            .saturating_add(2)
            .saturating_add(retained.len().saturating_mul(12))
            .saturating_add(4)
    };
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(&ifd.raw[..ifd.prefix_len]);
    output.extend_from_slice(&ifd.data[..ifd.ifd_offset]);
    ifd.byte_order.push_u16(&mut output, retained.len() as u16);
    for entry in retained {
        output.extend_from_slice(entry);
    }
    output.extend_from_slice(tail);
    if pad_short_tail && tail.len() < 4 {
        output.extend(std::iter::repeat_n(0, 4 - tail.len()));
    }
    output
}
