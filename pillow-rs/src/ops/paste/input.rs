use super::types::{PasteColorComponent, PasteSource};
use super::color::bad_single_color;
use crate::error::PilError;
use crate::raster::Mode;

pub(super) fn byte_color(value: i64) -> Result<u8, PilError> {
    u8::try_from(value).map_err(|_| PilError::TypeError("im must be Image or color".to_owned()))
}

pub(super) fn clipped_byte(value: i64) -> u8 {
    value.clamp(0, 255) as u8
}

pub(super) fn paste_scalar_source(value: i64, mode: Mode) -> PasteSource {
    if matches!(
        mode,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    ) {
        // Pillow's I;16 getink path writes the low two bytes of the integer
        // directly into its native sample, preserving the mode's byte order
        // at the exact-mode CPU owner.
        PasteSource::RawScalar(value)
    } else {
        match u8::try_from(value) {
            Ok(value) => PasteSource::Scalar(value),
            Err(_) => PasteSource::RawScalar(value),
        }
    }
}

pub(super) fn wide_integer_scalar_source(
    mode: Mode,
    float_value: Option<f64>,
) -> Result<PasteSource, PilError> {
    if mode == Mode::ImageF32 {
        return float_value
            .map(PasteSource::RawFloat)
            .ok_or_else(|| PilError::OverflowError("int too large to convert to float".into()));
    }
    Err(PilError::OverflowError(
        "Python int too large to convert to C long long".into(),
    ))
}

pub(super) fn paste_tuple_source(
    values: &[PasteColorComponent],
    mode: Mode,
) -> Result<PasteSource, PilError> {
    if values.len() == 1 {
        return paste_single_tuple_component(&values[0], mode);
    }
    if mode == Mode::ImageF32 {
        return Err(PilError::TypeError(
            "must be real number, not tuple".to_owned(),
        ));
    }
    if is_luma_alpha_mode(mode) {
        return paste_luma_alpha_tuple(values, mode);
    }
    if is_multi_band_mode(mode) {
        return paste_multi_band_tuple(values, mode);
    }
    Err(invalid_component_error(mode))
}

fn paste_single_tuple_component(
    value: &PasteColorComponent,
    mode: Mode,
) -> Result<PasteSource, PilError> {
    match value {
        PasteColorComponent::Integer(value) => Ok(paste_scalar_source(*value, mode)),
        PasteColorComponent::WideInteger { float_value, .. } => {
            wide_integer_scalar_source(mode, *float_value)
        }
        PasteColorComponent::Float(value) if mode == Mode::ImageF32 => {
            Ok(PasteSource::RawFloat(*value))
        }
        PasteColorComponent::Float(_) | PasteColorComponent::Invalid { .. }
            if is_luma_alpha_mode(mode) =>
        {
            // Pillow unwraps a singleton color tuple before calling
            // PyArg_ParseTuple("L|i"). The parser receives the tuple's item,
            // so CPython raises this SystemError for non-integer items.
            Err(PilError::SystemError(
                "new style getargs format but argument is not a tuple".to_owned(),
            ))
        }
        PasteColorComponent::Float(_) if is_single_band_mode(mode) => Err(bad_single_color()),
        PasteColorComponent::Float(_) => Err(invalid_component_error(mode)),
        PasteColorComponent::Invalid { type_name } if mode == Mode::ImageF32 => {
            Err(real_number_type_error(type_name))
        }
        PasteColorComponent::Invalid { type_name } => {
            Err(non_integer_component_error(type_name, mode))
        }
    }
}

fn paste_luma_alpha_tuple(
    values: &[PasteColorComponent],
    mode: Mode,
) -> Result<PasteSource, PilError> {
    if values.len() != 2 {
        return Err(invalid_component_error(mode));
    }
    let luma = paste_long_long_component(&values[0])?;
    let alpha = paste_c_int_component(&values[1])?;
    match (u8::try_from(luma), u8::try_from(alpha)) {
        (Ok(luma), Ok(alpha)) => Ok(PasteSource::LumaAlpha(luma, alpha)),
        _ => Ok(PasteSource::RawLumaAlpha(luma, alpha)),
    }
}

fn paste_multi_band_tuple(
    values: &[PasteColorComponent],
    mode: Mode,
) -> Result<PasteSource, PilError> {
    if !matches!(values.len(), 3 | 4) {
        return Err(invalid_component_error(mode));
    }
    let red = paste_long_long_component(&values[0])?;
    let green = paste_c_int_component(&values[1])?;
    let blue = paste_c_int_component(&values[2])?;
    if let Some(alpha) = values.get(3) {
        let alpha = paste_c_int_component(alpha)?;
        return match (
            u8::try_from(red),
            u8::try_from(green),
            u8::try_from(blue),
            u8::try_from(alpha),
        ) {
            (Ok(red), Ok(green), Ok(blue), Ok(alpha)) => {
                Ok(PasteSource::Rgba(red, green, blue, alpha))
            }
            _ => Ok(PasteSource::RawRgba(red, green, blue, alpha)),
        };
    }
    match (u8::try_from(red), u8::try_from(green), u8::try_from(blue)) {
        (Ok(red), Ok(green), Ok(blue)) => Ok(PasteSource::Rgb(red, green, blue)),
        _ => Ok(PasteSource::RawRgb(red, green, blue)),
    }
}

fn paste_long_long_component(value: &PasteColorComponent) -> Result<i64, PilError> {
    match value {
        PasteColorComponent::Integer(value) => Ok(*value),
        PasteColorComponent::WideInteger { .. } => Err(PilError::OverflowError(
            "Python int too large to convert to C long long".into(),
        )),
        PasteColorComponent::Float(_) => Err(cannot_convert_to_integer("float")),
        PasteColorComponent::Invalid { type_name } => Err(cannot_convert_to_integer(type_name)),
    }
}

fn paste_c_int_component(value: &PasteColorComponent) -> Result<i64, PilError> {
    let value = match value {
        PasteColorComponent::WideInteger { .. } => {
            return Err(PilError::OverflowError(
                "Python int too large to convert to C long".into(),
            ));
        }
        _ => paste_long_long_component(value)?,
    };
    i32::try_from(value).map(i64::from).map_err(|_| {
        PilError::OverflowError(if value < 0 {
            "signed integer is less than minimum".into()
        } else {
            "signed integer is greater than maximum".into()
        })
    })
}

fn is_single_band_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::One
            | Mode::ImageLuma8
            | Mode::ImagePalette8
            | Mode::ImageI32
            | Mode::ImageI16
            | Mode::ImageI16L
            | Mode::ImageI16B
            | Mode::ImageI16N
    )
}

fn is_luma_alpha_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8
    )
}

fn is_multi_band_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageRgb8
            | Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageCmyk8
            | Mode::ImageYCbCr8
            | Mode::ImageLab8
            | Mode::ImageHsv8
    )
}

pub(super) fn paste_list_source_error(mode: Mode) -> PilError {
    if mode == Mode::ImageF32 {
        return real_number_type_error("list");
    }
    paste_non_tuple_scalar_error(mode)
}

pub(super) fn paste_float_source_error(mode: Mode) -> PilError {
    paste_non_tuple_scalar_error(mode)
}

pub(super) fn paste_invalid_type_error(mode: Mode, type_name: &str) -> PilError {
    if mode == Mode::ImageF32 {
        return real_number_type_error(type_name);
    }
    paste_non_tuple_scalar_error(mode)
}

fn paste_non_tuple_scalar_error(mode: Mode) -> PilError {
    if is_single_band_mode(mode) {
        bad_single_color()
    } else if is_luma_alpha_mode(mode) || is_multi_band_mode(mode) {
        // getink rejects non-integer, non-tuple values before checking tuple
        // arity for byte images with more than one band.
        PilError::TypeError("color must be int or tuple".to_owned())
    } else {
        invalid_component_error(mode)
    }
}

fn non_integer_component_error(type_name: &str, mode: Mode) -> PilError {
    if is_single_band_mode(mode) {
        bad_single_color()
    } else if is_luma_alpha_mode(mode) {
        cannot_convert_to_integer(type_name)
    } else {
        invalid_component_error(mode)
    }
}

fn real_number_type_error(type_name: &str) -> PilError {
    PilError::TypeError(format!("must be real number, not {type_name}"))
}

fn cannot_convert_to_integer(type_name: &str) -> PilError {
    PilError::TypeError(format!("'{type_name}' object cannot be interpreted as an integer"))
}

pub(super) fn paste_source_from_color_string(
    value: &str,
    mode: Mode,
) -> Result<PasteSource, PilError> {
    // Pillow resolves a string through ImageColor.getcolor for the
    // destination mode before entering Paste.c. Preserve that distinction
    // from tuple colors: luma/alpha and RGBA modes carry different arities.
    let (r, g, b, a) = crate::color::parse_color_str_unclamped(value)?;
    // NO_PILLOW_SUPPORT: Rust-only high-depth modes resolve colors through
    // their exact native mode contract without adding Pillow spellings.
    let color = crate::color::getcolor_for_mode(r, g, b, a, mode);
    match color {
        crate::color::ColorValue::Gray(value) => {
            Ok(PasteSource::Scalar(byte_color(i64::from(value))?))
        }
        crate::color::ColorValue::GrayAlpha(value, alpha) => Ok(PasteSource::LumaAlpha(
            byte_color(i64::from(value))?,
            byte_color(i64::from(alpha))?,
        )),
        crate::color::ColorValue::Rgb(r, g, b) | crate::color::ColorValue::Hsv(r, g, b) => {
            Ok(PasteSource::Rgb(
                byte_color(i64::from(r))?,
                byte_color(i64::from(g))?,
                byte_color(i64::from(b))?,
            ))
        }
        crate::color::ColorValue::Rgba(r, g, b, a) => Ok(PasteSource::Rgba(
            byte_color(i64::from(r))?,
            byte_color(i64::from(g))?,
            byte_color(i64::from(b))?,
            byte_color(i64::from(a))?,
        )),
    }
}

pub(super) fn coordinate(value: i64) -> Result<i32, PilError> {
    i32::try_from(value).map_err(|_| {
        if value > i64::from(i32::MAX) {
            PilError::OverflowError("signed integer is greater than maximum".into())
        } else {
            PilError::OverflowError("signed integer is less than minimum".into())
        }
    })
}

pub(super) fn invalid_component_error(mode: Mode) -> PilError {
    let message = match mode {
        Mode::ImageF32 => "must be real number, not tuple",
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImagePalette8
        | Mode::ImageI32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => {
            "color must be int or single-element tuple"
        }
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8 => {
            "color must be int, or tuple of one or two elements"
        }
        _ => "color must be int, or tuple of one, three or four elements",
    };
    PilError::TypeError(message.to_owned())
}
