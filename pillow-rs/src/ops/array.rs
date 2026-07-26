//! Pillow-compatible array-interface descriptor resolution.
//!
//! Host bindings extract `__array_interface__` fields and pass only plain
//! shape/type/mode values here. Dtype inference and dimensional policy belong
//! to core so Python and JavaScript cannot drift.

use crate::error::PilError;

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
