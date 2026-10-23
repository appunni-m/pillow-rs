use super::types::PasteSource;
use crate::error::PilError;

pub(super) fn byte_color(value: i64) -> Result<u8, PilError> {
    u8::try_from(value).map_err(|_| PilError::TypeError("im must be Image or color".to_owned()))
}

pub(super) fn clipped_byte(value: i64) -> u8 {
    value.clamp(0, 255) as u8
}

pub(super) fn paste_scalar_source(value: i64, mode: &str) -> PasteSource {
    if matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N") {
        // Pillow's I;16 Paste.c path consumes the low byte of the Python
        // integer and duplicates it across the unsigned sample. Keeping the
        // source as the existing byte scalar preserves that behavior without
        // involving the pending TIFF decoder lane.
        PasteSource::Scalar(value.to_le_bytes()[0])
    } else {
        match u8::try_from(value) {
            Ok(value) => PasteSource::Scalar(value),
            Err(_) => PasteSource::RawScalar(value),
        }
    }
}

pub(super) fn paste_source_from_color_string(
    value: &str,
    mode: &str,
) -> Result<PasteSource, PilError> {
    // Pillow resolves a string through ImageColor.getcolor for the
    // destination mode before entering Paste.c. Preserve that distinction
    // from tuple colors: luma/alpha and RGBA modes carry different arities.
    let (r, g, b, a) = crate::color::parse_color_str_unclamped(value)?;
    let color = crate::color::getcolor(r, g, b, a, mode)?;
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

pub(super) fn invalid_component_error(mode: &str) -> PilError {
    let message = match mode {
        "F" => "must be real number, not tuple",
        "1" | "L" | "P" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" => {
            "color must be int or single-element tuple"
        }
        "LA" | "PA" => "color must be int, or tuple of one or two elements",
        _ => "color must be int, or tuple of one, three or four elements",
    };
    PilError::TypeError(message.to_owned())
}
