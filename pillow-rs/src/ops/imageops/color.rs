use super::ImageOpsColor;
use crate::compute::registry::support::PadFillRule;
use crate::error::PilError;
use crate::image::Image;
use crate::raster::Mode;

impl PartialEq for ImageOpsColor {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) | (Self::Invalid, Self::Invalid) => true,
            (Self::Name(left), Self::Name(right)) => left == right,
            (Self::Scalar(left), Self::Scalar(right)) => left == right,
            (Self::Components(left), Self::Components(right)) => left == right,
            (Self::TupleComponents(left), Self::TupleComponents(right)) => left == right,
            (Self::TupleNumericComponents(left), Self::TupleNumericComponents(right)) => {
                palette_components_equal(left, right)
            }
            (
                Self::TupleComponentsWithFallback {
                    components: left,
                    fallback: left_fallback,
                },
                Self::TupleComponentsWithFallback {
                    components: right,
                    fallback: right_fallback,
                },
            ) => {
                palette_components_equal(left, right) && left_fallback == right_fallback
            }
            (
                Self::WideTupleComponents {
                    components: left,
                    fallback: left_fallback,
                },
                Self::WideTupleComponents {
                    components: right,
                    fallback: right_fallback,
                },
            ) => {
                palette_components_equal(left, right) && left_fallback == right_fallback
            }
            (Self::ListNumericComponents(left), Self::ListNumericComponents(right)) => {
                palette_components_equal(left, right)
            }
            (Self::ListComponents(left), Self::ListComponents(right)) => left == right,
            (
                Self::PaletteFill {
                    index: left_index,
                    palette: left_palette,
                },
                Self::PaletteFill {
                    index: right_index,
                    palette: right_palette,
                },
            ) => left_index == right_index && left_palette == right_palette,
            (Self::FloatingScalar(left), Self::FloatingScalar(right)) => {
                left.to_bits() == right.to_bits()
            }
            (Self::FloatingComponents(left), Self::FloatingComponents(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| left.to_bits() == right.to_bits())
            }
            (Self::InvalidList, Self::InvalidList) => true,
            _ => false,
        }
    }
}

impl Eq for ImageOpsColor {}

fn palette_components_equal(
    left: &[crate::image::PaletteColorComponent],
    right: &[crate::image::PaletteColorComponent],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| match (left, right) {
            (
                crate::image::PaletteColorComponent::Integer(left),
                crate::image::PaletteColorComponent::Integer(right),
            ) => left == right,
            (
                crate::image::PaletteColorComponent::WideInteger {
                    negative: left_negative,
                    magnitude: left_magnitude,
                },
                crate::image::PaletteColorComponent::WideInteger {
                    negative: right_negative,
                    magnitude: right_magnitude,
                },
            ) => left_negative == right_negative && left_magnitude == right_magnitude,
            (
                crate::image::PaletteColorComponent::Float(left),
                crate::image::PaletteColorComponent::Float(right),
            ) => left.to_bits() == right.to_bits(),
            _ => false,
        })
}

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
        ImageOpsColor::TupleComponents(values) => {
            resolve_color_components(&values, mode).map(Some)
        }
        ImageOpsColor::TupleNumericComponents(components) if mode == Mode::ImagePalette8 => {
            resolve_palette_tuple_fill(&components).map(Some)
        }
        ImageOpsColor::TupleNumericComponents(_) => {
            Err(PilError::TypeError("color must be int or tuple".into()))
        }
        ImageOpsColor::TupleComponentsWithFallback {
            components: _,
            fallback,
        } => {
            resolve_imageops_color(*fallback, mode)
        }
        ImageOpsColor::WideTupleComponents {
            components,
            fallback: _,
        } if mode == Mode::ImagePalette8 && matches!(components.len(), 3 | 4) => {
            resolve_palette_tuple_fill(&components).map(Some)
        }
        ImageOpsColor::WideTupleComponents {
            components: _,
            fallback,
        } => resolve_imageops_color(*fallback, mode),
        ImageOpsColor::ListNumericComponents(components)
            if mode == Mode::ImagePalette8 && matches!(components.len(), 3 | 4) =>
        {
            resolve_palette_tuple_fill(&components).map(Some)
        }
        ImageOpsColor::ListNumericComponents(_) => Err(python_list_color_error(mode)),
        ImageOpsColor::PaletteFill { index, .. } if mode == Mode::ImagePalette8 => {
            Ok(Some((index, index, index, u8::MAX)))
        }
        ImageOpsColor::PaletteFill { .. } => {
            Err(PilError::TypeError("color must be int or tuple".into()))
        }
        ImageOpsColor::ListComponents(values) => {
            resolve_python_integer_list(&values, mode).map(Some)
        }
        ImageOpsColor::FloatingScalar(value) => floating_scalar_color(value, mode).map(Some),
        ImageOpsColor::FloatingComponents(values) => {
            floating_components_color(&values, mode).map(Some)
        }
        ImageOpsColor::InvalidList => Err(python_list_color_error(mode)),
    }
}

pub(crate) fn palette_fill_components(
    fill: &ImageOpsColor,
) -> Result<Option<Vec<crate::image::PaletteColorComponent>>, PilError> {
    let components = match fill {
        ImageOpsColor::Components(values)
        | ImageOpsColor::TupleComponents(values)
        | ImageOpsColor::ListComponents(values)
            if matches!(values.len(), 3 | 4) =>
        {
            values
                .iter()
                .copied()
                .map(crate::image::PaletteColorComponent::Integer)
                .collect()
        }
        ImageOpsColor::FloatingComponents(values) if matches!(values.len(), 3 | 4) => {
            values
                .iter()
                .copied()
                .map(crate::image::PaletteColorComponent::Float)
                .collect()
        }
        ImageOpsColor::TupleNumericComponents(values)
        | ImageOpsColor::ListNumericComponents(values)
            if matches!(values.len(), 3 | 4) =>
        {
            values.clone()
        }
        ImageOpsColor::TupleComponentsWithFallback { components, .. }
        | ImageOpsColor::WideTupleComponents { components, .. }
            if matches!(components.len(), 3 | 4) =>
        {
            components.clone()
        }
        ImageOpsColor::Name(name) => {
            // ImageColor.getcolor("...", "P") returns RGB and discards alpha.
            let (red, green, blue, _) = crate::color::parse_color_str_unclamped(name)?;
            vec![
                crate::image::PaletteColorComponent::Integer(i64::from(red)),
                crate::image::PaletteColorComponent::Integer(i64::from(green)),
                crate::image::PaletteColorComponent::Integer(i64::from(blue)),
            ]
        }
        _ => return Ok(None),
    };
    Ok(Some(components))
}

pub(crate) fn resolve_pad_palette_fill(
    image: &Image,
    fill_rule: PadFillRule,
    fill: &ImageOpsColor,
) -> Result<Option<(u8, Vec<u8>, Option<Vec<u8>>)>, PilError> {
    if fill_rule != PadFillRule::PaletteIndexOrColor {
        return Ok(None);
    }
    if let ImageOpsColor::PaletteFill { index, palette } = fill {
        let (output_palette, alpha) = match image.palette() {
            Some(palette) => (palette, image.palette_alpha()),
            None => (palette.clone(), None),
        };
        return Ok(Some((*index, output_palette, alpha)));
    }
    let Some(components) = palette_fill_components(fill)? else {
        return Ok(None);
    };
    // Native Rust fills use index zero and derive their temporary RGB palette
    // from components. A Python-prepared PaletteFill above retains its exact
    // host-selected index and palette bytes.
    let color = resolve_palette_tuple_bytes(&components)?;
    let (palette, alpha) = match image.palette() {
        Some(palette) => (palette, image.palette_alpha()),
        None => (color.to_vec(), None),
    };
    Ok(Some((0, palette, alpha)))
}

fn resolve_python_integer_list(
    values: &[i64],
    mode: Mode,
) -> Result<(u8, u8, u8, u8), PilError> {
    if mode == Mode::ImagePalette8 && matches!(values.len(), 3 | 4) {
        return resolve_color_components(values, mode);
    }
    Err(python_list_color_error(mode))
}

fn python_list_color_error(mode: Mode) -> PilError {
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

fn floating_scalar_color(value: f64, mode: Mode) -> Result<(u8, u8, u8, u8), PilError> {
    if is_native_f32_mode(mode) {
        // NO_PILLOW_SUPPORT: CoreOnly RGB32F/RGBA32F accept native float fills.
        // The typed operation carries the actual sample; this byte tuple is
        // only the compatibility field required by the shared pipeline shape.
        return Ok((0, 0, 0, 0));
    }
    if mode == Mode::ImageF32 {
        // Pillow's F-mode getink converts through double, then stores FLOAT32;
        // the existing ImageF32 raster carrier holds those sample bytes.
        return Ok(float32_color_bytes(value));
    }
    let message = if matches!(
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
        "color must be int or single-element tuple"
    } else {
        "color must be int or tuple"
    };
    Err(PilError::TypeError(message.into()))
}

fn floating_components_color(values: &[f64], mode: Mode) -> Result<(u8, u8, u8, u8), PilError> {
    if is_native_f32_mode(mode) {
        return match values.len() {
            1 | 3 | 4 => Ok((0, 0, 0, 0)),
            _ => Err(invalid_color(mode)),
        };
    }
    if mode == Mode::ImageF32 {
        // Pillow unwraps a one-element tuple before the same F-mode conversion.
        return match values {
            [value] => Ok(float32_color_bytes(*value)),
            _ => Err(PilError::TypeError("must be real number, not tuple".into())),
        };
    }
    let message = match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImagePalette8
        | Mode::ImageI32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => "color must be int or single-element tuple",
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageLumaA16 => match values.len() {
            1 | 2 => "'float' object cannot be interpreted as an integer",
            _ => "color must be int, or tuple of one or two elements",
        },
        _ => match values.len() {
            3 | 4 => "'float' object cannot be interpreted as an integer",
            _ => "color must be int, or tuple of one, three or four elements",
        },
    };
    Err(PilError::TypeError(message.into()))
}

fn is_native_f32_mode(mode: Mode) -> bool {
    matches!(mode, Mode::ImageRgb32F | Mode::ImageRgba32F)
}

fn float32_color_bytes(value: f64) -> (u8, u8, u8, u8) {
    let [red, green, blue, alpha] = (value as f32).to_le_bytes();
    (red, green, blue, alpha)
}

fn resolve_named_color(name: &str, mode: Mode) -> Result<(u8, u8, u8, u8), PilError> {
    if is_native_f32_mode(mode) {
        // NO_PILLOW_SUPPORT: CoreOnly RGB32F/RGBA32F accept numeric fills only.
        return Err(PilError::TypeError(
            "native high-depth modes require numeric fill values".into(),
        ));
    }
    if mode == Mode::ImagePalette8 {
        // ImageColor.getcolor("...", "P") returns RGB, after which
        // Image.new builds a temporary RGB palette and stores its index.
        let (red, green, blue, _) = crate::color::parse_color_str_unclamped(name)?;
        validate_palette_rgb_components(&[
            i64::from(red),
            i64::from(green),
            i64::from(blue),
        ])?;
        return Ok((0, 0, 0, u8::MAX));
    }
    let (red, green, blue, alpha) = crate::color::parse_color_str(name)?;
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
        [red, green, blue] if mode == Mode::ImagePalette8 => {
            validate_palette_rgb_components(&[*red, *green, *blue])?;
            Ok((0, 0, 0, u8::MAX))
        }
        [red, green, blue, alpha] if mode == Mode::ImagePalette8 => {
            // Image.new("P", ..., color) builds a temporary RGB palette for
            // an integer RGB(A) tuple. ImagePalette.getcolor rejects a
            // non-opaque fourth value before converting the RGB tuple to
            // bytes, then bytes() rejects out-of-range RGB components.
            if *alpha != i64::from(u8::MAX) {
                return Err(PilError::ValueError(
                    "cannot add non-opaque RGBA color to RGB palette".into(),
                ));
            }
            validate_palette_rgb_components(&[*red, *green, *blue])?;
            Ok((0, 0, 0, u8::MAX))
        }
        [cyan, magenta, yellow, black] if mode == Mode::ImageCmyk8 => Ok((
            clamp_color_component(*cyan),
            clamp_color_component(*magenta),
            clamp_color_component(*yellow),
            clamp_color_component(*black),
        )),
        [value, alpha]
            if matches!(
                mode,
                Mode::ImageLumaA8
                    | Mode::ImageLaPremultiplied8
                    | Mode::ImageLumaA16
                    | Mode::ImagePaletteAlpha8
            ) =>
        {
            Ok((
            clamp_color_component(*value),
            clamp_color_component(*value),
            clamp_color_component(*value),
            clamp_color_component(*alpha),
            ))
        }
        [red, green, blue] | [red, green, blue, _]
            if !is_luma_mode(mode)
                && !matches!(
                    mode,
                    Mode::ImageLumaA8
                        | Mode::ImageLaPremultiplied8
                        | Mode::ImagePaletteAlpha8
                        | Mode::ImageLumaA16
                ) =>
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

fn resolve_palette_tuple_fill(
    values: &[crate::image::PaletteColorComponent],
) -> Result<(u8, u8, u8, u8), PilError> {
    resolve_palette_tuple_bytes(values)?;
    Ok((0, 0, 0, u8::MAX))
}

pub(crate) fn resolve_palette_tuple_bytes(
    values: &[crate::image::PaletteColorComponent],
) -> Result<[u8; 3], PilError> {
    let rgb = match values {
        [red, green, blue] => [red, green, blue],
        [red, green, blue, alpha] => {
            if !palette_component_matches_opaque(alpha) {
                return Err(PilError::ValueError(
                    "cannot add non-opaque RGBA color to RGB palette".into(),
                ));
            }
            [red, green, blue]
        }
        _ => return Err(PilError::TypeError("color must be int or tuple".into())),
    };
    Ok([
        palette_component_byte(rgb[0])?,
        palette_component_byte(rgb[1])?,
        palette_component_byte(rgb[2])?,
    ])
}

fn palette_component_matches_opaque(value: &crate::image::PaletteColorComponent) -> bool {
    match value {
        crate::image::PaletteColorComponent::Integer(value) => {
            *value == i64::from(u8::MAX)
        }
        crate::image::PaletteColorComponent::WideInteger { .. } => false,
        crate::image::PaletteColorComponent::Float(value) => *value == f64::from(u8::MAX),
    }
}

fn palette_component_byte(
    value: &crate::image::PaletteColorComponent,
) -> Result<u8, PilError> {
    match value {
        crate::image::PaletteColorComponent::Integer(value) => {
            u8::try_from(*value).map_err(|_| palette_byte_range_error())
        }
        crate::image::PaletteColorComponent::WideInteger { .. } => {
            Err(palette_byte_range_error())
        }
        crate::image::PaletteColorComponent::Float(_) => Err(PilError::TypeError(
            "'float' object cannot be interpreted as an integer".into(),
        )),
    }
}

fn palette_byte_range_error() -> PilError {
    PilError::ValueError("bytes must be in range(0, 256)".into())
}

fn validate_palette_rgb_components(values: &[i64; 3]) -> Result<(), PilError> {
    if values.iter().any(|value| !(0..=i64::from(u8::MAX)).contains(value)) {
        return Err(PilError::ValueError(
            "bytes must be in range(0, 256)".into(),
        ));
    }
    Ok(())
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
    if mode.pillow_pixel_size() == Some(4) {
        // Pillow's packed UINT8 ink treats an integer as ABGR, then stores
        // the resulting RGBA bytes. The mode-owned kernel consumes the lanes
        // relevant to its exact two-, three-, or four-band layout.
        let [red, green, blue, alpha] = (value as u32).to_le_bytes();
        return (red, green, blue, alpha);
    }
    let value = clamp_color_component(value);
    if is_luma_mode(mode) {
        return (value, value, value, u8::MAX);
    }
    if mode == Mode::ImagePalette8 {
        return (value, 0, 0, u8::MAX);
    }
    let alpha = if mode == Mode::ImageLumaA16 { 0 } else { u8::MAX };
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

fn invalid_color(mode: Mode) -> PilError {
    if mode == Mode::ImageF32 {
        PilError::TypeError("must be real number, not tuple".into())
    } else if is_luma_mode(mode) || mode == Mode::ImagePalette8 {
        PilError::TypeError("color must be int or single-element tuple".into())
    } else if matches!(
        mode,
        Mode::ImageLumaA8
            | Mode::ImageLaPremultiplied8
            | Mode::ImagePaletteAlpha8
            | Mode::ImageLumaA16
    ) {
        PilError::TypeError("color must be int, or tuple of one or two elements".into())
    } else {
        PilError::TypeError("color must be int, or tuple of one, three or four elements".into())
    }
}
