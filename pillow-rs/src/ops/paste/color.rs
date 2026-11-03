use super::input::clipped_byte;
use super::types::{PasteSource, ResolvedPasteColor};
use crate::error::PilError;
use crate::raster::Mode;

impl PasteSource {
    pub(super) fn solid_color(&self, mode: Mode) -> Result<ResolvedPasteColor, PilError> {
        match mode {
            Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => single_band_color(self),
            Mode::ImageLumaA8
            | Mode::ImageLaPremultiplied8
            | Mode::ImagePaletteAlpha8 => luma_alpha_color(self),
            Mode::ImageRgb8
            | Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageCmyk8
            | Mode::ImageYCbCr8
            | Mode::ImageLab8
            | Mode::ImageHsv8 => {
                multi_band_color(mode, self)
            }
            Mode::ImageI32 | Mode::ImageF32 => numeric_color(mode, self),
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                luma16_color(self)
            }
            _ => Err(PilError::ValueError(format!(
                "unsupported paste destination mode {}",
                mode.pillow_name().unwrap_or("internal raster")
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

fn multi_band_color(mode: Mode, source: &PasteSource) -> Result<ResolvedPasteColor, PilError> {
    let preserves_alpha = matches!(
        mode,
        Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageCmyk8
    );
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

fn numeric_color(mode: Mode, source: &PasteSource) -> Result<ResolvedPasteColor, PilError> {
    match (mode, source) {
        (Mode::ImageI32, PasteSource::Scalar(value)) => {
            Ok(ResolvedPasteColor::Integer(i32::from(*value)))
        }
        (Mode::ImageI32, PasteSource::RawScalar(value)) => {
            // Pillow's INT32 ink stores the low four bytes after parsing a
            // signed long long, matching native two's-complement narrowing.
            let bytes = value.to_le_bytes();
            Ok(ResolvedPasteColor::Integer(i32::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
            ])))
        }
        (Mode::ImageF32, PasteSource::Scalar(value)) => {
            Ok(ResolvedPasteColor::Float(f32::from(*value)))
        }
        (Mode::ImageF32, PasteSource::RawScalar(value)) => {
            Ok(ResolvedPasteColor::Float(*value as f32))
        }
        (Mode::ImageF32, PasteSource::RawFloat(value)) => {
            Ok(ResolvedPasteColor::Float(*value as f32))
        }
        (Mode::ImageI32, _) => Err(bad_single_color()),
        // Pillow rejects tuple colors for F with its native real-number diagnostic.
        (Mode::ImageF32, _) => Err(PilError::TypeError(
            "must be real number, not tuple".to_owned(),
        )),
        _ => unreachable!("numeric modes are selected by solid_color"),
    }
}

fn luma16_color(source: &PasteSource) -> Result<ResolvedPasteColor, PilError> {
    match source {
        PasteSource::Scalar(value) => Ok(ResolvedPasteColor::Luma16(u16::from(*value))),
        PasteSource::RawScalar(value) => {
            let bytes = value.to_le_bytes();
            Ok(ResolvedPasteColor::Luma16(u16::from_le_bytes([
                bytes[0], bytes[1],
            ])))
        }
        _ => Err(bad_single_color()),
    }
}

pub(super) fn bad_single_color() -> PilError {
    PilError::TypeError("color must be int or single-element tuple".to_owned())
}

fn bad_luma_alpha_color() -> PilError {
    PilError::TypeError("color must be int, or tuple of one or two elements".to_owned())
}

fn bad_multi_band_color() -> PilError {
    PilError::TypeError("color must be int, or tuple of one, three or four elements".to_owned())
}
