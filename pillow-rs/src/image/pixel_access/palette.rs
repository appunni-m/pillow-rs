//! Palette-indexed pixel color allocation and access.

use crate::error::PilError;
use crate::image::{PaletteData, PaletteModel};

use super::super::Image;
use super::palette_components::{
    expand_palette_metadata, palette_component_matches_byte, palette_component_to_byte,
    palette_components_equal,
};
use super::{PaletteColorComponent, PutPixelValue};

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
pub(super) fn write_palette_entry(
    palette: &mut Vec<u8>,
    channels: usize,
    index: usize,
    color: &[u8],
) {
    // Pillow 13.0 ImagePalette._new_color_index fills reserved slots with
    // zero-valued native entries before writing the selected color.
    let start = index * channels;
    if start > palette.len() {
        palette.resize(start, 0);
    }
    let end = (start + channels).min(palette.len());
    palette.splice(start..end, color.iter().copied());
}
impl Image {
    /// Adds a native four-channel color to a CMYK palette and returns its index.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(super) fn resolve_cmyk_palette_color(
        &mut self,
        values: &[i64],
        reserved: &[u32],
    ) -> Result<u8, PilError> {
        if values.len() != 4 {
            return Err(PilError::ValueError(
                "CMYK palette colors require four components".into(),
            ));
        }
        let Some(PaletteData::Cmyk(mut palette)) = self.palette_data().cloned() else {
            return Err(PilError::InternalError(
                "CMYK palette mode has no native CMYK entries".into(),
            ));
        };
        if let Some(index) = palette.chunks_exact(4).position(|entry| {
            entry
                .iter()
                .zip(values)
                .all(|(&byte, &value)| i64::from(byte) == value)
        }) {
            return Ok(index as u8);
        }

        let entries = palette.len() / 4;
        let index = self.palette_allocation_index(entries, reserved, true)?;
        let color = values
            .iter()
            .map(|&value| {
                u8::try_from(value)
                    .map_err(|_| PilError::ValueError("bytes must be in range(0, 256)".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        write_palette_entry(&mut palette, 4, index, &color);
        self.replace_indexed_palette(&palette, "CMYK")?;
        Ok(index as u8)
    }

    /// Finds a CMYK palette color using Python's numeric tuple equality rules.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(super) fn palette_float_match_cmyk_index(&self, color: &[f64]) -> Option<usize> {
        let PaletteData::Cmyk(palette) = self.palette_data()? else {
            return None;
        };
        palette.chunks_exact(4).position(|entry| {
            entry.len() == color.len()
                && entry
                    .iter()
                    .zip(color)
                    .all(|(&byte, &value)| f64::from(byte) == value)
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(super) fn palette_allocation_index(
        &self,
        entries: usize,
        reserved: &[u32],
        reuse_unused: bool,
    ) -> Result<usize, PilError> {
        // TODO(pillow-rs#3ptb): freeze P/PA parity for reserved metadata
        // indices in short palettes and full-palette reuse, including when
        // public Python `Image.info` overrides a retained palette-alpha marker.
        // Pillow skips `background`/`transparency`; `write_palette_entry`
        // zero-fills any skipped entries before writing at the selected slot.
        if let Some(index) = Self::next_palette_index(entries, reserved) {
            return Ok(index);
        }
        if !reuse_unused {
            return Err(PilError::ValueError(
                "cannot allocate more than 256 colors".into(),
            ));
        }
        let mut used = [false; 256];
        for index in self.tobytes()? {
            used[usize::from(index)] = true;
        }
        Self::highest_unused_palette_index(reserved, used.len(), |index| used[index])
            .ok_or_else(|| PilError::ValueError("cannot allocate more than 256 colors".into()))
    }

    /// Selects an ImagePalette allocation slot from its entry count and image
    /// histogram. Python keeps palette mutation and the public error cause;
    /// this method owns the slot-selection algorithm for the base Image path.
    #[doc(hidden)]
    #[must_use]
    pub fn palette_allocation_index_from_histogram(
        entries: usize,
        reserved: &[u32],
        histogram: &[u64],
    ) -> Option<usize> {
        if let Some(index) = Self::next_palette_index(entries, reserved) {
            return Some(index);
        }
        Self::highest_unused_palette_index(reserved, histogram.len(), |index| histogram[index] != 0)
    }

    fn next_palette_index(entries: usize, reserved: &[u32]) -> Option<usize> {
        let mut next = entries;
        while next < 256 && Self::palette_index_is_reserved(next, reserved) {
            next += 1;
        }
        (next < 256).then_some(next)
    }

    fn highest_unused_palette_index(
        reserved: &[u32],
        histogram_len: usize,
        mut is_used: impl FnMut(usize) -> bool,
    ) -> Option<usize> {
        (0..histogram_len)
            .rev()
            .find(|&index| !is_used(index) && !Self::palette_index_is_reserved(index, reserved))
    }

    fn palette_index_is_reserved(index: usize, reserved: &[u32]) -> bool {
        u32::try_from(index).is_ok_and(|index| reserved.contains(&index))
    }

    /// Resolves an RGB(A) palette color before coordinate validation.
    ///
    /// Pillow allocates the palette entry even when the subsequent pixel
    /// coordinate fails. P returns its index; PA retains the separate alpha.
    /// Other values pass through unchanged.
    ///
    /// # Errors
    ///
    /// Returns a palette allocation or component-range error.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn prepare_putpixel_palette(
        &mut self,
        value: PutPixelValue,
        reserved: &[u32],
    ) -> Result<PutPixelValue, PilError> {
        if !self.has_palette_samples() {
            return Ok(value);
        }
        match value {
            PutPixelValue::FloatComponents(values) => {
                self.prepare_palette_float_components(&values, reserved)
            }
            PutPixelValue::Components(values) if matches!(values.len(), 3 | 4) => {
                self.prepare_palette_integer_components(&values, reserved)
            }
            value => Ok(value),
        }
    }

    /// Resolves a P-mode ImageOps color through its cloned palette.
    ///
    /// This reproduces ImagePalette.getcolor's append-only slot selection:
    /// existing entries retain their first index, while new entries append at
    /// the current end and fail once the palette already has 256 entries.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(crate) fn resolve_palette_fill_color(
        &self,
        components: &[PaletteColorComponent],
    ) -> Result<(u8, Vec<u8>, Option<Vec<u8>>), PilError> {
        let rgba = self.palette_model() == Some(PaletteModel::Rgba);
        let mut color = components.to_vec();
        if !rgba && color.len() == 4 {
            if !palette_component_matches_byte(&color[3], u8::MAX) {
                return Err(PilError::ValueError(
                    "cannot add non-opaque RGBA color to RGB palette".into(),
                ));
            }
            color.truncate(3);
        } else if rgba && color.len() == 3 {
            color.push(PaletteColorComponent::Integer(i64::from(u8::MAX)));
        }

        let channels = if rgba { 4 } else { 3 };
        if color.len() != channels {
            return Err(PilError::TypeError("unknown color specifier".into()));
        }
        let palette = if rgba {
            self.getpalette_rgba()
        } else {
            self.extract_palette()
        }
        .unwrap_or_default();
        if let Some(index) = palette.chunks_exact(channels).position(|entry| {
            entry
                .iter()
                .zip(&color)
                .all(|(&byte, component)| palette_component_matches_byte(component, byte))
        }) {
            let (rgb, alpha) = expand_palette_metadata(palette, rgba);
            return Ok((index as u8, rgb, alpha));
        }

        let index = palette.len() / channels;
        if index >= 256 {
            return Err(PilError::ValueError(
                "cannot allocate more than 256 colors".into(),
            ));
        }
        let color = color
            .iter()
            .map(palette_component_to_byte)
            .collect::<Result<Vec<_>, _>>()?;
        let start = index * channels;
        let end = (start + channels).min(palette.len());
        let mut updated = Vec::with_capacity(palette.len() + channels);
        updated.extend_from_slice(&palette[..start]);
        updated.extend_from_slice(&color);
        updated.extend_from_slice(&palette[end..]);
        let (rgb, alpha) = expand_palette_metadata(updated, rgba);
        Ok((index as u8, rgb, alpha))
    }

    /// Resolves numeric Draw tuple components through Pillow's P palette.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(crate) fn prepare_draw_palette_tuple_components(
        &mut self,
        components: &[PaletteColorComponent],
        reserved: &[u32],
        color_cache: &mut Vec<(Vec<PaletteColorComponent>, u8)>,
    ) -> Result<PutPixelValue, PilError> {
        let palette_model = self.palette_model().unwrap_or(PaletteModel::Rgb);
        let cmyk_palette = palette_model == PaletteModel::Cmyk;
        let rgba_palette = palette_model == PaletteModel::Rgba;
        let color = Self::normalize_draw_palette_color(components, rgba_palette, cmyk_palette)?;

        if let Some((_, index)) = color_cache
            .iter()
            .find(|(cached, _)| palette_components_equal(cached, &color))
        {
            return Ok(PutPixelValue::Integer(i64::from(*index)));
        }

        let lookup_color: Vec<f64> = color
            .iter()
            .map(|component| match component {
                PaletteColorComponent::Integer(value) => *value as f64,
                PaletteColorComponent::WideInteger { .. } => f64::NAN,
                PaletteColorComponent::Float(value) => *value,
            })
            .collect();
        let matching_index = if cmyk_palette {
            self.palette_float_match_cmyk_index(&lookup_color)
        } else {
            self.palette_float_match_index(&lookup_color, rgba_palette)
        };
        if let Some(index) = matching_index {
            color_cache.push((color, index as u8));
            return Ok(PutPixelValue::Integer(index as i64));
        }

        let index = self.draw_palette_allocation_index(reserved)?;
        // Pillow inserts the ImagePalette.colors key after choosing a slot but
        // before `bytes(color)` validates component types and ranges. Retain
        // that mutation even when the conversion below returns an error.
        color_cache.push((color.clone(), index as u8));

        let mut integer_color = Vec::with_capacity(color.len());
        for component in &color {
            match component {
                PaletteColorComponent::Integer(value) if (0..=255).contains(value) => {
                    integer_color.push(*value);
                }
                PaletteColorComponent::Integer(_) | PaletteColorComponent::WideInteger { .. } => {
                    return Err(PilError::ValueError(
                        "bytes must be in range(0, 256)".into(),
                    ));
                }
                PaletteColorComponent::Float(_) => {
                    return Err(PilError::TypeError(
                        "'float' object cannot be interpreted as an integer".into(),
                    ));
                }
            }
        }

        let channels = if rgba_palette || cmyk_palette { 4 } else { 3 };
        if integer_color.len() == channels {
            return self.prepare_palette_integer_components(&integer_color, reserved);
        }

        if integer_color.is_empty() {
            return Ok(PutPixelValue::Integer(index as i64));
        }

        self.store_draw_palette_color(index, channels, &integer_color, palette_model)?;
        Ok(PutPixelValue::Integer(index as i64))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn normalize_draw_palette_color(
        components: &[PaletteColorComponent],
        rgba_palette: bool,
        cmyk_palette: bool,
    ) -> Result<Vec<PaletteColorComponent>, PilError> {
        let mut color = components.to_vec();
        if rgba_palette && color.len() == 3 {
            color.push(PaletteColorComponent::Integer(255));
        } else if !rgba_palette && !cmyk_palette && color.len() == 4 {
            let opaque = match &color[3] {
                PaletteColorComponent::Integer(value) => *value == 255,
                PaletteColorComponent::Float(value) => *value == 255.0,
                PaletteColorComponent::WideInteger { .. } => false,
            };
            if !opaque {
                return Err(PilError::ValueError(
                    "cannot add non-opaque RGBA color to RGB palette".into(),
                ));
            }
            color.truncate(3);
        }
        // Pillow's ImagePalette.getcolor normalizes only RGB and RGBA tuples.
        // CMYK uses four bytes to choose a slot but writes bytes(color) as
        // supplied, including non-four-item tuples that alter the raw buffer.
        Ok(color)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn store_draw_palette_color(
        &mut self,
        index: usize,
        channels: usize,
        color: &[i64],
        palette_model: PaletteModel,
    ) -> Result<(), PilError> {
        let cmyk_palette = palette_model == PaletteModel::Cmyk;
        let rgba_palette = palette_model == PaletteModel::Rgba;
        let mut palette = if cmyk_palette {
            self.palette_data()
                .and_then(PaletteData::cmyk_entries)
                .map(<[u8]>::to_vec)
        } else if rgba_palette {
            self.getpalette_rgba()
        } else {
            self.extract_palette()
        }
        .unwrap_or_default();
        let start = index * channels;
        if start < palette.len() {
            let end = (start + channels).min(palette.len());
            palette.splice(start..end, color.iter().map(|&value| value as u8));
        } else {
            palette.extend(color.iter().map(|&value| value as u8));
        }
        // Pillow's public Python ImagePalette retains arbitrary trailing bytes
        // in its own buffer. This core path installs only complete palette
        // entries used by indexed pixels; the wrapper keeps its buffer intact.
        self.replace_indexed_palette_from_draw(&palette, palette_model.pillow_name())
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(super) fn draw_palette_allocation_index(
        &self,
        reserved: &[u32],
    ) -> Result<usize, PilError> {
        let palette_model = self.palette_model().unwrap_or(PaletteModel::Rgb);
        let cmyk_palette = palette_model == PaletteModel::Cmyk;
        let rgba_palette = palette_model == PaletteModel::Rgba;
        let channels = if rgba_palette || cmyk_palette { 4 } else { 3 };
        let palette_len = if cmyk_palette {
            self.palette_data()
                .and_then(PaletteData::cmyk_entries)
                .map(<[u8]>::len)
        } else if rgba_palette {
            self.getpalette_rgba().map(|palette| palette.len())
        } else {
            self.extract_palette().map(|palette| palette.len())
        };
        self.palette_allocation_index(palette_len.unwrap_or_default() / channels, reserved, true)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn palette_float_match_index(&self, color: &[f64], rgba_palette: bool) -> Option<usize> {
        let palette = if rgba_palette {
            self.getpalette_rgba()
        } else {
            self.extract_palette()
        }
        .unwrap_or_default();
        palette
            .chunks_exact(if rgba_palette { 4 } else { 3 })
            .position(|entry| {
                entry.len() == color.len()
                    && entry
                        .iter()
                        .zip(color)
                        .all(|(&byte, &value)| f64::from(byte) == value)
            })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn normalize_palette_float_color(&self, values: &[f64]) -> Result<(Vec<f64>, bool), PilError> {
        let rgba_palette = self.palette_model() == Some(PaletteModel::Rgba);
        let mut color = values.to_vec();
        if rgba_palette && color.len() == 3 {
            color.push(255.0);
        } else if !rgba_palette && color.len() == 4 {
            if color[3] != 255.0 {
                return Err(PilError::ValueError(
                    "cannot add non-opaque RGBA color to RGB palette".into(),
                ));
            }
            color.truncate(3);
        }
        Ok((color, rgba_palette))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn prepare_palette_float_components(
        &self,
        values: &[f64],
        reserved: &[u32],
    ) -> Result<PutPixelValue, PilError> {
        // NO_PILLOW_SUPPORT: this typed Rust input path cannot mutate the
        // Python ImagePalette.colors cache. Public Python list/tuple writes
        // resolve through ImagePalette.getcolor before reaching this helper.
        // TODO(pillow-rs#3ptb): verify the public wrapper's cache side effects
        // for float colors and noncanonical tuple arities in frozen parity.
        let pa = self.known_mode_identity() == Some(crate::raster::Mode::ImagePaletteAlpha8);
        let cmyk_palette = !pa && self.palette_model() == Some(PaletteModel::Cmyk);
        let (color, rgba) = if cmyk_palette {
            (values.to_vec(), false)
        } else {
            self.normalize_palette_float_color(values)?
        };
        let palette = if cmyk_palette {
            self.palette_data()
                .and_then(PaletteData::cmyk_entries)
                .map(<[u8]>::to_vec)
        } else if rgba {
            self.getpalette_rgba()
        } else {
            self.extract_palette()
        }
        .unwrap_or_default();
        let matching_index = if cmyk_palette {
            self.palette_float_match_cmyk_index(&color)
        } else {
            self.palette_float_match_index(&color, rgba)
        };
        if let Some(index) = matching_index {
            return Ok(if pa {
                PutPixelValue::Components(vec![index as i64, 255])
            } else {
                PutPixelValue::Integer(index as i64)
            });
        }
        let channels = if rgba || cmyk_palette { 4 } else { 3 };
        if pa {
            self.palette_allocation_index(palette.len() / channels, reserved, false)?;
        } else {
            self.ensure_draw_palette_entry_available(reserved)?;
        }
        Err(PilError::TypeError(
            "'float' object cannot be interpreted as an integer".into(),
        ))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn prepare_palette_integer_components(
        &mut self,
        values: &[i64],
        reserved: &[u32],
    ) -> Result<PutPixelValue, PilError> {
        let pa = self.known_mode_identity() == Some(crate::raster::Mode::ImagePaletteAlpha8);
        if !pa && self.palette_model() == Some(PaletteModel::Cmyk) {
            // This typed core route handles one native four-byte CMYK color.
            // The Python Image wrapper resolves public three/four-item tuples
            // through ImagePalette.getcolor before calling ImagingCore.
            let index = self.resolve_cmyk_palette_color(values, reserved)?;
            return Ok(PutPixelValue::Integer(i64::from(index)));
        }
        let alpha = values.get(3).copied().unwrap_or(255);
        let rgba_palette = self.palette_model() == Some(PaletteModel::Rgba);
        if !pa && !rgba_palette && alpha != 255 {
            return Err(PilError::ValueError(
                "cannot add non-opaque RGBA color to RGB palette".into(),
            ));
        }
        if pa {
            let palette = if rgba_palette {
                self.getpalette_rgba()
            } else {
                self.extract_palette()
            }
            .unwrap_or_default();
            let channels = if rgba_palette { 4 } else { 3 };
            if palette.len() / channels >= 256 {
                // Allocation precedes bytes(color). A full PA palette cannot
                // allocate through Pillow's histogram path, even if a later
                // component conversion would fail for another reason.
                if let Some(index) = palette.chunks_exact(channels).position(|entry| {
                    entry[..3]
                        .iter()
                        .zip(&values[..3])
                        .all(|(&byte, &value)| i64::from(byte) == value)
                        && (!rgba_palette || entry[3] == 255)
                }) {
                    return Ok(PutPixelValue::Components(vec![index as i64, alpha]));
                }
                return Err(PilError::ValueError(
                    "cannot allocate more than 256 colors".into(),
                ));
            }
        }
        if !pa
            && (values[..3].iter().any(|&value| !(0..=255).contains(&value))
                || (rgba_palette
                    && values
                        .get(3)
                        .is_some_and(|&value| !(0..=255).contains(&value))))
        {
            // Pillow selects an available palette index before `bytes(color)`
            // validates component ranges. If the palette cannot allocate,
            // that failure takes precedence over the byte-range error.
            self.ensure_draw_palette_entry_available(reserved)?;
        }
        let mut color = [0u8; 3];
        for (byte, &value) in color.iter_mut().zip(&values[..3]) {
            *byte = u8::try_from(value)
                .map_err(|_| PilError::ValueError("bytes must be in range(0, 256)".into()))?;
        }
        let index = if rgba_palette {
            let palette_alpha = if pa {
                255
            } else {
                u8::try_from(alpha)
                    .map_err(|_| PilError::ValueError("bytes must be in range(0, 256)".into()))?
            };
            self.resolve_rgba_palette_color(
                [color[0], color[1], color[2], palette_alpha],
                pa,
                reserved,
            )?
        } else {
            let (index, palette) = self.resolve_palette_color_with_reserved(color, reserved)?;
            if let Some(palette) = palette {
                self.replace_indexed_palette(&palette, "RGB")?;
            }
            index
        };
        Ok(if pa {
            PutPixelValue::Components(vec![i64::from(index), alpha])
        } else {
            PutPixelValue::Integer(i64::from(index))
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn resolve_rgba_palette_color(
        &mut self,
        color: [u8; 4],
        pa: bool,
        reserved: &[u32],
    ) -> Result<u8, PilError> {
        let mut palette = self.getpalette_rgba().unwrap_or_default();
        if let Some(index) = palette
            .chunks_exact(4)
            .position(|entry| entry == color.as_slice())
        {
            return Ok(index as u8);
        }
        let entries = palette.len() / 4;
        let index = self.palette_allocation_index(entries, reserved, !pa)?;
        if index >= entries {
            // Match ImagePalette's direct append even when the returned index
            // was advanced past reserved background/transparency slots.
            palette.extend_from_slice(&color);
        } else {
            palette[index * 4..index * 4 + 4].copy_from_slice(&color);
        }
        self.replace_indexed_palette(&palette, "RGBA")?;
        Ok(index as u8)
    }
}
