use super::TransformFill;
use crate::error::PilError;
use crate::raster::Mode;

type TransformFillResult = ((u8, u8, u8, u8), Option<u8>);

fn transform_component(value: i64) -> Result<u8, PilError> {
    u8::try_from(value).map_err(|_| PilError::ValueError("bytes must be in range(0, 256)".into()))
}

fn clamp_transform_component(value: i64) -> u8 {
    value.clamp(0, i64::from(u8::MAX)) as u8
}

fn packed_transform_scalar(value: i64) -> (u8, u8, u8, u8) {
    let bytes = value.to_le_bytes();
    (bytes[0], bytes[1], bytes[2], bytes[3])
}

fn packed_transform_float(value: i64) -> (u8, u8, u8, u8) {
    let bytes = (value as f32).to_le_bytes();
    (bytes[0], bytes[1], bytes[2], bytes[3])
}

fn packed_transform_float64(value: f64) -> (u8, u8, u8, u8) {
    let bytes = (value as f32).to_le_bytes();
    (bytes[0], bytes[1], bytes[2], bytes[3])
}

fn transform_fill_arity_error(mode: Mode) -> PilError {
    let message = match mode {
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
        Mode::ImageF32 => "must be real number, not tuple",
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageLumaA16 => {
            "color must be int, or tuple of one or two elements"
        }
        _ => "color must be int, or tuple of one, three or four elements",
    };
    PilError::TypeError(message.into())
}

fn transform_float_scalar_error(mode: Mode) -> PilError {
    let message = match mode {
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
        _ => "color must be int or tuple",
    };
    PilError::TypeError(message.into())
}

fn transform_color_value_fill(
    mode: Mode,
    color: crate::color::ColorValue,
) -> Result<(u8, u8, u8, u8), PilError> {
    let clamp = |value: i32| value.clamp(0, i32::from(u8::MAX)) as u8;
    match color {
        crate::color::ColorValue::Gray(value) => match mode {
            Mode::ImageF32 => Ok(packed_transform_float(i64::from(value))),
            Mode::ImageI32 => Ok(packed_transform_scalar(i64::from(value))),
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                let bytes = (value as u16).to_le_bytes();
                Ok((bytes[0], bytes[1], 0, 0))
            }
            _ => Ok((clamp(value), 0, 0, 0)),
        },
        crate::color::ColorValue::GrayAlpha(gray, alpha) => Ok((clamp(gray), clamp(alpha), 0, 0)),
        crate::color::ColorValue::Rgb(red, green, blue) => Ok((
            clamp(red),
            clamp(green),
            clamp(blue),
            if matches!(mode, Mode::ImageRgba8 | Mode::ImageCmyk8) {
                255
            } else {
                0
            },
        )),
        crate::color::ColorValue::Rgba(red, green, blue, alpha) => Ok((
            clamp(red),
            clamp(green),
            clamp(blue),
            // `getcolor` constructs `Rgba` only for alpha-suffixed modes, so
            // the alpha component is part of the destination representation.
            clamp(alpha),
        )),
        crate::color::ColorValue::Hsv(hue, saturation, value) => {
            Ok((clamp(hue), clamp(saturation), clamp(value), 0))
        }
    }
}

fn transform_components_fill(
    mode: Mode,
    values: &[i64],
    default_palette_fill: Option<u8>,
) -> Result<TransformFillResult, PilError> {
    let fill = match mode {
        Mode::ImagePalette8 => match values {
            [value] => {
                let index = (*value).clamp(0, i64::from(u8::MAX)) as u8;
                return Ok(((index, 0, 0, 255), Some(index)));
            }
            [red, green, blue] => {
                let r = transform_component(*red)?;
                let g = transform_component(*green)?;
                let b = transform_component(*blue)?;
                let _ = (r, g, b);
                return Ok(((0, 0, 0, 255), default_palette_fill));
            }
            [red, green, blue, alpha] => {
                let r = transform_component(*red)?;
                let g = transform_component(*green)?;
                let b = transform_component(*blue)?;
                let a = transform_component(*alpha)?;
                let _ = (r, g, b);
                if a != 255 {
                    return Err(PilError::ValueError(
                        "cannot add non-opaque RGBA color to RGB palette".into(),
                    ));
                }
                return Ok(((0, 0, 0, 255), default_palette_fill));
            }
            _ => return Err(transform_fill_arity_error(mode)),
        },
        Mode::One | Mode::ImageLuma8 => match values {
            [value] => (clamp_transform_component(*value), 0, 0, 0),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        Mode::ImageI32 => match values {
            [value] => packed_transform_scalar(*value),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        Mode::ImageF32 => match values {
            [value] => packed_transform_float(*value),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => match values {
            [value] => {
                let bytes = (*value as u16).to_le_bytes();
                (bytes[0], bytes[1], 0, 0)
            }
            _ => return Err(transform_fill_arity_error(mode)),
        },
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageLumaA16 => match values {
            [value] => (clamp_transform_component(*value), 0, 0, 0),
            [gray, alpha] => (
                clamp_transform_component(*gray),
                clamp_transform_component(*alpha),
                0,
                0,
            ),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        Mode::ImageRgb8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageRgb16
        | Mode::ImageRgb32F => match values {
            [value] => packed_transform_scalar(*value),
            [red, green, blue] | [red, green, blue, _] => (
                clamp_transform_component(*red),
                clamp_transform_component(*green),
                clamp_transform_component(*blue),
                0,
            ),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageRgba16
        | Mode::ImageRgba32F => match values {
            [value] => packed_transform_scalar(*value),
            [red, green, blue] => (
                clamp_transform_component(*red),
                clamp_transform_component(*green),
                clamp_transform_component(*blue),
                255,
            ),
            [red, green, blue, alpha] => (
                clamp_transform_component(*red),
                clamp_transform_component(*green),
                clamp_transform_component(*blue),
                clamp_transform_component(*alpha),
            ),
            _ => return Err(transform_fill_arity_error(mode)),
        },
    };
    Ok((fill, default_palette_fill))
}

pub(super) fn public_transform_fill(
    fillcolor: Option<TransformFill>,
    mode: Mode,
) -> Result<TransformFillResult, PilError> {
    let palette_mode = mode == Mode::ImagePalette8;
    let default_palette_fill = palette_mode.then_some(0);
    match fillcolor {
        // Image.transform creates an uninitialized image when no fill
        // color is supplied. The native transform then leaves exposed
        // pixels zeroed, including alpha, rather than using opaque black.
        None => Ok(((0, 0, 0, 0), default_palette_fill)),
        Some(TransformFill::Scalar(value)) if palette_mode => {
            let index = value.clamp(0, i64::from(u8::MAX)) as u8;
            Ok(((index, 0, 0, 255), Some(index)))
        }
        Some(TransformFill::Scalar(value)) => {
            let fill = match mode {
                Mode::One | Mode::ImageLuma8 => {
                    (clamp_transform_component(value), 0, 0, 0)
                }
                Mode::ImageF32 => packed_transform_float(value),
                Mode::ImageI32 => packed_transform_scalar(value),
                Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                    let bytes = (value as u16).to_le_bytes();
                    (bytes[0], bytes[1], 0, 0)
                }
                _ => packed_transform_scalar(value),
            };
            Ok((fill, None))
        }
        Some(TransformFill::Components(values)) => {
            transform_components_fill(mode, &values, default_palette_fill)
        }
        Some(TransformFill::Name(name)) => {
            // Color parsing and mode conversion belong to core. Pillow's
            // palette transform resolves named colors through a temporary
            // palette and rasterizes them as index zero.
            let (r, g, b, a) = crate::color::parse_color_str_unclamped(&name)?;
            if palette_mode {
                return Ok(((0, 0, 0, 255), default_palette_fill));
            }
            let color = crate::color::getcolor_for_mode(r, g, b, a, mode);
            Ok((transform_color_value_fill(mode, color)?, None))
        }
        Some(TransformFill::FloatingScalar(value)) if mode == Mode::ImageF32 => {
            Ok((packed_transform_float64(value), None))
        }
        Some(TransformFill::FloatingScalar(_)) => Err(transform_float_scalar_error(mode)),
        Some(TransformFill::FloatingComponents(values)) if mode == Mode::ImageF32 => {
            match values.as_slice() {
                [value] => Ok((packed_transform_float64(*value), None)),
                _ => Err(transform_fill_arity_error(mode)),
            }
        }
        Some(TransformFill::FloatingComponents(_)) => Err(PilError::TypeError(
            "'float' object cannot be interpreted as an integer".into(),
        )),
        Some(TransformFill::Invalid) => {
            Err(PilError::TypeError("color must be int or tuple".into()))
        }
    }
}

