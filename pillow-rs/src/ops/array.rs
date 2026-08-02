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

/// Build an image from the flat integer representation used by Python list
/// inputs to `Image.fromarray`.
///
/// The binding is responsible only for turning Python list/tuple objects into
/// `i32` values. Mode arity, empty/partial rows, range checks, and image
/// construction stay here so every host binding shares one contract.
pub fn from_array_pixel_values(
    values: &[i32],
    explicit_mode: Option<&str>,
) -> Result<Image, PilError> {
    let mode = explicit_mode.unwrap_or("L");
    let bands = match mode {
        "1" | "L" | "I" | "F" | "P" => 1,
        "LA" | "PA" => 2,
        "RGB" | "YCbCr" | "HSV" => 3,
        "RGBA" | "CMYK" => 4,
        _ => return Err(PilError::ValueError("unrecognized image mode".into())),
    };
    if values.is_empty() || values.len() % bands != 0 {
        return Err(PilError::ValueError(
            "fromarray_pixel_list: not enough pixel values for the given mode".into(),
        ));
    }
    let width = values
        .len()
        .checked_div(bands)
        .and_then(|width| u32::try_from(width).ok())
        .filter(|&width| width > 0)
        .ok_or_else(|| {
            PilError::ValueError(
                "fromarray_pixel_list: not enough pixel values for the given mode".into(),
            )
        })?;
    let bytes = crate::ops::utils::flatten_pixel_list(values)?;
    Image::frombytes(mode, (width, 1), &bytes)
}

/// Build an image from a packed byte object supplied to `Image.fromarray`.
///
/// The Python binding only obtains the bytes from the host object. Mode
/// selection, width conversion, and raw-image construction remain in core.
pub fn from_array_bytes(data: &[u8], explicit_mode: Option<&str>) -> Result<Image, PilError> {
    let mode = explicit_mode.unwrap_or("L");
    let width =
        u32::try_from(data.len()).map_err(|_| PilError::ValueError("array is too large".into()))?;
    Image::frombytes(mode, (width, 1), data)
}

/// Build an image from an array-interface descriptor and its packed bytes.
///
/// Shape/type inference and dimensional validation are core behavior; host
/// bindings only obtain the descriptor fields and the buffer bytes.
pub fn from_array_interface(
    shape: &[usize],
    typestr: &str,
    explicit_mode: Option<&str>,
    data: &[u8],
) -> Result<Image, PilError> {
    let layout = resolve_array_layout(shape, typestr, explicit_mode)?;
    let width = u32::try_from(layout.width)
        .map_err(|_| PilError::ValueError("image width is too large".into()))?;
    let height = u32::try_from(layout.height)
        .map_err(|_| PilError::ValueError("image height is too large".into()))?;
    Image::frombytes(&layout.raw_mode, (width, height), data)
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

#[cfg(test)]
mod tests {
    use super::ArrayLayout;
    use super::resolve_array_layout;

    #[test]
    fn resolves_scalar_and_color_type_keys() {
        assert_eq!(
            resolve_array_layout(&[4, 5], "|u1", None).unwrap(),
            ArrayLayout {
                mode: "L".into(),
                raw_mode: "L".into(),
                width: 5,
                height: 4,
                dimensions: 2,
                mode_reinterprets_dtype: false,
            }
        );
        let rgba = resolve_array_layout(&[4, 5, 4], "|u1", None).unwrap();
        assert_eq!(
            (rgba.mode.as_str(), rgba.raw_mode.as_str()),
            ("RGBA", "RGBA")
        );
    }

    #[test]
    fn preserves_exact_descriptor_errors() {
        assert_eq!(
            resolve_array_layout(&[2, 3, 5], "|u1", None)
                .unwrap_err()
                .to_string(),
            "Cannot handle this data type: (1, 1, 5), |u1"
        );
        assert_eq!(
            resolve_array_layout(&[1, 2, 3], "|u1", Some("L"))
                .unwrap_err()
                .to_string(),
            "Too many dimensions: 3 > 2."
        );
    }
}
