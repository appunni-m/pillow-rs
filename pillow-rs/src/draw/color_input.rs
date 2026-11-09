//! Mode-aware resolution for Python integers passed to ImageDraw.

use super::{Draw, DrawColorInput, Mode, PaletteColorComponent};
use crate::error::PilError;
use crate::image::PutPixelValue;

fn draw_color_error() -> PilError {
    PilError::TypeError(
        // Pillow's ImageDraw._getink() reports this exact message for a
        // component sequence with an unsupported arity.
        "color must be int, or tuple of one, three or four elements".to_owned(),
    )
}

pub(super) fn draw_single_band_color_error() -> PilError {
    PilError::TypeError("color must be int or single-element tuple".to_owned())
}

fn draw_python_list_color_error(mode: Mode) -> PilError {
    let message = if mode == Mode::ImageF32 {
        "must be real number, not list"
    } else if mode.band_count() == 1 {
        "color must be int or single-element tuple"
    } else {
        "color must be int or tuple"
    };
    PilError::TypeError(message.to_owned())
}

fn is_l16_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    )
}

pub(super) fn is_single_band_draw_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 | Mode::ImageI32 | Mode::ImageF32
    ) || is_l16_mode(mode)
}

pub(super) fn is_double_band_draw_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8
    )
}

fn draw_float_color_error(mode: Mode) -> PilError {
    if matches!(
        mode,
        Mode::One
            | Mode::ImageLuma8
            | Mode::ImagePalette8
            | Mode::ImageI32
            | Mode::ImageI16
            | Mode::ImageI16L
            | Mode::ImageI16B
            | Mode::ImageI16N
    ) {
        PilError::TypeError("color must be int or single-element tuple".to_owned())
    } else {
        PilError::TypeError("color must be int or tuple".to_owned())
    }
}

fn draw_byte(value: i64) -> u8 {
    // Pillow clamps tuple components to the byte range before handing them to
    // the ImagingDraw backend; integer inks use a separate packed path below.
    value.clamp(0, 255) as u8
}

fn draw_component_byte(mode: Mode, channel: usize, value: i64) -> Result<u8, PilError> {
    let value = draw_c_int_byte(value)?;
    if mode == Mode::ImageLab8 && matches!(channel, 1 | 2) {
        Ok(value.wrapping_add(128))
    } else {
        Ok(value)
    }
}

fn resolve_integer_color(mode: Mode, value: i64) -> Result<(u8, u8, u8, u8), PilError> {
    if mode == Mode::ImageF32 {
        // Pillow first converts Python integers to double in PyFloat_AsDouble.
        let bytes = (value as f64 as f32).to_le_bytes();
        return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
    }
    if mode == Mode::ImageI32 {
        // Pillow reads a signed long long, then stores its low 32 bits in I.
        let bytes = (value as i32).to_le_bytes();
        return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
    }
    if is_l16_mode(mode) {
        // Pillow's I;16* draw path packs an integer as a native 32-bit ink
        // and stores its low two bytes. Keep the packed little-endian bytes
        // here; the native canvas applies the declared I;16B byte-order tag
        // when turning them into its host-native u16 sample.
        let bytes = (value as i32 as u16).to_le_bytes();
        return Ok((bytes[0], bytes[1], 0, 255));
    }
    if matches!(
        mode,
        Mode::ImagePaletteAlpha8 | Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8
    ) {
        // Pillow packs integer multi-band inks into all four bytes. The native
        // PA/LA/La owners consume the first byte as their sample and byte 3 as
        // alpha, matching Pillow's two-band getink representation.
        let bytes = value.to_le_bytes();
        return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
    }
    // Pillow's multi-band UINT8 ink path extracts the low four bytes from its
    // signed long-long value (ABGR compatibility packing).
    let packed = value.to_le_bytes();
    let value = draw_byte(value);
    Ok(match mode {
        // Multi-band integer inks are packed little-endian by Pillow's
        // ImagingDraw implementation, including values outside one byte.
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageHsv8 | Mode::ImageLab8 => {
            (packed[0], packed[1], packed[2], 255)
        }
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8 | Mode::ImageCmyk8 => {
            (packed[0], packed[1], packed[2], packed[3])
        }
        _ => (value, value, value, 255),
    })
}

fn resolve_component_color(mode: Mode, values: &[i64]) -> Result<(u8, u8, u8, u8), PilError> {
    if is_l16_mode(mode) {
        if values.len() != 1 {
            return Err(PilError::TypeError(
                "color must be int or single-element tuple".to_owned(),
            ));
        }
        let bytes = (values[0] as i32 as u16).to_le_bytes();
        return Ok((bytes[0], bytes[1], 0, 255));
    }
    // Pillow unwraps singleton tuples before resolving ink, so tuple(value,)
    // follows the scalar packed path for every mode.
    if values.len() == 1 {
        return resolve_integer_color(mode, values[0]);
    }
    if is_double_band_draw_mode(mode) {
        return match values {
            [value, alpha] => {
                let alpha = draw_c_int_byte(*alpha)?;
                let value = draw_byte(*value);
                Ok((value, value, value, alpha))
            }
            _ => Err(PilError::TypeError(
                "color must be int, or tuple of one or two elements".to_owned(),
            )),
        };
    }
    if is_single_band_draw_mode(mode) {
        let message = if mode == Mode::ImageF32 {
            "must be real number, not tuple"
        } else {
            "color must be int or single-element tuple"
        };
        return Err(PilError::TypeError(message.to_owned()));
    }
    match values {
        [r, g, b] => Ok((
            draw_byte(*r),
            draw_component_byte(mode, 1, *g)?,
            draw_component_byte(mode, 2, *b)?,
            255,
        )),
        [r, g, b, a] => Ok((
            draw_byte(*r),
            draw_component_byte(mode, 1, *g)?,
            draw_component_byte(mode, 2, *b)?,
            draw_component_byte(mode, 3, *a)?,
        )),
        _ => Err(draw_color_error()),
    }
}

fn integer_overflow_long_long() -> PilError {
    // Pillow surfaces CPython's PyLong_AsLongLong message for both signs.
    PilError::OverflowError("int too big to convert".to_owned())
}

fn integer_overflow_c_int(negative: bool) -> PilError {
    // Tuple channels follow CPython's signed C-int conversion messages.
    let message = if negative {
        "signed integer is less than minimum"
    } else {
        "signed integer is greater than maximum"
    };
    PilError::OverflowError(message.to_owned())
}

fn integer_overflow_float() -> PilError {
    PilError::OverflowError("int too large to convert to float".to_owned())
}

fn wide_integer_bit(magnitude: &[u8], position: usize) -> bool {
    let byte_from_end = position / 8;
    let byte_index = magnitude.len() - byte_from_end - 1;
    magnitude[byte_index] & (1 << (position % 8)) != 0
}

fn wide_integer_has_bits_below(magnitude: &[u8], exclusive_bit: usize) -> bool {
    let full_bytes = exclusive_bit / 8;
    let remainder = exclusive_bit % 8;
    if full_bytes > 0
        && magnitude[magnitude.len() - full_bytes..]
            .iter()
            .any(|byte| *byte != 0)
    {
        return true;
    }
    remainder > 0 && magnitude[magnitude.len() - full_bytes - 1] & ((1 << remainder) - 1) != 0
}

/// Converts an arbitrary-size integer with Python's nearest-even int-to-f64
/// rounding before the later f32 cast used by Pillow's F drawing mode.
fn wide_integer_as_f64(negative: bool, magnitude: &[u8]) -> Option<f64> {
    let Some(first) = magnitude.iter().position(|byte| *byte != 0) else {
        return Some(0.0);
    };
    let magnitude = &magnitude[first..];
    let leading_bits = 8 - magnitude[0].leading_zeros() as usize;
    let bit_length = (magnitude.len() - 1)
        .checked_mul(8)?
        .checked_add(leading_bits)?;
    if bit_length > 1024 {
        return None;
    }
    let discarded = bit_length.saturating_sub(53);
    let mut significand = 0_u64;
    for position in (discarded..bit_length).rev() {
        significand = (significand << 1)
            | if wide_integer_bit(magnitude, position) {
                1
            } else {
                0
            };
    }
    if discarded > 0
        && wide_integer_bit(magnitude, discarded - 1)
        && (wide_integer_has_bits_below(magnitude, discarded - 1) || significand & 1 == 1)
    {
        significand += 1;
    }
    let mut exponent = discarded as i32;
    if significand == (1_u64 << 53) {
        significand >>= 1;
        exponent += 1;
    }
    let value = (significand as f64) * 2.0_f64.powi(exponent);
    value
        .is_finite()
        .then_some(if negative { -value } else { value })
}

fn resolve_wide_integer_color(
    mode: Mode,
    value: &PaletteColorComponent,
) -> Result<(u8, u8, u8, u8), PilError> {
    let PaletteColorComponent::WideInteger {
        negative,
        magnitude,
    } = value
    else {
        return Err(integer_overflow_long_long());
    };
    if mode != Mode::ImageF32 {
        return Err(integer_overflow_long_long());
    }
    let value = wide_integer_as_f64(*negative, magnitude).ok_or_else(integer_overflow_float)?;
    let bytes = (value as f32).to_le_bytes();
    Ok((bytes[0], bytes[1], bytes[2], bytes[3]))
}

fn draw_c_int_byte(value: i64) -> Result<u8, PilError> {
    let value = i32::try_from(value).map_err(|_| integer_overflow_c_int(value < 0))?;
    Ok(draw_byte(i64::from(value)))
}

fn resolve_wide_integer_components(
    mode: Mode,
    values: &[PaletteColorComponent],
) -> Result<(u8, u8, u8, u8), PilError> {
    if mode == Mode::ImageF32 {
        if values.len() != 1 {
            return Err(PilError::TypeError(
                "must be real number, not tuple".to_owned(),
            ));
        }
        return resolve_wide_integer_color(mode, &values[0]);
    }
    if values.len() == 1 {
        return match &values[0] {
            PaletteColorComponent::Integer(value) => resolve_integer_color(mode, *value),
            PaletteColorComponent::WideInteger { .. } => Err(integer_overflow_long_long()),
            PaletteColorComponent::Float(_) => Err(draw_color_error()),
        };
    }

    let expected_arity = if is_double_band_draw_mode(mode) {
        values.len() == 2
    } else if is_single_band_draw_mode(mode) {
        return Err(draw_single_band_color_error());
    } else {
        matches!(values.len(), 3 | 4)
    };
    if !expected_arity {
        return Err(if is_double_band_draw_mode(mode) {
            PilError::TypeError("color must be int, or tuple of one or two elements".to_owned())
        } else {
            draw_color_error()
        });
    }

    let mut integer_values = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        match value {
            PaletteColorComponent::Integer(value) => {
                if index > 0 && i32::try_from(*value).is_err() {
                    return Err(integer_overflow_c_int(*value < 0));
                }
                integer_values.push(*value);
            }
            PaletteColorComponent::WideInteger { .. } if index == 0 => {
                return Err(integer_overflow_long_long());
            }
            PaletteColorComponent::WideInteger { negative, .. } => {
                return Err(integer_overflow_c_int(*negative));
            }
            PaletteColorComponent::Float(_) => return Err(draw_color_error()),
        }
    }
    resolve_component_color(mode, &integer_values)
}

impl Draw {
    /// Resolves a host-neutral color according to Pillow's drawing-mode rules.
    pub fn color_with_input(&self, input: DrawColorInput) -> Result<(u8, u8, u8, u8), PilError> {
        let mode = self.effective_mode()?;
        match input {
            DrawColorInput::None => Ok(self.default_shape_ink()?.unwrap_or((0, 0, 0, 255))),
            DrawColorInput::String(value) => {
                let color = crate::color::parse_color_str(&value)?;
                if mode == Mode::ImagePalette8 {
                    // ImageColor.getcolor(..., "P") resolves to RGB. Without
                    // a palette, Pillow's single-band draw ink rejects it.
                    Err(draw_single_band_color_error())
                } else {
                    Ok(color)
                }
            }
            DrawColorInput::Integer(value) => resolve_integer_color(mode, value),
            DrawColorInput::WideInteger(value) => resolve_wide_integer_color(mode, &value),
            DrawColorInput::Float(value) if mode == Mode::ImageF32 => {
                let bytes = (value as f32).to_le_bytes();
                Ok((bytes[0], bytes[1], bytes[2], bytes[3]))
            }
            DrawColorInput::Float(_) => Err(draw_float_color_error(mode)),
            DrawColorInput::Components(values) | DrawColorInput::TupleComponents(values) => {
                resolve_component_color(mode, &values)
            }
            DrawColorInput::TupleFloatComponents(values)
                if mode == Mode::ImageF32 && values.len() == 1 =>
            {
                let bytes = (values[0] as f32).to_le_bytes();
                Ok((bytes[0], bytes[1], bytes[2], bytes[3]))
            }
            DrawColorInput::TupleMixedComponents(values)
                if mode == Mode::ImageF32 && values.len() == 1 =>
            {
                match &values[0] {
                    PaletteColorComponent::Integer(value) => resolve_integer_color(mode, *value),
                    PaletteColorComponent::Float(value) => {
                        let bytes = (*value as f32).to_le_bytes();
                        Ok((bytes[0], bytes[1], bytes[2], bytes[3]))
                    }
                    PaletteColorComponent::WideInteger { .. } => {
                        resolve_wide_integer_color(mode, &values[0])
                    }
                }
            }
            DrawColorInput::TupleFloatComponents(_) | DrawColorInput::TupleMixedComponents(_)
                if mode == Mode::ImageF32 =>
            {
                Err(PilError::TypeError(
                    "must be real number, not tuple".to_owned(),
                ))
            }
            DrawColorInput::TupleFloatComponents(_) | DrawColorInput::TupleMixedComponents(_)
                if mode == Mode::ImagePalette8 =>
            {
                Err(draw_single_band_color_error())
            }
            DrawColorInput::TupleComponentsWithWideInteger(values) => {
                resolve_wide_integer_components(mode, &values)
            }
            DrawColorInput::TupleFloatComponents(_) | DrawColorInput::TupleMixedComponents(_) => {
                Err(draw_color_error())
            }
            DrawColorInput::ListComponents(_) | DrawColorInput::InvalidList => {
                Err(draw_python_list_color_error(mode))
            }
            DrawColorInput::InvalidTuple | DrawColorInput::Invalid
                if mode == Mode::ImagePalette8 =>
            {
                Err(draw_single_band_color_error())
            }
            DrawColorInput::InvalidTuple => Err(draw_color_error()),
            DrawColorInput::Invalid => Err(draw_color_error()),
        }
    }

    /// Resolves a Draw color and installs palette entries before queuing it.
    pub fn resolve_draw_color_with_input(
        &mut self,
        input: DrawColorInput,
    ) -> Result<(u8, u8, u8, u8), PilError> {
        if self.effective_mode()? == Mode::ImagePalette8 && self.image.palette_input().rgb.is_some()
        {
            let components = match &input {
                DrawColorInput::String(value) => {
                    let (red, green, blue, _) = crate::color::parse_color_str(value)?;
                    Some(vec![
                        PaletteColorComponent::Integer(i64::from(red)),
                        PaletteColorComponent::Integer(i64::from(green)),
                        PaletteColorComponent::Integer(i64::from(blue)),
                    ])
                }
                DrawColorInput::Components(values) | DrawColorInput::TupleComponents(values) => {
                    Some(
                        values
                            .iter()
                            .copied()
                            .map(PaletteColorComponent::Integer)
                            .collect(),
                    )
                }
                DrawColorInput::TupleComponentsWithWideInteger(values) => Some(values.clone()),
                DrawColorInput::TupleFloatComponents(values) => Some(
                    values
                        .iter()
                        .copied()
                        .map(PaletteColorComponent::Float)
                        .collect(),
                ),
                DrawColorInput::TupleMixedComponents(values) => Some(values.clone()),
                _ => None,
            };
            if let Some(components) = components {
                let color = self.image.prepare_draw_palette_tuple_components(
                    &components,
                    &self.palette_reserved_indices,
                    &mut self.palette_color_cache,
                )?;
                if let PutPixelValue::Integer(index) = color {
                    let index = index as u8;
                    return Ok((index, index, index, 255));
                }
            }
        }
        self.color_with_input(input)
    }

    /// Resolves the default ImageDraw ink according to the selected mode.
    pub fn text_color_with_input(
        &self,
        input: DrawColorInput,
    ) -> Result<(u8, u8, u8, u8), PilError> {
        if !matches!(input, DrawColorInput::None) {
            return self.color_with_input(input);
        }

        Ok(Self::default_draw_ink_for_mode(self.effective_mode()?))
    }

    /// Resolve ImageDraw's omitted fill from the selected drawing mode.
    pub(crate) fn default_draw_ink_for_mode(mode: Mode) -> (u8, u8, u8, u8) {
        match mode {
            Mode::ImageRgb8
            | Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageLumaA8
            | Mode::ImageLaPremultiplied8
            | Mode::ImagePaletteAlpha8
            | Mode::ImageCmyk8
            | Mode::ImageYCbCr8
            | Mode::ImageLab8
            | Mode::ImageHsv8 => (255, 255, 255, 255),
            Mode::ImageI32 => (1, 0, 0, 0),
            Mode::ImageF32 => (0, 0, 128, 63),
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                (255, 255, 0, 0)
            }
            _ => (0, 0, 0, 255),
        }
    }

    /// Resolves a text ink and any P-mode palette entry before raster work.
    pub fn resolve_text_color_with_input(
        &mut self,
        input: DrawColorInput,
    ) -> Result<(u8, u8, u8, u8), PilError> {
        if matches!(&input, DrawColorInput::None) {
            self.text_color_with_input(input)
        } else {
            self.resolve_draw_color_with_input(input)
        }
    }
}
