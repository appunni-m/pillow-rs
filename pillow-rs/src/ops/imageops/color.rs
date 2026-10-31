use super::ImageOpsColor;
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) fn resolve_imageops_color(
    input: ImageOpsColor,
    mode: Mode,
) -> Result<Option<(u8, u8, u8, u8)>, PilError> {
    match input {
        ImageOpsColor::None => Ok(None),
        ImageOpsColor::Invalid => Err(PilError::TypeError("color must be int or tuple".into())),
        ImageOpsColor::Name(name) => resolve_named_color(&name, mode).map(Some),
        ImageOpsColor::Scalar(value) => Ok(Some(scalar_color(value, mode))),
        ImageOpsColor::Components(values) => resolve_color_components(&values, mode).map(Some),
    }
}

fn resolve_named_color(name: &str, mode: Mode) -> Result<(u8, u8, u8, u8), PilError> {
    let (red, green, blue, alpha) = crate::color::parse_color_str(name)?;
    if mode == Mode::ImagePalette8 {
        // Pillow creates a temporary palette entry for tuple/name fills, then
        // pastes into the source palette without copying it. The fill is index
        // zero; only scalar colors are raw palette indices.
        return Ok((0, 0, 0, u8::MAX));
    }
    let value = crate::color::getcolor_for_mode(
        i32::from(red),
        i32::from(green),
        i32::from(blue),
        i32::from(alpha),
        mode,
    );
    Ok(color_value(value, mode))
}

fn resolve_color_components(values: &[i64], mode: Mode) -> Result<(u8, u8, u8, u8), PilError> {
    match values {
        [value] => Ok(scalar_color(*value, mode)),
        [_, _, _] if mode == Mode::ImagePalette8 => Ok((0, 0, 0, u8::MAX)),
        [_, _, _, alpha]
            if mode == Mode::ImagePalette8 && *alpha == i64::from(u8::MAX) =>
        {
            Ok((0, 0, 0, u8::MAX))
        }
        [_, _, _, _] if mode == Mode::ImagePalette8 => Err(PilError::ValueError(
            "cannot add non-opaque RGBA color to RGB palette".into(),
        )),
        [cyan, magenta, yellow, black] if mode == Mode::ImageCmyk8 => Ok((
            clamp_color_component(*cyan),
            clamp_color_component(*magenta),
            clamp_color_component(*yellow),
            clamp_color_component(*black),
        )),
        [value, alpha]
            if matches!(mode, Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8) =>
        {
            Ok((
            clamp_color_component(*value),
            clamp_color_component(*value),
            clamp_color_component(*value),
            clamp_color_component(*alpha),
            ))
        }
        [red, green, blue] | [red, green, blue, _]
            if !is_luma_mode(mode) && mode != Mode::ImageLumaA8 =>
        {
            let alpha = if matches!(
                mode,
                Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8
            ) && values.len() == 4
            {
                clamp_color_component(values[3])
            } else {
                u8::MAX
            };
            Ok((
                clamp_color_component(*red),
                clamp_color_component(*green),
                clamp_color_component(*blue),
                alpha,
            ))
        }
        _ => Err(invalid_color(mode)),
    }
}

fn color_value(value: crate::color::ColorValue, mode: Mode) -> (u8, u8, u8, u8) {
    match value {
        crate::color::ColorValue::Gray(value) => scalar_color(i64::from(value), mode),
        crate::color::ColorValue::GrayAlpha(value, alpha) => {
            let value = clamp_color_component(i64::from(value));
            (value, value, value, clamp_color_component(i64::from(alpha)))
        }
        crate::color::ColorValue::Rgb(red, green, blue) => (
            clamp_color_component(i64::from(red)),
            clamp_color_component(i64::from(green)),
            clamp_color_component(i64::from(blue)),
            u8::MAX,
        ),
        crate::color::ColorValue::Rgba(red, green, blue, alpha) => (
            clamp_color_component(i64::from(red)),
            clamp_color_component(i64::from(green)),
            clamp_color_component(i64::from(blue)),
            clamp_color_component(i64::from(alpha)),
        ),
        crate::color::ColorValue::Hsv(hue, saturation, value) => (
            clamp_color_component(i64::from(hue)),
            clamp_color_component(i64::from(saturation)),
            clamp_color_component(i64::from(value)),
            u8::MAX,
        ),
    }
}

fn scalar_color(value: i64, mode: Mode) -> (u8, u8, u8, u8) {
    if mode == Mode::ImageF32 {
        // Keep F samples in their native four-byte scalar representation.
        let [red, green, blue, alpha] = (value as f32).to_le_bytes();
        return (red, green, blue, alpha);
    }
    if mode == Mode::ImageI32 {
        // I-mode fills carry one signed little-endian int32 sample.
        let [red, green, blue, alpha] = (value as i32).to_le_bytes();
        return (red, green, blue, alpha);
    }
    let value = clamp_color_component(value);
    if is_luma_mode(mode) {
        return (value, value, value, u8::MAX);
    }
    if mode == Mode::ImageLumaA8 {
        return (value, value, value, 0);
    }
    if mode == Mode::ImagePalette8 {
        return (value, 0, 0, u8::MAX);
    }
    let alpha = if has_zero_scalar_fourth_byte(mode) {
        0
    } else {
        u8::MAX
    };
    (value, 0, 0, alpha)
}

fn clamp_color_component(value: i64) -> u8 {
    value.clamp(0, i64::from(u8::MAX)) as u8
}

fn is_luma_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::One
            | Mode::ImageLuma8
            | Mode::ImageI32
            | Mode::ImageF32
            | Mode::ImageI16
            | Mode::ImageI16L
            | Mode::ImageI16B
            | Mode::ImageI16N
    )
}

fn has_zero_scalar_fourth_byte(mode: Mode) -> bool {
    // RGBX, CMYK, and alpha storage modes follow Pillow's packed scalar rule.
    matches!(
        mode,
        Mode::ImageLumaA8
            | Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageCmyk8
            | Mode::ImagePaletteAlpha8
    )
}

fn invalid_color(mode: Mode) -> PilError {
    if mode == Mode::ImageF32 {
        PilError::TypeError("must be real number, not tuple".into())
    } else if is_luma_mode(mode) {
        PilError::TypeError("color must be int or single-element tuple".into())
    } else if mode == Mode::ImageLumaA8 {
        PilError::TypeError("color must be int, or tuple of one or two elements".into())
    } else {
        PilError::TypeError("color must be int, or tuple of one, three or four elements".into())
    }
}
