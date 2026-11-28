use crate::error::PilError;
use crate::image::{
    Image, ImageInfoValue, ImagingCoreTransparencyKey, PaletteColorComponent, PaletteData,
};
use crate::raster::Mode;

use super::input::{
    ConvertInputOutput, ConvertTransparencyInfoDelta, ConvertTransparencyInputKind,
    ConvertTransparencyPreflight, ConvertTransparencyValue, ConvertWarning,
};

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(super) fn transparency_value_from_info(value: ImageInfoValue) -> ConvertTransparencyValue {
    match value {
        ImageInfoValue::Integer(value) => {
            ConvertTransparencyValue::Scalar(PaletteColorComponent::Integer(value))
        }
        ImageInfoValue::Bytes(value) => ConvertTransparencyValue::Bytes(value),
        ImageInfoValue::IntegerTuple(values) => ConvertTransparencyValue::Tuple(
            values
                .into_iter()
                .map(PaletteColorComponent::Integer)
                .collect(),
        ),
        ImageInfoValue::Float(value) => {
            ConvertTransparencyValue::Scalar(PaletteColorComponent::Float(value))
        }
        ImageInfoValue::IntegerList(values) => ConvertTransparencyValue::Sequence(
            values
                .into_iter()
                .map(PaletteColorComponent::Integer)
                .collect(),
        ),
        ImageInfoValue::FloatList(values) => ConvertTransparencyValue::Sequence(
            values
                .into_iter()
                .map(PaletteColorComponent::Float)
                .collect(),
        ),
        ImageInfoValue::String(_) => ConvertTransparencyValue::Other("str".into()),
        ImageInfoValue::Object(_) => ConvertTransparencyValue::Other("dict".into()),
    }
}

impl Image {
    /// Preflights native transparency retained in core metadata for Rust callers.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_convert_transparency_from_metadata(
        &self,
        target_mode: Mode,
    ) -> Result<ConvertTransparencyPreflight, PilError> {
        let Some(value) = self.pending_transparency_info() else {
            return Ok(ConvertTransparencyPreflight::Preserve);
        };
        self.prepare_convert_transparency(target_mode, transparency_value_from_info(value), None)
    }

    /// Resolves wrapper transparency into a typed Convert preflight before execution.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn prepare_convert_transparency(
        &self,
        target_mode: Mode,
        value: ConvertTransparencyValue,
        background_index: Option<u32>,
    ) -> Result<ConvertTransparencyPreflight, PilError> {
        let source_mode = self.mode_identity()?;
        // Pillow 13 promotes only the source/target pairs declared by the
        // native ConvertTransparent operation; keep route selection tied to
        // that same support contract.
        if is_convert_transparent_alpha_route(source_mode, target_mode) {
            return transparent_alpha_preflight(value);
        }
        if is_palette_alpha_route(source_mode, target_mode) {
            return palette_alpha_preflight(value);
        }
        if !is_transparency_color_route(source_mode, target_mode) {
            return Ok(ConvertTransparencyPreflight::Preserve);
        }
        if source_mode == Mode::ImagePalette8
            && !matches!(&value, ConvertTransparencyValue::Bytes(_))
            && !self.has_attached_palette()
        {
            // Pillow asserts palette presence before scalar validation or
            // tuple palette lookup on its P-to-color-key route.
            return Err(PilError::AssertionError(String::new()));
        }
        match value {
            ConvertTransparencyValue::None => Ok(ConvertTransparencyPreflight::Preserve),
            ConvertTransparencyValue::Bytes(_) => Ok(ConvertTransparencyPreflight::RemoveByteTable),
            ConvertTransparencyValue::Scalar(value) => {
                let rgb = self.rgb_from_scalar_transparency(source_mode, &value)?;
                Ok(rgb.map_or(ConvertTransparencyPreflight::Preserve, |rgb| {
                    rgb_key(rgb, background_index, palette_index(&value))
                }))
            }
            ConvertTransparencyValue::Tuple(values) => {
                let (rgb, transparency_index) = match source_mode {
                    Mode::ImageLuma8 => (Some(gray_tuple_pixel(&values)?), None),
                    Mode::ImageRgb8 => (Some(rgb_tuple_pixel(&values)?), None),
                    Mode::ImagePalette8 => {
                        let rgb = self
                            .rgb_from_palette_tuple(&values, background_index)
                            .map_err(map_palette_transparency_error)?;
                        (rgb, None)
                    }
                    _ => (None, None),
                };
                Ok(rgb.map_or(ConvertTransparencyPreflight::Preserve, |rgb| {
                    rgb_key(rgb, background_index, transparency_index)
                }))
            }
            ConvertTransparencyValue::PaletteColor(color) => {
                if source_mode != Mode::ImagePalette8 {
                    return Err(PilError::InternalError(
                        "P palette color reached a non-P Convert route".into(),
                    ));
                }
                Ok(
                    color.map_or(ConvertTransparencyPreflight::Preserve, |color| {
                        rgb_key(color.rgb(), background_index, None)
                    }),
                )
            }
            ConvertTransparencyValue::Sequence(_) => Err(transparency_type_error(source_mode)),
            ConvertTransparencyValue::Other(_) => Err(transparency_type_error(source_mode)),
            ConvertTransparencyValue::ImagingCoreKey(_) => Err(PilError::InternalError(
                "ImagingCore transparency key reached a non-alpha Convert route".into(),
            )),
        }
    }

    /// Converts a scalar Pillow pixel key into the RGB value used by Convert.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn rgb_from_scalar_transparency(
        &self,
        source_mode: Mode,
        value: &PaletteColorComponent,
    ) -> Result<Option<[u8; 3]>, PilError> {
        if !matches!(
            value,
            PaletteColorComponent::Integer(_) | PaletteColorComponent::WideInteger { .. }
        ) {
            return Err(transparency_type_error(source_mode));
        }
        let value = component_to_c_long_long(value)?;
        let rgb = match source_mode {
            Mode::ImageLuma8 => [clip_byte(value); 3],
            Mode::ImageRgb8 => packed_rgb(value),
            Mode::ImagePalette8 => {
                let index = usize::from(clip_byte(value));
                let palette = self.palette().unwrap_or_default();
                palette_rgb_at(&palette, PaletteLayout::Rgb, index)
            }
            _ => return Ok(None),
        };
        Ok(Some(rgb))
    }

    /// Resolves a native palette tuple through the source P palette and maps
    /// the selected entry to the RGB key used by Convert.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn rgb_from_palette_tuple(
        &self,
        values: &[PaletteColorComponent],
        background_index: Option<u32>,
    ) -> Result<Option<[u8; 3]>, PilError> {
        let alpha = self.palette_alpha().unwrap_or_default();
        let palette_data = self
            .palette_data()
            .cloned()
            .unwrap_or_else(|| PaletteData::from_alpha(&alpha));
        let layout = PaletteLayout::from_data(&palette_data);
        let mode_palette =
            palette_in_layout(&self.palette().unwrap_or_default(), &alpha, &palette_data);
        let normalized = normalize_palette_color(values, layout)?;
        if let Some(index) = palette_color_index(&mode_palette, layout, &normalized) {
            return Ok(Some(palette_rgb_at(&mode_palette, layout, index)));
        }
        let color_bytes = palette_color_bytes(&normalized)?;
        let Some(index) =
            self.allocate_source_palette_index(&mode_palette, layout, background_index)?
        else {
            return Ok(None);
        };
        let reserved = palette_reserved_indices(background_index, None);
        let mut updated = mode_palette;
        pad_palette_for_reserved_slots(&mut updated, layout, &reserved);
        updated = update_palette_bytes(updated, layout, index, &color_bytes);
        Ok(Some(palette_rgb_at(&updated, layout, index)))
    }

    /// Selects a source palette index, using image pixels only when the palette
    /// has no appendable slot.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn allocate_source_palette_index(
        &self,
        palette: &[u8],
        layout: PaletteLayout,
        background_index: Option<u32>,
    ) -> Result<Option<usize>, PilError> {
        let channels = layout.channels();
        let entries = palette.len() / channels;
        let reserved = palette_reserved_indices(background_index, None);
        if let Some(index) = Image::palette_allocation_index_from_histogram(entries, &reserved, &[])
        {
            return Ok(Some(index));
        }
        let histogram = palette_histogram(self.tobytes()?);
        Ok(Image::palette_allocation_index_from_histogram(
            entries, &reserved, &histogram,
        ))
    }
}

impl Image {
    /// Finishes the output-side transparency transition after the selected Convert strategy.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(super) fn finish_convert_transparency(
        &self,
        mut image: Image,
        target_mode: Mode,
        transparency: Option<ConvertTransparencyPreflight>,
        adaptive_palette: bool,
    ) -> Result<ConvertInputOutput, PilError> {
        let Some(transparency) = transparency else {
            return Ok(convert_output(
                image,
                ConvertTransparencyInfoDelta::Preserve,
                None,
            ));
        };
        match transparency {
            ConvertTransparencyPreflight::Preserve => Ok(convert_output(
                image,
                ConvertTransparencyInfoDelta::Preserve,
                None,
            )),
            ConvertTransparencyPreflight::RemoveByteTable => Ok(convert_output(
                image,
                ConvertTransparencyInfoDelta::Remove,
                None,
            )),
            ConvertTransparencyPreflight::PaletteAlphaTable(_)
            | ConvertTransparencyPreflight::PaletteAlphaIndex(_) => Ok(convert_output(
                image,
                ConvertTransparencyInfoDelta::Remove,
                None,
            )),
            ConvertTransparencyPreflight::TransparentAlpha { .. } => Err(PilError::InternalError(
                "transparent-alpha Convert bypassed its prepared execution".into(),
            )),
            ConvertTransparencyPreflight::RgbKey {
                rgb,
                background_index,
                transparency_index,
                host_palette_callback,
            } => match target_mode {
                Mode::ImageLuma8 => Ok(convert_output(
                    image,
                    ConvertTransparencyInfoDelta::Set(ImageInfoValue::Integer(i64::from(
                        crate::color::rgb_to_luma_u8(rgb[0], rgb[1], rgb[2]),
                    ))),
                    None,
                )),
                Mode::ImageRgb8 => Ok(convert_output(
                    image,
                    ConvertTransparencyInfoDelta::Set(ImageInfoValue::IntegerTuple(
                        rgb.into_iter().map(i64::from).collect(),
                    )),
                    None,
                )),
                Mode::ImagePalette8 => {
                    if host_palette_callback {
                        return Ok(convert_output(
                            image,
                            ConvertTransparencyInfoDelta::SetPaletteColor {
                                rgb,
                                adaptive_palette,
                            },
                            None,
                        ));
                    }
                    let reserved = palette_reserved_indices(background_index, transparency_index);
                    match put_palette_transparency_color(&mut image, rgb, &reserved) {
                        Ok(Some(index)) => Ok(convert_output(
                            image,
                            ConvertTransparencyInfoDelta::Set(ImageInfoValue::Integer(i64::from(
                                index,
                            ))),
                            None,
                        )),
                        Ok(None) if adaptive_palette => Ok(convert_output(
                            image,
                            ConvertTransparencyInfoDelta::Remove,
                            Some(ConvertWarning::TransparencyPaletteAllocation),
                        )),
                        Ok(None) => Ok(convert_output(
                            image,
                            ConvertTransparencyInfoDelta::Remove,
                            None,
                        )),
                        Err(_) if adaptive_palette => Ok(convert_output(
                            image,
                            ConvertTransparencyInfoDelta::Remove,
                            Some(ConvertWarning::TransparencyPaletteAllocation),
                        )),
                        Err(PilError::ValueError(message)) => Ok(convert_output(
                            image,
                            ConvertTransparencyInfoDelta::Remove,
                            (message != "cannot allocate more than 256 colors")
                                .then_some(ConvertWarning::TransparencyPaletteAllocation),
                        )),
                        Err(error) => Err(error),
                    }
                }
                _ => Ok(convert_output(
                    image,
                    ConvertTransparencyInfoDelta::Preserve,
                    None,
                )),
            },
        }
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(super) fn is_convert_transparent_alpha_route(source: Mode, target: Mode) -> bool {
    crate::compute::pool_cpu::ops::color::ops::CONVERT_TRANSPARENT_SUPPORT
        .scope_for_target(source, target)
        .is_some()
}

/// Selects the host value parser from the same staged source/target routes.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(super) fn convert_transparency_input_kind(
    source: Mode,
    target: Mode,
) -> Option<ConvertTransparencyInputKind> {
    if is_convert_transparent_alpha_route(source, target) {
        Some(ConvertTransparencyInputKind::ImagingCoreKey)
    } else if is_palette_alpha_route(source, target) {
        Some(ConvertTransparencyInputKind::PaletteAlpha)
    } else if is_transparency_color_route(source, target) {
        match source {
            Mode::ImageLuma8 => Some(ConvertTransparencyInputKind::LumaPixelColor),
            Mode::ImageRgb8 => Some(ConvertTransparencyInputKind::RgbPixelColor),
            Mode::ImagePalette8 => Some(ConvertTransparencyInputKind::PaletteColor),
            _ => None,
        }
    } else {
        None
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn transparent_alpha_preflight(
    value: ConvertTransparencyValue,
) -> Result<ConvertTransparencyPreflight, PilError> {
    // Preserve scalar versus RGB-sequence form: scalar keys expand only for
    // gray source modes inside the selected ConvertTransparent strategy.
    let key = match value {
        ConvertTransparencyValue::ImagingCoreKey(key) => key,
        ConvertTransparencyValue::Scalar(value) => {
            ImagingCoreTransparencyKey::Scalar(component_to_c_int(&value)?)
        }
        ConvertTransparencyValue::Bytes(values) => {
            rgb_transparency_key(bytes_transparency_key(&values)?)
        }
        ConvertTransparencyValue::Tuple(values) => {
            rgb_transparency_key(tuple_transparency_key(&values)?)
        }
        ConvertTransparencyValue::Sequence(values) => {
            rgb_transparency_key(tuple_transparency_key(&values)?)
        }
        ConvertTransparencyValue::PaletteColor(_) => {
            return Err(PilError::InternalError(
                "P palette color reached transparent-alpha conversion".into(),
            ));
        }
        ConvertTransparencyValue::None => {
            return Err(PilError::TypeError(
                "'NoneType' object cannot be interpreted as an integer".into(),
            ));
        }
        ConvertTransparencyValue::Other(type_name) => {
            return Err(PilError::TypeError(format!(
                "'{type_name}' object cannot be interpreted as an integer"
            )));
        }
    };
    Ok(ConvertTransparencyPreflight::TransparentAlpha { key })
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn tuple_transparency_key(values: &[PaletteColorComponent]) -> Result<[i32; 3], PilError> {
    if values.len() != 3 {
        return Err(PilError::TypeError(
            "transparency must contain exactly three integer components".into(),
        ));
    }
    Ok([
        component_to_c_int(&values[0])?,
        component_to_c_int(&values[1])?,
        component_to_c_int(&values[2])?,
    ])
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn bytes_transparency_key(values: &[u8]) -> Result<[i32; 3], PilError> {
    if values.len() != 3 {
        return Err(PilError::TypeError(
            "transparency must contain exactly three integer components".into(),
        ));
    }
    Ok([
        i32::from(values[0]),
        i32::from(values[1]),
        i32::from(values[2]),
    ])
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn rgb_transparency_key([red, green, blue]: [i32; 3]) -> ImagingCoreTransparencyKey {
    ImagingCoreTransparencyKey::Rgb(red, green, blue)
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn is_palette_alpha_route(source: Mode, target: Mode) -> bool {
    source == Mode::ImagePalette8
        && matches!(
            target,
            Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 | Mode::ImageRgba8
        )
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_alpha_preflight(
    value: ConvertTransparencyValue,
) -> Result<ConvertTransparencyPreflight, PilError> {
    match value {
        ConvertTransparencyValue::Bytes(values) => {
            Ok(ConvertTransparencyPreflight::PaletteAlphaTable(values))
        }
        ConvertTransparencyValue::Scalar(PaletteColorComponent::Integer(index)) => {
            let index = i32::try_from(index).map_err(|_| {
                PilError::OverflowError("Python int too large to convert to C int".into())
            })?;
            Ok(ConvertTransparencyPreflight::PaletteAlphaIndex(index))
        }
        ConvertTransparencyValue::Scalar(PaletteColorComponent::WideInteger { .. }) => Err(
            PilError::OverflowError("Python int too large to convert to C int".into()),
        ),
        ConvertTransparencyValue::Scalar(PaletteColorComponent::Float(_)) => Err(
            PilError::ValueError("Transparency for P mode should be bytes or int".into()),
        ),
        ConvertTransparencyValue::None
        | ConvertTransparencyValue::Tuple(_)
        | ConvertTransparencyValue::Sequence(_)
        | ConvertTransparencyValue::PaletteColor(_)
        | ConvertTransparencyValue::Other(_)
        | ConvertTransparencyValue::ImagingCoreKey(_) => Err(PilError::ValueError(
            "Transparency for P mode should be bytes or int".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn is_transparency_color_route(source: Mode, target: Mode) -> bool {
    matches!(
        source,
        Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImagePalette8
    ) && matches!(
        target,
        Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImagePalette8
    )
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn rgb_key(
    rgb: [u8; 3],
    background_index: Option<u32>,
    transparency_index: Option<u32>,
) -> ConvertTransparencyPreflight {
    ConvertTransparencyPreflight::RgbKey {
        rgb,
        background_index,
        transparency_index,
        host_palette_callback: false,
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn convert_output(
    image: Image,
    transparency_info: ConvertTransparencyInfoDelta,
    warning: Option<ConvertWarning>,
) -> ConvertInputOutput {
    ConvertInputOutput {
        image,
        transparency_info,
        warning,
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn transparency_type_error(mode: Mode) -> PilError {
    match mode {
        Mode::ImageRgb8 => PilError::TypeError("color must be int or tuple".into()),
        _ => PilError::TypeError("color must be int or single-element tuple".into()),
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn component_to_c_long_long(value: &PaletteColorComponent) -> Result<i64, PilError> {
    match value {
        PaletteColorComponent::Integer(value) => Ok(*value),
        PaletteColorComponent::WideInteger { .. } => Err(PilError::OverflowError(
            "Python int too large to convert to C long long".into(),
        )),
        PaletteColorComponent::Float(_) => Err(PilError::TypeError(
            "'float' object cannot be interpreted as an integer".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn clip_byte(value: i64) -> u8 {
    value.clamp(0, i64::from(u8::MAX)) as u8
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn packed_rgb(value: i64) -> [u8; 3] {
    let value = value as u64;
    [value as u8, (value >> 8) as u8, (value >> 16) as u8]
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn gray_tuple_pixel(values: &[PaletteColorComponent]) -> Result<[u8; 3], PilError> {
    if values.len() != 1 {
        return Err(PilError::TypeError(
            "color must be int or single-element tuple".into(),
        ));
    }
    let value = clip_byte(component_to_c_long_long(&values[0])?);
    Ok([value; 3])
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn rgb_tuple_pixel(values: &[PaletteColorComponent]) -> Result<[u8; 3], PilError> {
    if values.len() == 1 {
        return component_to_c_long_long(&values[0]).map(packed_rgb);
    }
    if !matches!(values.len(), 3 | 4) {
        return Err(PilError::TypeError(
            "color must be int, or tuple of one, three or four elements".into(),
        ));
    }
    let red = clip_byte(component_to_c_long_long(&values[0])?);
    let green = clip_byte(i64::from(component_to_c_int(&values[1])?));
    let blue = clip_byte(i64::from(component_to_c_int(&values[2])?));
    if let Some(alpha) = values.get(3) {
        let _ = component_to_c_int(alpha)?;
    }
    Ok([red, green, blue])
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn component_to_c_int(value: &PaletteColorComponent) -> Result<i32, PilError> {
    let value = component_to_c_long_long(value)?;
    i32::try_from(value)
        .map_err(|_| PilError::OverflowError("Python int too large to convert to C int".into()))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn normalize_palette_color(
    values: &[PaletteColorComponent],
    layout: PaletteLayout,
) -> Result<Vec<PaletteColorComponent>, PilError> {
    let mut color = values.to_vec();
    if layout == PaletteLayout::Rgb && color.len() == 4 {
        if !component_equals_byte(&color[3], 255) {
            return Err(PilError::ValueError(
                "cannot add non-opaque RGBA color to RGB palette".into(),
            ));
        }
        color.truncate(3);
    } else if layout == PaletteLayout::Rgba && color.len() == 3 {
        color.push(PaletteColorComponent::Integer(255));
    }
    Ok(color)
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_in_layout(rgb: &[u8], alpha: &[u8], data: &PaletteData) -> Vec<u8> {
    match data {
        PaletteData::Rgb => rgb.to_vec(),
        PaletteData::Cmyk(entries) => entries.clone(),
        PaletteData::Rgba => {
            let mut palette = Vec::with_capacity(rgb.len() / 3 * 4);
            for (index, color) in rgb.chunks(3).enumerate() {
                if color.len() != 3 {
                    break;
                }
                palette.extend_from_slice(color);
                palette.push(alpha.get(index).copied().unwrap_or(255));
            }
            palette
        }
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_color_index(
    palette: &[u8],
    layout: PaletteLayout,
    color: &[PaletteColorComponent],
) -> Option<usize> {
    let channels = layout.channels();
    palette
        .chunks(channels)
        .enumerate()
        .find_map(|(index, entry)| {
            (entry.len() == color.len()
                && entry
                    .iter()
                    .zip(color)
                    .all(|(byte, component)| component_equals_byte(component, *byte)))
            .then_some(index)
        })
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn component_equals_byte(component: &PaletteColorComponent, value: u8) -> bool {
    match component {
        PaletteColorComponent::Integer(component) => *component == i64::from(value),
        PaletteColorComponent::Float(component) => *component == f64::from(value),
        PaletteColorComponent::WideInteger {
            negative,
            magnitude,
        } => !negative && (magnitude.is_empty() && value == 0 || magnitude.as_slice() == [value]),
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_color_bytes(color: &[PaletteColorComponent]) -> Result<Vec<u8>, PilError> {
    color
        .iter()
        .map(|component| match component {
            PaletteColorComponent::Integer(value) if (0..=255).contains(value) => Ok(*value as u8),
            PaletteColorComponent::WideInteger { .. } => Err(PilError::ValueError(
                "bytes must be in range(0, 256)".into(),
            )),
            PaletteColorComponent::Integer(_) => Err(PilError::ValueError(
                "bytes must be in range(0, 256)".into(),
            )),
            PaletteColorComponent::Float(_) => Err(PilError::TypeError(
                "'float' object cannot be interpreted as an integer".into(),
            )),
        })
        .collect()
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
#[derive(Clone, Copy, PartialEq, Eq)]
enum PaletteLayout {
    Rgb,
    Rgba,
    Cmyk,
}

impl PaletteLayout {
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn from_data(data: &PaletteData) -> Self {
        match data {
            PaletteData::Rgb => Self::Rgb,
            PaletteData::Rgba => Self::Rgba,
            PaletteData::Cmyk(_) => Self::Cmyk,
        }
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    const fn channels(self) -> usize {
        match self {
            Self::Rgb => 3,
            Self::Rgba | Self::Cmyk => 4,
        }
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn map_palette_transparency_error(error: PilError) -> PilError {
    match error {
        PilError::ValueError(_) => {
            PilError::ValueError("Couldn't allocate a palette color for transparency".to_owned())
        }
        error => error,
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn update_palette_bytes(
    mut palette: Vec<u8>,
    layout: PaletteLayout,
    index: usize,
    color: &[u8],
) -> Vec<u8> {
    let offset = index.saturating_mul(layout.channels());
    if offset < palette.len() {
        let end = offset.saturating_add(layout.channels()).min(palette.len());
        palette.splice(offset..end, color.iter().copied());
    } else {
        palette.resize(offset, 0);
        palette.extend_from_slice(color);
    }
    palette
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn pad_palette_for_reserved_slots(palette: &mut Vec<u8>, layout: PaletteLayout, reserved: &[u32]) {
    let channels = layout.channels();
    let mut entries = palette.len() / channels;
    while entries < 256 && u32::try_from(entries).is_ok_and(|index| reserved.contains(&index)) {
        entries += 1;
        palette.resize(entries * channels, 0);
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_rgb_at(palette: &[u8], layout: PaletteLayout, index: usize) -> [u8; 3] {
    let base = index.saturating_mul(layout.channels());
    let color = [
        palette.get(base).copied().unwrap_or(0),
        palette.get(base + 1).copied().unwrap_or(0),
        palette.get(base + 2).copied().unwrap_or(0),
    ];
    if layout == PaletteLayout::Cmyk {
        crate::color::cmyk_pixel_to_rgb([
            color[0],
            color[1],
            color[2],
            palette.get(base + 3).copied().unwrap_or(0),
        ])
    } else {
        color
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_index(value: &PaletteColorComponent) -> Option<u32> {
    match value {
        PaletteColorComponent::Integer(value) => u32::try_from(*value).ok(),
        PaletteColorComponent::Float(value)
            if value.is_finite()
                && value.fract() == 0.0
                && *value >= 0.0
                && *value <= u32::MAX as f64 =>
        {
            Some(*value as u32)
        }
        PaletteColorComponent::WideInteger {
            negative,
            magnitude,
        } if !negative && magnitude.len() <= 4 => {
            let mut value = 0_u32;
            for byte in magnitude {
                value = (value << 8) | u32::from(*byte);
            }
            Some(value)
        }
        _ => None,
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_reserved_indices(
    background_index: Option<u32>,
    transparency_index: Option<u32>,
) -> Vec<u32> {
    [background_index, transparency_index]
        .into_iter()
        .flatten()
        .collect()
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_histogram(samples: Vec<u8>) -> Vec<u64> {
    let mut histogram = vec![0_u64; 256];
    for sample in samples {
        histogram[usize::from(sample)] += 1;
    }
    histogram
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn put_palette_transparency_color(
    image: &mut Image,
    rgb: [u8; 3],
    reserved: &[u32],
) -> Result<Option<u8>, PilError> {
    let mut palette = image.palette().unwrap_or_default();
    if let Some(index) = palette
        .chunks(3)
        .position(|entry| entry == rgb)
        .and_then(|index| u8::try_from(index).ok())
    {
        return Ok(Some(index));
    }
    let entries = palette.len() / 3;
    let index = match Image::palette_allocation_index_from_histogram(entries, reserved, &[]) {
        Some(index) => Some(index),
        None => {
            let histogram = palette_histogram(image.tobytes()?);
            Image::palette_allocation_index_from_histogram(entries, reserved, &histogram)
        }
    };
    let Some(index) = index.and_then(|index| u8::try_from(index).ok()) else {
        return Ok(None);
    };
    pad_palette_for_reserved_slots(&mut palette, PaletteLayout::Rgb, reserved);
    let offset = usize::from(index) * 3;
    if offset < palette.len() {
        let end = (offset + 3).min(palette.len());
        palette.splice(offset..end, rgb);
    } else {
        palette.resize(offset, 0);
        palette.extend_from_slice(&rgb);
    }
    image.replace_indexed_palette(&palette, "RGB")?;
    Ok(Some(index))
}
