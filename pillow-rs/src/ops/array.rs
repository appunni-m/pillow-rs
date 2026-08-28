//! Pillow-compatible array-interface descriptor resolution.
//!
//! Host bindings extract `__array_interface__` fields and pass only plain
//! shape/type/mode values here. Dtype inference and dimensional policy belong
//! to core so Python and JavaScript cannot drift.

use crate::error::PilError;
use crate::image::Image;

/// Resolved Pillow image and raw-decoder layout for an array descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrayLayout {
    /// Public Pillow image mode.
    pub mode: String,
    /// Pillow raw decoder mode.
    pub raw_mode: String,
    /// Image width.
    pub width: usize,
    /// Image height.
    pub height: usize,
    /// Number of array dimensions.
    pub dimensions: usize,
    /// Whether explicit mode changes the inferred dtype interpretation.
    pub mode_reinterprets_dtype: bool,
}

struct TypeMapEntry {
    shape_tail: &'static [usize],
    typestr: &'static str,
    mode: &'static str,
    raw_mode: &'static str,
    color_modes: &'static [&'static str],
}

const TYPE_MAP: &[TypeMapEntry] = &[
    TypeMapEntry {
        shape_tail: &[],
        typestr: "|b1",
        mode: "1",
        raw_mode: "1;8",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: "|u1",
        mode: "L",
        raw_mode: "L",
        color_modes: &["P"],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: "|i1",
        mode: "I",
        raw_mode: "I;8",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: "<u2",
        mode: "I",
        raw_mode: "I;16",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: ">u2",
        mode: "I",
        raw_mode: "I;16B",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: "<i2",
        mode: "I",
        raw_mode: "I;16S",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: ">i2",
        mode: "I",
        raw_mode: "I;16BS",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: "<u4",
        mode: "I",
        raw_mode: "I;32",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: ">u4",
        mode: "I",
        raw_mode: "I;32B",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: "<i4",
        mode: "I",
        raw_mode: "I",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: ">i4",
        mode: "I",
        raw_mode: "I;32BS",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: "<f4",
        mode: "F",
        raw_mode: "F",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: ">f4",
        mode: "F",
        raw_mode: "F;32BF",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: "<f8",
        mode: "F",
        raw_mode: "F;64F",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[],
        typestr: ">f8",
        mode: "F",
        raw_mode: "F;64BF",
        color_modes: &[],
    },
    TypeMapEntry {
        shape_tail: &[2],
        typestr: "|u1",
        mode: "LA",
        raw_mode: "LA",
        color_modes: &["La", "PA"],
    },
    TypeMapEntry {
        shape_tail: &[3],
        typestr: "|u1",
        mode: "RGB",
        raw_mode: "RGB",
        color_modes: &["YCbCr", "LAB", "HSV"],
    },
    TypeMapEntry {
        shape_tail: &[4],
        typestr: "|u1",
        mode: "RGBA",
        raw_mode: "RGBA",
        color_modes: &["RGBa", "RGBX", "CMYK"],
    },
];

/// Resolve Pillow's array-interface type key, mode, raw mode, size, and
/// dimensional limit.
///
/// # Errors
///
/// Returns Pillow-compatible [`PilError::TypeError`] for unsupported type keys
/// and [`PilError::ValueError`] for excessive dimensions.
pub fn resolve_array_layout(
    shape: &[usize],
    typestr: &str,
    explicit_mode: Option<&str>,
) -> Result<ArrayLayout, PilError> {
    let shape_tail = shape.get(2..).unwrap_or_default();
    let Some(entry) = TYPE_MAP
        .iter()
        .find(|entry| entry.shape_tail == shape_tail && entry.typestr == typestr)
    else {
        return Err(PilError::TypeError(format!(
            "Cannot handle this data type: {}, {typestr}",
            format_typekey_shape(shape_tail)
        )));
    };

    let mode = explicit_mode.unwrap_or(entry.mode);
    let dimensions = shape.len();
    let maximum_dimensions = match mode {
        "1" | "L" | "I" | "P" | "F" => 2,
        "RGB" => 3,
        _ => 4,
    };
    if dimensions > maximum_dimensions {
        return Err(PilError::ValueError(format!(
            "Too many dimensions: {dimensions} > {maximum_dimensions}."
        )));
    }

    let (width, height) = match shape {
        [length] => (1, *length),
        [height, width, ..] => (*width, *height),
        [] => {
            return Err(PilError::IndexError("tuple index out of range".to_string()));
        }
    };
    let mode_reinterprets_dtype = mode != entry.mode && !entry.color_modes.contains(&mode);
    Ok(ArrayLayout {
        mode: mode.to_string(),
        raw_mode: explicit_mode.unwrap_or(entry.raw_mode).to_string(),
        width,
        height,
        dimensions,
        mode_reinterprets_dtype,
    })
}

/// Build an image from a resolved array-interface layout and its packed bytes.
///
/// The caller resolves the descriptor before obtaining host buffer bytes so
/// Pillow's validation-before-buffer ordering is preserved. Conversion and
/// raw-image construction remain in the Rust core.
pub fn from_resolved_array_interface(layout: &ArrayLayout, data: &[u8]) -> Result<Image, PilError> {
    let width = u32::try_from(layout.width)
        // Pillow's frombuffer dimension conversion raises this exact
        // OverflowError before attempting allocation.
        .map_err(|_| PilError::OverflowError("signed integer is greater than maximum".into()))?;
    let height = u32::try_from(layout.height)
        .map_err(|_| PilError::OverflowError("signed integer is greater than maximum".into()))?;

    if layout.mode == "1" && layout.raw_mode == "1;8" {
        // Pillow's ``|b1`` array typemap is a byte-per-pixel source decoder,
        // not the packed bitmap layout accepted by Image.frombytes("1").
        // Pack each nonzero source byte into the public mode-1 scanline before
        // entering the ordinary core decoder.
        let samples = layout
            .width
            .checked_mul(layout.height)
            .ok_or_else(|| PilError::OverflowError("array dimensions overflow".into()))?;
        let input = data
            .get(..samples)
            .ok_or_else(|| PilError::ValueError("not enough image data".into()))?;
        let row_bytes = layout.width.div_ceil(8);
        let output_len = row_bytes
            .checked_mul(layout.height)
            .ok_or_else(|| PilError::OverflowError("array buffer size overflow".into()))?;
        let mut packed = vec![0u8; output_len];
        for (index, &sample) in input.iter().enumerate() {
            if sample != 0 {
                let row = index / layout.width;
                let column = index % layout.width;
                packed[row * row_bytes + column / 8] |= 0x80 >> (column % 8);
            }
        }
        return Image::frombytes("1", (width, height), &packed);
    }

    if let Some(normalized) = normalize_scalar_array(layout, data)? {
        // Pillow's array typemap exposes several raw scalar encodings while
        // the resulting public image remains mode I or F.  Normalize those
        // encodings to the native four-byte representation before entering
        // the ordinary Image::frombytes mode path.
        return Image::frombytes(&layout.mode, (width, height), &normalized);
    }
    Image::frombytes(&layout.raw_mode, (width, height), data)
}

fn normalize_scalar_array(layout: &ArrayLayout, data: &[u8]) -> Result<Option<Vec<u8>>, PilError> {
    let samples = layout
        .width
        .checked_mul(layout.height)
        .ok_or_else(|| PilError::OverflowError("array dimensions overflow".into()))?;

    let bytes_per_sample = match layout.raw_mode.as_str() {
        "I;8" => Some(1),
        "I;16S" | "I;16BS" => Some(2),
        "I;32" | "I;32B" | "I;32BS" | "F;32BF" => Some(4),
        "F;64F" | "F;64BF" => Some(8),
        _ => None,
    };
    let Some(bytes_per_sample) = bytes_per_sample else {
        return Ok(None);
    };
    let required = samples
        .checked_mul(bytes_per_sample)
        .ok_or_else(|| PilError::OverflowError("array buffer size overflow".into()))?;
    let input = data
        .get(..required)
        .ok_or_else(|| PilError::ValueError("not enough image data".into()))?;
    let output_capacity = samples
        .checked_mul(4)
        .ok_or_else(|| PilError::OverflowError("array buffer size overflow".into()))?;
    let mut output = Vec::with_capacity(output_capacity);

    if layout.mode == "I" {
        for sample in input.chunks_exact(bytes_per_sample) {
            let value = match layout.raw_mode.as_str() {
                // Pillow's I;8 raw decoder widens the byte as unsigned even
                // when the NumPy descriptor is signed |i1.
                "I;8" => i32::from(sample[0]),
                "I;16S" => i32::from(i16::from_le_bytes([sample[0], sample[1]])),
                "I;16BS" => i32::from(i16::from_be_bytes([sample[0], sample[1]])),
                "I;32" => u32::from_le_bytes(sample.try_into().unwrap()) as i32,
                "I;32B" => u32::from_be_bytes(sample.try_into().unwrap()) as i32,
                "I;32BS" => i32::from_be_bytes(sample.try_into().unwrap()),
                _ => return Ok(None),
            };
            output.extend_from_slice(&value.to_ne_bytes());
        }
    } else if layout.mode == "F" {
        for sample in input.chunks_exact(bytes_per_sample) {
            // Pillow stores F images as float32, so f64 array inputs must
            // intentionally narrow to the public image representation.
            #[allow(clippy::cast_possible_truncation)]
            let value = match layout.raw_mode.as_str() {
                "F;32BF" => f32::from_be_bytes(sample.try_into().unwrap()),
                "F;64F" => f64::from_le_bytes(sample.try_into().unwrap()) as f32,
                "F;64BF" => f64::from_be_bytes(sample.try_into().unwrap()) as f32,
                _ => return Ok(None),
            };
            output.extend_from_slice(&value.to_ne_bytes());
        }
    } else {
        return Ok(None);
    }

    Ok(Some(output))
}

fn format_typekey_shape(shape_tail: &[usize]) -> String {
    let mut values = vec![1, 1];
    values.extend_from_slice(shape_tail);
    format!(
        "({})",
        values
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}
