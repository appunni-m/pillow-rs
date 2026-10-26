use super::TransformFill;
use crate::error::PilError;
use crate::image::Image;

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

fn transform_fill_arity_error(mode: &str) -> PilError {
    let message = match mode {
        "1" | "L" | "P" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" => {
            "color must be int or single-element tuple"
        }
        "F" => "must be real number, not tuple",
        "LA" | "La" | "PA" => "color must be int, or tuple of one or two elements",
        _ => "color must be int, or tuple of one, three or four elements",
    };
    PilError::TypeError(message.into())
}

fn transform_float_scalar_error(mode: &str) -> PilError {
    let message = match mode {
        "1" | "L" | "P" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" => {
            "color must be int or single-element tuple"
        }
        _ => "color must be int or tuple",
    };
    PilError::TypeError(message.into())
}

fn transform_color_value_fill(
    mode: &str,
    color: crate::color::ColorValue,
) -> Result<(u8, u8, u8, u8), PilError> {
    let clamp = |value: i32| value.clamp(0, i32::from(u8::MAX)) as u8;
    match color {
        crate::color::ColorValue::Gray(value) => match mode {
            "F" => Ok(packed_transform_float(i64::from(value))),
            "I" => Ok(packed_transform_scalar(i64::from(value))),
            "I;16" | "I;16L" | "I;16B" | "I;16N" => {
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
            if matches!(mode, "RGBA" | "CMYK") {
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
    mode: &str,
    values: &[i64],
    default_palette_fill: Option<u8>,
) -> Result<TransformFillResult, PilError> {
    let fill = match mode {
        "P" => match values {
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
        "1" | "L" => match values {
            [value] => (clamp_transform_component(*value), 0, 0, 0),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        "I" => match values {
            [value] => packed_transform_scalar(*value),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        "F" => match values {
            [value] => packed_transform_float(*value),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        "I;16" | "I;16L" | "I;16B" | "I;16N" => match values {
            [value] => {
                let bytes = (*value as u16).to_le_bytes();
                (bytes[0], bytes[1], 0, 0)
            }
            _ => return Err(transform_fill_arity_error(mode)),
        },
        "LA" | "La" | "PA" => match values {
            [value] => (clamp_transform_component(*value), 0, 0, 0),
            [gray, alpha] => (
                clamp_transform_component(*gray),
                clamp_transform_component(*alpha),
                0,
                0,
            ),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        _ if matches!(mode, "RGB" | "YCbCr" | "HSV") => match values {
            [value] => packed_transform_scalar(*value),
            [red, green, blue] | [red, green, blue, _] => (
                clamp_transform_component(*red),
                clamp_transform_component(*green),
                clamp_transform_component(*blue),
                0,
            ),
            _ => return Err(transform_fill_arity_error(mode)),
        },
        _ => match values {
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

impl Image {
    pub(crate) fn public_transform_fill(
        &self,
        fillcolor: Option<TransformFill>,
    ) -> Result<TransformFillResult, PilError> {
        let mode = self.mode()?;
        let palette_mode = self.has_palette_mode();
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
                let fill = match mode.as_str() {
                    "1" | "L" => (clamp_transform_component(value), 0, 0, 0),
                    "F" => packed_transform_float(value),
                    "I" => packed_transform_scalar(value),
                    "I;16" | "I;16L" | "I;16B" | "I;16N" => {
                        let bytes = (value as u16).to_le_bytes();
                        (bytes[0], bytes[1], 0, 0)
                    }
                    _ => packed_transform_scalar(value),
                };
                Ok((fill, None))
            }
            Some(TransformFill::Components(values)) => {
                transform_components_fill(&mode, &values, default_palette_fill)
            }
            Some(TransformFill::Name(name)) => {
                // Color parsing and mode conversion belong to core. Pillow's
                // palette transform resolves named colors through a temporary
                // palette and rasterizes them as index zero.
                let (r, g, b, a) = crate::color::parse_color_str_unclamped(&name)?;
                if palette_mode {
                    crate::color::getcolor(r, g, b, a, &mode)?;
                    return Ok(((0, 0, 0, 255), default_palette_fill));
                }
                let color = crate::color::getcolor(r, g, b, a, &mode)?;
                Ok((transform_color_value_fill(&mode, color)?, None))
            }
            Some(TransformFill::FloatingScalar(value)) if mode == "F" => {
                Ok((packed_transform_float64(value), None))
            }
            Some(TransformFill::FloatingScalar(_)) => Err(transform_float_scalar_error(&mode)),
            Some(TransformFill::FloatingComponents(values)) if mode == "F" => {
                match values.as_slice() {
                    [value] => Ok((packed_transform_float64(*value), None)),
                    _ => Err(transform_fill_arity_error(&mode)),
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
}
