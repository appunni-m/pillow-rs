use super::input::clipped_byte;
use super::types::{PasteSource, ResolvedPasteColor};
use crate::error::PilError;

impl PasteSource {
    pub(super) fn solid_color(&self, mode: &str) -> Result<ResolvedPasteColor, PilError> {
        match mode {
            "1" | "L" | "P" => single_band_color(self),
            "LA" | "PA" => luma_alpha_color(self),
            "RGB" | "RGBA" | "RGBa" | "RGBX" | "CMYK" | "YCbCr" | "HSV" => {
                multi_band_color(mode, self)
            }
            "I" | "F" => numeric_color(mode, self),
            "I;16" | "I;16L" | "I;16B" | "I;16N" => luma16_color(self),
            _ => Err(PilError::ValueError(format!(
                "unsupported paste destination mode {mode}"
            ))),
        }
    }
}

fn single_band_color(source: &PasteSource) -> Result<ResolvedPasteColor, PilError> {
    match source {
        PasteSource::Scalar(value) => Ok(ResolvedPasteColor::Bytes((*value, *value, *value, 255))),
        PasteSource::RawScalar(value) => {
            let value = clipped_byte(*value);
            Ok(ResolvedPasteColor::Bytes((value, value, value, 255)))
        }
        _ => Err(bad_single_color()),
    }
}

fn luma_alpha_color(source: &PasteSource) -> Result<ResolvedPasteColor, PilError> {
    match source {
        PasteSource::Scalar(value) => Ok(ResolvedPasteColor::Bytes((*value, *value, *value, 0))),
        PasteSource::RawScalar(value) => {
            let bytes = value.to_le_bytes();
            Ok(ResolvedPasteColor::Bytes((
                bytes[0], bytes[0], bytes[0], bytes[3],
            )))
        }
        PasteSource::LumaAlpha(luma, alpha) => {
            Ok(ResolvedPasteColor::Bytes((*luma, *luma, *luma, *alpha)))
        }
        PasteSource::RawLumaAlpha(luma, alpha) => Ok(ResolvedPasteColor::Bytes((
            clipped_byte(*luma),
            clipped_byte(*luma),
            clipped_byte(*luma),
            clipped_byte(*alpha),
        ))),
        _ => Err(bad_luma_alpha_color()),
    }
}

fn multi_band_color(mode: &str, source: &PasteSource) -> Result<ResolvedPasteColor, PilError> {
    let preserves_alpha = matches!(mode, "RGBA" | "RGBa" | "RGBX" | "CMYK");
    let alpha = |value| if preserves_alpha { value } else { 255 };
    let color = match source {
        PasteSource::Scalar(value) => (*value, 0, 0, 0),
        PasteSource::RawScalar(value) => {
            let bytes = value.to_le_bytes();
            (bytes[0], bytes[1], bytes[2], alpha(bytes[3]))
        }
        PasteSource::Rgb(r, g, b) => (*r, *g, *b, 255),
        PasteSource::RawRgb(r, g, b) => (clipped_byte(*r), clipped_byte(*g), clipped_byte(*b), 255),
        PasteSource::Rgba(r, g, b, a) => (*r, *g, *b, alpha(*a)),
        PasteSource::RawRgba(r, g, b, a) => (
            clipped_byte(*r),
            clipped_byte(*g),
            clipped_byte(*b),
            alpha(clipped_byte(*a)),
        ),
        _ => return Err(bad_multi_band_color()),
    };
    Ok(ResolvedPasteColor::Bytes(color))
}

fn numeric_color(mode: &str, source: &PasteSource) -> Result<ResolvedPasteColor, PilError> {
    match (mode, source) {
        ("I", PasteSource::Scalar(value)) => Ok(ResolvedPasteColor::Integer(i32::from(*value))),
        ("I", PasteSource::RawScalar(value)) => Ok(ResolvedPasteColor::Integer(
            i32::try_from(*value)
                .map_err(|_| PilError::TypeError("im must be Image or color".to_owned()))?,
        )),
        ("F", PasteSource::Scalar(value)) => Ok(ResolvedPasteColor::Float(f32::from(*value))),
        ("F", PasteSource::RawScalar(value)) => Ok(ResolvedPasteColor::Float(*value as f32)),
        ("F", PasteSource::RawFloat(value)) => Ok(ResolvedPasteColor::Float(*value as f32)),
        ("I", _) => Err(bad_single_color()),
        // Pillow rejects tuple colors for F with its native real-number diagnostic.
        ("F", _) => Err(PilError::TypeError(
            "must be real number, not tuple".to_owned(),
        )),
        _ => unreachable!("numeric modes are selected by solid_color"),
    }
}

fn luma16_color(source: &PasteSource) -> Result<ResolvedPasteColor, PilError> {
    match source {
        PasteSource::Scalar(value) => Ok(ResolvedPasteColor::Luma16(u16::from(*value) * 0x0101)),
        _ => Err(bad_single_color()),
    }
}

fn bad_single_color() -> PilError {
    PilError::TypeError("color must be int or single-element tuple".to_owned())
}

fn bad_luma_alpha_color() -> PilError {
    PilError::TypeError("color must be int, or tuple of one or two elements".to_owned())
}

fn bad_multi_band_color() -> PilError {
    PilError::TypeError("color must be int, or tuple of one, three or four elements".to_owned())
}
