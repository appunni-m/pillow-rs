//! Pixel access and mutation methods for [`Image`].

use crate::compute::PipelineOperationTelemetryCapture;
use crate::error::PilError;
use crate::raster::Mode;

use super::data_access::is_l16_identity;
use super::{Arc, FormattedPixelValue, Image, PipelineOp};

/// Host-neutral value extracted for a public `Image.putpixel` call.
#[derive(Debug, Clone, PartialEq)]
pub enum PutPixelValue {
    /// An integer scalar, retaining its range for core validation.
    Integer(i64),
    /// A non-integral numeric value, accepted by Pillow's `F` mode.
    Float(f64),
    /// A list or tuple of integer components, retaining the source range.
    Components(Vec<i64>),
    /// A list or tuple of floating-point components, retaining fractional values.
    FloatComponents(Vec<f64>),
    /// A value that is not a supported scalar or component sequence.
    Invalid,
}

mod palette_components;

pub use palette_components::PaletteColorComponent;
use palette_components::{
    expand_palette_metadata, palette_component_matches_byte, palette_component_to_byte,
    palette_components_equal,
};

impl Image {
    /// Returns one pixel as an RGBA tuple.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::IndexError`] when coordinates are outside the image.
    pub fn getpixel(&self, x: u32, y: u32) -> Result<(u8, u8, u8, u8), PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let img = self.materialized_shared()?;
        operation_receipts.capture();
        let (w, h) = (img.width(), img.height());
        if x >= w || y >= h {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_pixel_rgba(
            img.mode(),
            &context,
        )?;
        let rgba = crate::compute::pool_cpu::ops::analysis::ops::execute_get_pixel_rgba(
            img.as_ref(),
            x,
            y,
            selection,
        )?;
        crate::compute::record_eager_cpu_operation_receipt_with_selection_and_receipts(
            "GetPixelRgba",
            requested_backend,
            rgba.len(),
            "RGBA8 pixel access uses the declared exact-mode Default strategy",
            selection,
            operation_receipts,
        );
        Ok((rgba[0], rgba[1], rgba[2], rgba[3]))
    }

    /// Returns an already-loaded native RGB pixel without converting it to RGBA.
    ///
    /// This binding fast path returns `None` unless this handle already owns a
    /// loaded `RGB` image backed by three-byte RGB pixels. Callers can then
    /// preserve deferred decoding and pipeline execution on their ordinary
    /// materialization path. Coordinates are checked before reading a pixel.
    #[doc(hidden)]
    #[inline]
    pub fn getpixel_rgb_if_loaded(&self, x: u32, y: u32) -> Result<Option<[u8; 3]>, PilError> {
        let Image::Loaded(data) = self else {
            return Ok(None);
        };
        if data.image.mode() != crate::raster::Mode::ImageRgb8 {
            return Ok(None);
        }
        if x >= data.image.width() || y >= data.image.height() {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_pixel(
            Mode::ImageRgb8,
            &context,
        )?;
        let rgb = crate::compute::pool_cpu::ops::analysis::ops::execute_get_pixel_rgb(
            data.image.as_ref(),
            x,
            y,
            selection,
        )?;
        crate::compute::record_eager_cpu_operation_receipt_with_selection(
            "GetPixel",
            requested_backend,
            rgb.len(),
            "RGB formatted pixel access reads the native three-byte carrier",
            selection,
        );
        Ok(Some(rgb))
    }

    /// Returns one pixel with Pillow's mode-specific scalar or tuple shape.
    pub fn getpixel_formatted(&self, x: u32, y: u32) -> Result<FormattedPixelValue, PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let img = self.materialized_shared()?;
        operation_receipts.capture();
        let (width, height) = (img.width(), img.height());
        if x >= width || y >= height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_pixel(
            img.mode(),
            &context,
        )?;
        let value = crate::compute::pool_cpu::ops::analysis::ops::execute_get_pixel(
            img.as_ref(),
            x,
            y,
            selection,
        )?;
        let output_bytes = crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::
            formatted_pixel_bytes(&value);
        crate::compute::record_eager_cpu_operation_receipt_with_selection_and_receipts(
            "GetPixel",
            requested_backend,
            output_bytes,
            "formatted pixel access reads the exact native mode strategy",
            selection,
            operation_receipts,
        );
        Ok(value)
    }

    /// Queues an in-place single-pixel write.
    ///
    /// The color is normalized as `(r, g, b, a)` before entering the pipeline.
    /// Materialization performs the mode-specific storage conversion.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::IndexError`] when coordinates are outside the image,
    /// or another [`PilError`] when image size lookup requires decoding and that
    /// fails.
    pub fn putpixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), PilError> {
        let (w, h) = self.size()?;
        if x >= w || y >= h {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let (r, g, b) = if self.known_mode_identity() == Some(crate::raster::Mode::ImageLab8) {
            // The public LAB bands expose signed A/B values, while the shared
            // RGB8 raster stores them with Pillow's unsigned +128 bias.
            (r, g.wrapping_add(128), b.wrapping_add(128))
        } else {
            (r, g, b)
        };
        let (color, palette_index, updated_palette) = if self.has_palette_mode() {
            if a != 255 {
                return Err(PilError::ValueError(
                    "cannot add non-opaque RGBA color to RGB palette".into(),
                ));
            }
            let (index, palette) = self.resolve_palette_color([r, g, b])?;
            ((index, index, index, 255), true, palette)
        } else {
            ((r, g, b, a), false, None)
        };
        let mut new_self = Image::push_op(
            self,
            PipelineOp::PutPixel {
                x,
                y,
                color,
                palette_index,
            },
        );
        if let Some(updated_palette) = updated_palette {
            let Image::Pipeline { palette, .. } = &mut new_self else {
                return Err(PilError::InternalError(
                    "putpixel did not create an operation pipeline".into(),
                ));
            };
            *palette = Some(updated_palette);
        }
        *self = new_self;
        Ok(())
    }

    /// Resolve a Pillow-style RGB tuple to a palette index.
    ///
    /// Existing colors retain their first palette index. A missing color uses
    /// the next table entry, or an unused entry when the table already contains
    /// 256 colors. The returned palette is present only when allocation changed
    /// it.
    fn resolve_palette_color(&self, color: [u8; 3]) -> Result<(u8, Option<Vec<u8>>), PilError> {
        let reserved = self.palette_reserved_indices();
        self.resolve_palette_color_with_reserved(color, &reserved)
    }

    // Pillow reserves only the current `Image.info` values during allocation;
    // do not infer an index from a retained palette-alpha sidecar.
    fn palette_reserved_indices(&self) -> Vec<u32> {
        self.compatibility_info()
            .into_iter()
            .filter_map(|(name, value)| {
                if !matches!(name.as_str(), "background" | "transparency") {
                    return None;
                }
                let index = match value {
                    super::ImageInfoValue::Integer(index) => u32::try_from(index).ok(),
                    super::ImageInfoValue::Float(index)
                        if index.is_finite() && index.fract() == 0.0 =>
                    {
                        (0.0..=255.0).contains(&index).then_some(index as u32)
                    }
                    _ => None,
                }?;
                (index < 256).then_some(index)
            })
            .collect()
    }

    fn resolve_palette_color_with_reserved(
        &self,
        color: [u8; 3],
        reserved: &[u32],
    ) -> Result<(u8, Option<Vec<u8>>), PilError> {
        // Pillow's Image.putpixel creates an empty ImagePalette when a public
        // P-mode operation leaves no palette attached (for example, after
        // ImageDraw.bitmap). Treat the missing palette as an empty table so
        // tuple-color writes allocate the same first entry instead of raising
        // a binding-visible PaletteError.
        let mut palette = self.extract_palette().unwrap_or_default();
        if let Some(index) = palette
            .chunks_exact(3)
            .position(|entry| entry == color.as_slice())
        {
            return Ok((index as u8, None));
        }

        let entries = palette.len() / 3;
        let index = self.palette_allocation_index(entries, reserved, true)?;

        if index >= entries {
            // ImagePalette advances the returned index past reserved slots,
            // then appends bytes at the current palette length without padding.
            palette.extend_from_slice(&color);
        } else {
            palette[index * 3..index * 3 + 3].copy_from_slice(&color);
        }
        Ok((index as u8, Some(palette)))
    }

    fn palette_allocation_index(
        &self,
        entries: usize,
        reserved: &[u32],
        reuse_unused: bool,
    ) -> Result<usize, PilError> {
        // TODO(pillow-rs#3ptb): freeze P/PA parity for reserved metadata
        // indices in short palettes and full-palette reuse, including when
        // public Python `Image.info` overrides a retained palette-alpha marker.
        // Pillow skips `background`/`transparency`, but appends at the current
        // palette end even when the returned index is later.
        let mut next = entries;
        while next < 256 && reserved.contains(&(next as u32)) {
            next += 1;
        }
        if next < 256 {
            return Ok(next);
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
        (0..256)
            .rev()
            .find(|&index| !used[index] && !reserved.contains(&(index as u32)))
            .ok_or_else(|| PilError::ValueError("cannot allocate more than 256 colors".into()))
    }

    pub(crate) fn ensure_draw_palette_entry_available(
        &self,
        reserved: &[u32],
    ) -> Result<(), PilError> {
        self.draw_palette_allocation_index(reserved).map(|_| ())
    }

    /// Queues a Pillow-style single-integer pixel write.
    ///
    /// The mode-specific scalar expansion comes from this image's own raster
    /// identity before the same [`Image::putpixel`] pipeline path is used.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Image::putpixel`].
    pub fn putpixel_mode(&mut self, x: u32, y: u32, v: u8) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        if x >= width || y >= height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let mode = self.mode_identity()?;
        self.putpixel_mode_for_mode(x, y, v, mode)
    }

    fn putpixel_mode_for_mode(
        &mut self,
        x: u32,
        y: u32,
        v: u8,
        mode: Mode,
    ) -> Result<(), PilError> {
        if is_l16_identity(mode) {
            return self.putpixel_l16(x, y, i64::from(v));
        }
        if mode == Mode::ImagePalette8 {
            let (w, h) = self.size()?;
            if x >= w || y >= h {
                return Err(PilError::IndexError("image index out of range".into()));
            }
            *self = Image::push_op(
                self,
                PipelineOp::PutPixel {
                    x,
                    y,
                    color: (v, v, v, 255),
                    palette_index: true,
                },
            );
            return Ok(());
        }
        let (r, g, b, a) = match mode {
            Mode::ImageLuma8 | Mode::One => (v, v, v, 255),
            // A scalar PA write supplies the palette index and leaves the
            // alpha sample transparent, matching Pillow's getink expansion.
            Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 => (v, 0, 0, 0),
            Mode::ImageRgb8 => (v, 0, 0, 255),
            // Pillow treats a scalar as the first sample for every
            // three-band mode, including YCbCr and HSV; the remaining
            // samples are zero rather than copies of the scalar.
            Mode::ImageYCbCr8 | Mode::ImageHsv8 => (v, 0, 0, 255),
            Mode::ImageRgba8 | Mode::ImageRgbx8 | Mode::ImageCmyk8 => (v, 0, 0, 0),
            _ => (v, v, v, 255),
        };
        self.putpixel(x, y, r, g, b, a)
    }

    /// Writes one integer sample for Pillow's unsigned 16-bit luma modes.
    ///
    /// These modes are stored as native `u16` samples, while their public byte
    /// order is applied only when the image is serialized. Keep the integer
    /// conversion here so `putpixel` preserves Pillow's modulo-65536 behavior
    /// for negative and over-range values without routing the value through the
    /// eight-bit `PipelineOp::PutPixel` representation.
    fn putpixel_l16(&mut self, x: u32, y: u32, value: i64) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        if x >= width || y >= height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let pixel_index = (y as usize)
            .checked_mul(width as usize)
            .and_then(|row| row.checked_add(x as usize))
            .ok_or_else(|| PilError::DimensionError("pixel index overflow".into()))?;
        let mode = self.mode_identity()?;
        if !is_l16_identity(mode) {
            return Err(PilError::InternalError(
                "putpixel I;16 storage mismatch".into(),
            ));
        }
        self.putpixel_sample_at(
            mode,
            pixel_index,
            crate::compute::pool_cpu::ops::put::ops::PutPixelValueAtInput::Luma16(value as u16),
        )
    }

    fn putpixel_sample_at(
        &mut self,
        mode: Mode,
        pixel_index: usize,
        input: crate::compute::pool_cpu::ops::put::ops::PutPixelValueAtInput,
    ) -> Result<(), PilError> {
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let selection = crate::compute::pool_cpu::ops::put::ops::select_put_pixel_value_at(
            mode,
            &context,
        )?;

        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        self.load()?;
        operation_receipts.capture();
        let Image::Loaded(data) = self else {
            let reason = if is_l16_identity(mode) {
                "putpixel I;16 did not materialize writable storage"
            } else {
                "putdata did not materialize writable storage"
            };
            return Err(PilError::InternalError(reason.into()));
        };
        crate::compute::pool_cpu::ops::put::ops::execute_put_pixel_value_at(
            Arc::make_mut(&mut data.image),
            pixel_index,
            input,
            selection,
        )?;
        data.known_zero_fill = false;
        let fallback_detail = if is_l16_identity(mode) {
            "I;16 pixel writes update host storage synchronously"
        } else {
            "numeric I/F pixel writes update host storage synchronously"
        };
        crate::compute::record_eager_cpu_operation_receipt_with_selection_and_receipts(
            "PutPixel",
            requested_backend,
            0,
            fallback_detail,
            selection,
            operation_receipts,
        );
        Ok(())
    }

    /// Writes a scalar pixel for Pillow's numeric `I` and `F` modes.
    ///
    /// Those modes preserve the host numeric value as a four-byte signed or
    /// floating-point sample. The ordinary byte-oriented mode helper remains
    /// intentionally narrow for `L`, `1`, and `P` inputs. The source raster's
    /// exact identity determines whether the value is stored as I32 or F32.
    pub fn putpixel_mode_scalar(&mut self, x: u32, y: u32, value: f64) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        if x >= width || y >= height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let image_mode = self.mode_identity()?;
        self.putpixel_mode_scalar_for_mode(x, y, value, image_mode)
    }

    fn putpixel_mode_scalar_for_mode(
        &mut self,
        x: u32,
        y: u32,
        value: f64,
        image_mode: Mode,
    ) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        if x >= width || y >= height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        if !matches!(image_mode, Mode::ImageI32 | Mode::ImageF32) {
            return self.putpixel_mode_for_mode(x, y, value as u8, image_mode);
        }
        let pixel_index = (y as usize)
            .checked_mul(width as usize)
            .and_then(|row| row.checked_add(x as usize))
            .ok_or_else(|| PilError::DimensionError("pixel index overflow".into()))?;
        let bytes = super::data_access::putpixel_numeric_bytes(image_mode, value)?;
        self.putpixel_sample_at(
            image_mode,
            pixel_index,
            crate::compute::pool_cpu::ops::put::ops::PutPixelValueAtInput::NumericBytes(bytes),
        )
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
    pub(crate) fn resolve_palette_fill_color(
        &self,
        components: &[PaletteColorComponent],
    ) -> Result<(u8, Vec<u8>, Option<Vec<u8>>), PilError> {
        let rgba = self.palette_mode() == Some("RGBA");
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
    pub(crate) fn prepare_draw_palette_tuple_components(
        &mut self,
        components: &[PaletteColorComponent],
        reserved: &[u32],
        color_cache: &mut Vec<(Vec<PaletteColorComponent>, u8)>,
    ) -> Result<PutPixelValue, PilError> {
        let rgba_palette = self.palette_mode() == Some("RGBA");
        let mut color = components.to_vec();
        if rgba_palette && color.len() == 3 {
            color.push(PaletteColorComponent::Integer(255));
        } else if !rgba_palette && color.len() == 4 {
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
        if let Some(index) = self.palette_float_match_index(&lookup_color, rgba_palette) {
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
                PaletteColorComponent::Integer(_)
                | PaletteColorComponent::WideInteger { .. } => {
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

        let channels = if rgba_palette { 4 } else { 3 };
        if integer_color.len() == channels {
            return self.prepare_palette_integer_components(&integer_color, reserved);
        }

        if integer_color.is_empty() {
            return Ok(PutPixelValue::Integer(index as i64));
        }

        let mode = if rgba_palette { "RGBA" } else { "RGB" };
        let mut palette = if rgba_palette {
            self.getpalette_rgba()
        } else {
            self.extract_palette()
        }
        .unwrap_or_default();
        let start = index * channels;
        if start < palette.len() {
            let end = (start + channels).min(palette.len());
            palette.splice(start..end, integer_color.iter().map(|&value| value as u8));
        } else {
            palette.extend(integer_color.iter().map(|&value| value as u8));
        }
        // Pillow's public Python ImagePalette retains arbitrary trailing bytes
        // in its own buffer. This core path installs only complete palette
        // entries used by indexed pixels; the wrapper keeps its buffer intact.
        self.replace_indexed_palette_from_draw(&palette, mode)?;
        Ok(PutPixelValue::Integer(index as i64))
    }

    fn draw_palette_allocation_index(&self, reserved: &[u32]) -> Result<usize, PilError> {
        let rgba_palette = self.palette_mode() == Some("RGBA");
        let channels = if rgba_palette { 4 } else { 3 };
        let palette = if rgba_palette {
            self.getpalette_rgba()
        } else {
            self.extract_palette()
        }
        .unwrap_or_default();
        self.palette_allocation_index(palette.len() / channels, reserved, true)
    }

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

    fn normalize_palette_float_color(
        &self,
        values: &[f64],
    ) -> Result<(Vec<f64>, bool), PilError> {
        let rgba_palette = self.palette_mode() == Some("RGBA");
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

    fn prepare_palette_float_components(
        &self,
        values: &[f64],
        reserved: &[u32],
    ) -> Result<PutPixelValue, PilError> {
        // TODO(pillow-rs#3ptb): parity-check nonstandard tuple arities and
        // palette-color cache state after `bytes(color)` rejects float values.
        // Python palette keys compare equal integer and float components
        // before `bytes(color)` validates a new entry. Normalize tuple arity
        // first, then preserve Pillow's allocation-error precedence.
        let pa = self.known_mode_identity() == Some(crate::raster::Mode::ImagePaletteAlpha8);
        let (color, rgba) = self.normalize_palette_float_color(values)?;
        let palette = if rgba {
            self.getpalette_rgba()
        } else {
            self.extract_palette()
        }
        .unwrap_or_default();
        if let Some(index) = self.palette_float_match_index(&color, rgba) {
            return Ok(if pa {
                PutPixelValue::Components(vec![index as i64, 255])
            } else {
                PutPixelValue::Integer(index as i64)
            });
        }
        let channels = if rgba { 4 } else { 3 };
        if pa {
            self.palette_allocation_index(palette.len() / channels, reserved, false)?;
        } else {
            self.ensure_draw_palette_entry_available(reserved)?;
        }
        Err(PilError::TypeError(
            "'float' object cannot be interpreted as an integer".into(),
        ))
    }

    fn prepare_palette_integer_components(
        &mut self,
        values: &[i64],
        reserved: &[u32],
    ) -> Result<PutPixelValue, PilError> {
        let pa = self.known_mode_identity() == Some(crate::raster::Mode::ImagePaletteAlpha8);
        let alpha = values.get(3).copied().unwrap_or(255);
        let rgba_palette = self.palette_mode() == Some("RGBA");
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

    /// Applies Pillow's public `putpixel` scalar/tuple normalization.
    pub fn putpixel_value(&mut self, x: u32, y: u32, value: PutPixelValue) -> Result<(), PilError> {
        let allocates_palette_color = self.has_palette_samples()
            && matches!(&value, PutPixelValue::Components(values) if matches!(values.len(), 3 | 4));
        let value = if allocates_palette_color {
            let reserved = self.palette_reserved_indices();
            self.prepare_putpixel_palette(value, &reserved)?
        } else {
            self.prepare_putpixel_palette(value, &[])?
        };
        let mode = self.mode_identity()?;
        match value {
            PutPixelValue::Integer(value) => {
                if is_l16_identity(mode) {
                    return self.putpixel_l16(x, y, value);
                }
                if mode == Mode::ImageI32 {
                    // getink narrows the signed integer before storage. Going
                    // through f64 first loses low bits and saturates on cast.
                    return self.putpixel_mode_scalar_for_mode(x, y, f64::from(value as i32), mode);
                }
                if mode == Mode::ImageF32 {
                    return self.putpixel_mode_scalar_for_mode(x, y, value as f64, mode);
                }
                if matches!(mode, Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8) {
                    return self.putpixel_mode_for_mode(x, y, value.clamp(0, 255) as u8, mode);
                }
                // Multiband integer ink is ABGR, including alpha in bits
                // 24..31 for two-band images. A singleton tuple takes this
                // same path; its member is not a clipped first component.
                let bytes = value.to_le_bytes();
                self.putpixel(x, y, bytes[0], bytes[1], bytes[2], bytes[3])
            }
            PutPixelValue::Float(value) => {
                if mode == Mode::ImageF32 {
                    self.putpixel_mode_scalar_for_mode(x, y, value, mode)
                } else {
                    Err(if mode.pillow_name().is_some_and(|name| name.len() == 1) {
                        PilError::TypeError("color must be int or single-element tuple".into())
                    } else {
                        PilError::TypeError("color must be int or tuple".into())
                    })
                }
            }
            PutPixelValue::Invalid => Err(if mode.pillow_name().is_some_and(|name| name.len() == 1) {
                PilError::TypeError("color must be int or single-element tuple".into())
            } else {
                // Pillow routes non-integral component sequences through its
                // tuple-arity validator, rather than the scalar tuple error.
                PilError::TypeError(
                    "color must be int, or tuple of one, three or four elements".into(),
                )
            }),
            PutPixelValue::Components(values) => {
                if let [value] = values.as_slice() {
                    return self.putpixel_value(x, y, PutPixelValue::Integer(*value));
                }
                let bands = mode.band_count();
                let palette_color = matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
                    && matches!(values.len(), 3 | 4);
                if !palette_color
                    && ((bands == 1)
                        || (bands == 2 && values.len() != 2)
                        || (bands >= 3 && !matches!(values.len(), 3 | 4)))
                {
                    return Err(PilError::TypeError(
                        match bands {
                            1 => "color must be int or single-element tuple",
                            2 => "color must be int, or tuple of one or two elements",
                            _ => "color must be int, or tuple of one, three or four elements",
                        }
                        .into(),
                    ));
                }
                let bytes: Vec<u8> = values
                    .iter()
                    .map(|&value| value.clamp(0, 255) as u8)
                    .collect();
                match bytes.as_slice() {
                    [value, alpha] => self.putpixel(x, y, *value, *value, *value, *alpha),
                    [r, g, b] => self.putpixel(x, y, *r, *g, *b, 255),
                    [r, g, b, a] => self.putpixel(x, y, *r, *g, *b, *a),
                    _ => Err(PilError::InternalError(
                        "validated putpixel arity changed".into(),
                    )),
                }
            }
            PutPixelValue::FloatComponents(values) => match values.as_slice() {
                [value] if mode == Mode::ImageF32 => {
                    self.putpixel_mode_scalar_for_mode(x, y, *value, mode)
                }
                _ => Err(if mode.pillow_name().is_some_and(|name| name.len() == 1) {
                    PilError::TypeError("color must be int or single-element tuple".into())
                } else {
                    PilError::TypeError(
                        "color must be int, or tuple of one, three or four elements".into(),
                    )
                }),
            },
        }
    }
}
