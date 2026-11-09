use super::TransformFill;
use crate::error::PilError;
use crate::raster::Mode;

type TransformFillResult = ((u8, u8, u8, u8), Option<u8>, Option<Vec<u8>>);

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
        | Mode::ImageI16N => "color must be int or single-element tuple",
        Mode::ImageF32 => "must be real number, not tuple",
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageLumaA16 => "color must be int, or tuple of one or two elements",
        _ => "color must be int, or tuple of one, three or four elements",
    };
    PilError::TypeError(message.into())
}

fn transform_scalar_type_error(mode: Mode) -> PilError {
    // Pillow's getink separates one-band scalar faults from packed colors.
    let message = match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImagePalette8
        | Mode::ImageI32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => "color must be int or single-element tuple",
        _ => "color must be int or tuple",
    };
    PilError::TypeError(message.into())
}

fn transform_list_error(mode: Mode) -> PilError {
    let message = match mode {
        Mode::ImageF32 => "must be real number, not list",
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImagePalette8
        | Mode::ImageI32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => "color must be int or single-element tuple",
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
            Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
                let value = clamp(value);
                Ok((value, u8::MAX, value, value))
            }
            _ => Ok((clamp(value), 0, 0, 0)),
        },
        crate::color::ColorValue::GrayAlpha(gray, alpha) => {
            let gray = clamp(gray);
            if matches!(
                mode,
                Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8
            ) {
                Ok((gray, clamp(alpha), gray, gray))
            } else {
                Ok((gray, clamp(alpha), 0, 0))
            }
        }
        crate::color::ColorValue::Rgb(red, green, blue) => Ok((
            clamp(red),
            clamp(green),
            clamp(blue),
            if matches!(
                mode,
                Mode::ImageRgba8
                    | Mode::ImageRgbx8
                    | Mode::ImageCmyk8
                    | Mode::ImageRgb8
                    | Mode::ImageYCbCr8
                    | Mode::ImageLab8
                    | Mode::ImageHsv8
            ) {
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
        crate::color::ColorValue::Hsv(hue, saturation, value) => Ok((
            clamp(hue),
            clamp(saturation),
            clamp(value),
            if mode == Mode::ImageHsv8 { u8::MAX } else { 0 },
        )),
    }
}

fn transform_components_fill(
    mode: Mode,
    values: &[i64],
    default_palette_fill: Option<u8>,
) -> Result<TransformFillResult, PilError> {
    let fill = match mode {
        Mode::ImagePalette8 => {
            return transform_palette_components(values, default_palette_fill);
        }
        Mode::One | Mode::ImageLuma8 => single_byte_component_fill(mode, values)?,
        Mode::ImageI32 => single_value_fill(mode, values, packed_transform_scalar)?,
        Mode::ImageF32 => single_value_fill(mode, values, packed_transform_float)?,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            single_i16_component_fill(mode, values)?
        }
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            luma_alpha_component_fill(mode, values)?
        }
        Mode::ImageLumaA16 => luma_alpha16_component_fill(mode, values)?,
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            rgb_component_fill(mode, values)?
        }
        Mode::ImageRgb16 | Mode::ImageRgb32F => rgb16_component_fill(mode, values)?,
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageRgba16
        | Mode::ImageRgba32F => rgba_component_fill(mode, values)?,
    };
    Ok((fill, default_palette_fill, None))
}

fn transform_palette_components(
    values: &[i64],
    default_palette_fill: Option<u8>,
) -> Result<TransformFillResult, PilError> {
    match values {
        [value] => {
            let index = (*value).clamp(0, i64::from(u8::MAX)) as u8;
            Ok(((index, 0, 0, 255), Some(index), None))
        }
        [red, green, blue] => {
            let _ = (
                transform_component(*red)?,
                transform_component(*green)?,
                transform_component(*blue)?,
            );
            Ok((
                (0, 0, 0, 255),
                default_palette_fill,
                Some(vec![*red as u8, *green as u8, *blue as u8]),
            ))
        }
        [red, green, blue, alpha] => {
            if *alpha != i64::from(u8::MAX) {
                return Err(PilError::ValueError(
                    "cannot add non-opaque RGBA color to RGB palette".into(),
                ));
            }
            let _ = (
                transform_component(*red)?,
                transform_component(*green)?,
                transform_component(*blue)?,
            );
            Ok((
                (0, 0, 0, 255),
                default_palette_fill,
                Some(vec![*red as u8, *green as u8, *blue as u8]),
            ))
        }
        _ => Err(transform_fill_arity_error(Mode::ImagePalette8)),
    }
}

fn single_byte_component_fill(mode: Mode, values: &[i64]) -> Result<(u8, u8, u8, u8), PilError> {
    match values {
        [value] => Ok((clamp_transform_component(*value), 0, 0, 0)),
        _ => Err(transform_fill_arity_error(mode)),
    }
}

fn single_value_fill(
    mode: Mode,
    values: &[i64],
    pack: fn(i64) -> (u8, u8, u8, u8),
) -> Result<(u8, u8, u8, u8), PilError> {
    match values {
        [value] => Ok(pack(*value)),
        _ => Err(transform_fill_arity_error(mode)),
    }
}

fn single_i16_component_fill(mode: Mode, values: &[i64]) -> Result<(u8, u8, u8, u8), PilError> {
    match values {
        [value] => {
            let bytes = (*value as u16).to_le_bytes();
            Ok((bytes[0], bytes[1], 0, 0))
        }
        _ => Err(transform_fill_arity_error(mode)),
    }
}

fn luma_alpha_component_fill(mode: Mode, values: &[i64]) -> Result<(u8, u8, u8, u8), PilError> {
    match values {
        [value] => {
            let gray = clamp_transform_component(*value);
            Ok((gray, u8::MAX, gray, gray))
        }
        [gray, alpha] => {
            let gray = clamp_transform_component(*gray);
            Ok((gray, clamp_transform_component(*alpha), gray, gray))
        }
        _ => Err(transform_fill_arity_error(mode)),
    }
}

fn luma_alpha16_component_fill(mode: Mode, values: &[i64]) -> Result<(u8, u8, u8, u8), PilError> {
    match values {
        [value] => Ok((clamp_transform_component(*value), 0, 0, 0)),
        [gray, alpha] => Ok((
            clamp_transform_component(*gray),
            clamp_transform_component(*alpha),
            0,
            0,
        )),
        _ => Err(transform_fill_arity_error(mode)),
    }
}

fn rgb_component_fill(mode: Mode, values: &[i64]) -> Result<(u8, u8, u8, u8), PilError> {
    match values {
        [value] => Ok(packed_transform_scalar(*value)),
        [red, green, blue] => Ok((
            clamp_transform_component(*red),
            clamp_transform_component(*green),
            clamp_transform_component(*blue),
            u8::MAX,
        )),
        [red, green, blue, alpha] => Ok((
            clamp_transform_component(*red),
            clamp_transform_component(*green),
            clamp_transform_component(*blue),
            clamp_transform_component(*alpha),
        )),
        _ => Err(transform_fill_arity_error(mode)),
    }
}

fn rgb16_component_fill(mode: Mode, values: &[i64]) -> Result<(u8, u8, u8, u8), PilError> {
    match values {
        [value] => Ok(packed_transform_scalar(*value)),
        [red, green, blue] | [red, green, blue, _] => Ok((
            clamp_transform_component(*red),
            clamp_transform_component(*green),
            clamp_transform_component(*blue),
            0,
        )),
        _ => Err(transform_fill_arity_error(mode)),
    }
}

fn rgba_component_fill(mode: Mode, values: &[i64]) -> Result<(u8, u8, u8, u8), PilError> {
    match values {
        [value] => Ok(packed_transform_scalar(*value)),
        [red, green, blue] => Ok((
            clamp_transform_component(*red),
            clamp_transform_component(*green),
            clamp_transform_component(*blue),
            u8::MAX,
        )),
        [red, green, blue, alpha] => Ok((
            clamp_transform_component(*red),
            clamp_transform_component(*green),
            clamp_transform_component(*blue),
            clamp_transform_component(*alpha),
        )),
        _ => Err(transform_fill_arity_error(mode)),
    }
}

pub(crate) fn public_transform_fill(
    fillcolor: Option<TransformFill>,
    mode: Mode,
) -> Result<TransformFillResult, PilError> {
    let palette_mode = mode == Mode::ImagePalette8;
    let default_palette_fill = palette_mode.then_some(0);
    match fillcolor {
        // Image.transform creates an uninitialized image when no fill
        // color is supplied. The native transform then leaves exposed
        // pixels zeroed, including alpha, rather than using opaque black.
        None => Ok(((0, 0, 0, 0), default_palette_fill, None)),
        Some(TransformFill::Scalar(value)) if palette_mode => {
            let index = value.clamp(0, i64::from(u8::MAX)) as u8;
            Ok(((index, 0, 0, 255), Some(index), None))
        }
        Some(TransformFill::Scalar(value))
            if matches!(
                mode,
                Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8
            ) =>
        {
            let (luma, lane_one, lane_two, alpha) = packed_transform_scalar(value);
            Ok(((luma, alpha, lane_one, lane_two), None, None))
        }
        Some(TransformFill::Scalar(value)) => {
            let fill = match mode {
                Mode::One | Mode::ImageLuma8 => (clamp_transform_component(value), 0, 0, 0),
                Mode::ImageF32 => packed_transform_float(value),
                Mode::ImageI32 => packed_transform_scalar(value),
                Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                    let bytes = (value as u16).to_le_bytes();
                    (bytes[0], bytes[1], 0, 0)
                }
                _ => packed_transform_scalar(value),
            };
            Ok((fill, None, None))
        }
        Some(TransformFill::Components(values)) => {
            transform_components_fill(mode, &values, default_palette_fill)
        }
        Some(TransformFill::PaletteComponents(values)) if palette_mode => {
            let [red, green, blue] = crate::ops::imageops::resolve_palette_tuple_bytes(&values)?;
            Ok((
                (0, 0, 0, u8::MAX),
                default_palette_fill,
                Some(vec![red, green, blue]),
            ))
        }
        Some(TransformFill::PaletteComponents(_)) => {
            Err(PilError::TypeError("color must be int or tuple".into()))
        }
        Some(TransformFill::Name(name)) => {
            // Color parsing and mode conversion belong to core. Pillow's
            // palette transform resolves named colors through a temporary
            // palette and rasterizes them as index zero.
            let (r, g, b, a) = crate::color::parse_color_str_unclamped(&name)?;
            if palette_mode {
                let palette = vec![
                    transform_component(i64::from(r))?,
                    transform_component(i64::from(g))?,
                    transform_component(i64::from(b))?,
                ];
                return Ok(((0, 0, 0, 255), default_palette_fill, Some(palette)));
            }
            let color = crate::color::getcolor_for_mode(r, g, b, a, mode);
            Ok((transform_color_value_fill(mode, color)?, None, None))
        }
        Some(TransformFill::FloatingScalar(value)) if mode == Mode::ImageF32 => {
            Ok((packed_transform_float64(value), None, None))
        }
        Some(TransformFill::FloatingScalar(_)) => Err(transform_scalar_type_error(mode)),
        Some(TransformFill::FloatingComponents(values)) if mode == Mode::ImageF32 => {
            match values.as_slice() {
                [value] => Ok((packed_transform_float64(*value), None, None)),
                _ => Err(transform_fill_arity_error(mode)),
            }
        }
        Some(TransformFill::FloatingComponents(_)) => Err(PilError::TypeError(
            "'float' object cannot be interpreted as an integer".into(),
        )),
        Some(TransformFill::Invalid) => Err(transform_scalar_type_error(mode)),
        Some(TransformFill::InvalidList) => Err(transform_list_error(mode)),
    }
}
