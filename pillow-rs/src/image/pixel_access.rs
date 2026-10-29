//! Pixel access and mutation methods for [`Image`].

use crate::error::PilError;
use crate::raster::GenericImageView;

use super::{
    Arc, DynamicImage, FormattedPixelValue, Image, PipelineOp, PutDataValue,
    is_l16_mode, pillow_band_count,
};

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

impl Image {
    /// Returns one pixel as an RGBA tuple.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::IndexError`] when coordinates are outside the image.
    pub fn getpixel(&self, x: u32, y: u32) -> Result<(u8, u8, u8, u8), PilError> {
        let img = self.materialized_shared()?;
        let (w, h) = (img.width(), img.height());
        if x >= w || y >= h {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let rgba = img.get_pixel(x, y).0;
        Ok((
            rgba[0],
            rgba.get(1).copied().unwrap_or(0),
            rgba.get(2).copied().unwrap_or(0),
            rgba.get(3).copied().unwrap_or(255),
        ))
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
        if data.explicit_mode.unwrap_or(data.image.mode()) != crate::raster::Mode::ImageRgb8 {
            return Ok(None);
        }
        if x >= data.image.width() || y >= data.image.height() {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let DynamicImage::ImageRgb8(rgb) = data.image.as_ref() else {
            return Ok(None);
        };
        Ok(Some(rgb.get_pixel(x, y).0))
    }

    /// Returns one pixel with Pillow's mode-specific scalar or tuple shape.
    pub fn getpixel_formatted(&self, x: u32, y: u32) -> Result<FormattedPixelValue, PilError> {
        let img = self.materialized_shared()?;
        let mode = self.mode_from_materialized(&img);
        let (width, height) = (img.width(), img.height());
        if is_l16_mode(&mode) {
            if x >= width || y >= height {
                return Err(PilError::IndexError("image index out of range".into()));
            }
            let index = (y as usize)
                .checked_mul(width as usize)
                .and_then(|row| row.checked_add(x as usize))
                .ok_or_else(|| PilError::InternalError("pixel index overflow".into()))?;
            return img
                .to_luma16()
                .as_raw()
                .get(index)
                .copied()
                .map(|value| FormattedPixelValue::Integer(i32::from(value)))
                .ok_or_else(|| PilError::InternalError("I;16 pixel index out of bounds".into()));
        }
        if mode == "I" || mode == "F" {
            if x >= width || y >= height {
                return Err(PilError::IndexError("image index out of range".into()));
            }
            let index = (y as usize)
                .checked_mul(width as usize)
                .and_then(|row| row.checked_add(x as usize))
                .ok_or_else(|| PilError::InternalError("pixel index overflow".into()))?;
            let offset = index
                .checked_mul(4)
                .ok_or_else(|| PilError::InternalError("scalar pixel offset overflow".into()))?;
            let end = offset
                .checked_add(4)
                .ok_or_else(|| PilError::InternalError("scalar pixel offset overflow".into()))?;
            let sample = img.as_bytes().get(offset..end).ok_or_else(|| {
                PilError::InternalError("scalar pixel index out of bounds".into())
            })?;
            if mode == "I" {
                return Ok(FormattedPixelValue::Integer(i32::from_le_bytes([
                    sample[0], sample[1], sample[2], sample[3],
                ])));
            }
            return Ok(FormattedPixelValue::Float(f32::from_le_bytes([
                sample[0], sample[1], sample[2], sample[3],
            ]) as f64));
        }

        let (r, g, b, a) = self.getpixel(x, y)?;
        Ok(match mode.as_str() {
            "L" | "1" | "P" => FormattedPixelValue::Scalar(r),
            "LA" | "La" | "PA" => FormattedPixelValue::Components(vec![r, a]),
            "RGB" => FormattedPixelValue::Components(vec![r, g, b]),
            // Pillow stores LAB A/B samples in the unsigned byte image as
            // values biased by 128. Merge applies that native C encoding;
            // decode it again at the public pixel boundary.
            "LAB" => {
                FormattedPixelValue::Components(vec![r, g.wrapping_sub(128), b.wrapping_sub(128)])
            }
            "RGBA" | "RGBa" | "RGBX" | "CMYK" => FormattedPixelValue::Components(vec![r, g, b, a]),
            _ => FormattedPixelValue::Components(vec![r, g, b]),
        })
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
        self.resolve_palette_color_with_reserved(color, &[])
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
        let mut next = entries;
        while next < 256 && reserved.contains(&(next as u32)) {
            next += 1;
        }
        let index = if next < 256 {
            next
        } else {
            let mut used = [false; 256];
            for index in self.tobytes()? {
                used[usize::from(index)] = true;
            }
            let transparent_index = self
                .palette_alpha()
                .filter(|alpha| alpha.len() == 1 && alpha[0] == 0)
                .map(|_| 0);
            (0..256)
                .rev()
                .find(|&index| {
                    !used[index]
                        && Some(index) != transparent_index
                        && !reserved.contains(&(index as u32))
                })
                .ok_or_else(|| {
                    PilError::ValueError("cannot allocate more than 256 colors".into())
                })?
        };

        if index >= entries {
            palette.extend_from_slice(&color);
        } else {
            palette[index * 3..index * 3 + 3].copy_from_slice(&color);
        }
        Ok((index as u8, Some(palette)))
    }

    /// Queues a Pillow-style single-integer pixel write.
    ///
    /// Binding crates use this path when host input is a scalar instead of a
    /// color tuple. `mode` decides how the scalar expands into the internal
    /// RGBA tuple before the same [`Image::putpixel`] pipeline path is used.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Image::putpixel`].
    pub fn putpixel_mode(&mut self, x: u32, y: u32, v: u8, mode: &str) -> Result<(), PilError> {
        // TODO(cpu-migration): take `Mode` directly from typed Rust callers.
        if is_l16_mode(mode) {
            return self.putpixel_l16(x, y, i64::from(v));
        }
        if mode == "P" {
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
            "L" | "1" => (v, v, v, 255),
            // A scalar PA write supplies the palette index and leaves the
            // alpha sample transparent, matching Pillow's getink expansion.
            "LA" | "PA" => (v, 0, 0, 0),
            "RGB" => (v, 0, 0, 255),
            // Pillow treats a scalar as the first sample for every
            // three-band mode, including YCbCr and HSV; the remaining
            // samples are zero rather than copies of the scalar.
            "YCbCr" | "HSV" => (v, 0, 0, 255),
            "RGBA" | "RGBX" | "CMYK" => (v, 0, 0, 0),
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
        self.load()?;
        if let Image::Loaded(data) = self {
            let image = Arc::make_mut(&mut data.image);
            let destination = image
                .as_mut_luma16()
                .ok_or_else(|| PilError::InternalError("putpixel I;16 storage mismatch".into()))?;
            let pixel = destination.as_mut().get_mut(pixel_index).ok_or_else(|| {
                PilError::InternalError("putpixel I;16 offset out of bounds".into())
            })?;
            *pixel = value as u16;
            data.known_zero_fill = false;
            return Ok(());
        }
        Err(PilError::InternalError(
            "putpixel I;16 did not materialize writable storage".into(),
        ))
    }

    /// Writes a scalar pixel for Pillow's numeric `I` and `F` modes.
    ///
    /// Those modes preserve the host numeric value as a four-byte signed or
    /// floating-point sample. The ordinary byte-oriented mode helper remains
    /// intentionally narrow for `L`, `1`, and `P` inputs.
    pub fn putpixel_mode_scalar(
        &mut self,
        x: u32,
        y: u32,
        value: f64,
        mode: &str,
    ) -> Result<(), PilError> {
        if !matches!(mode, "I" | "F") {
            return self.putpixel_mode(x, y, value as u8, mode);
        }
        let (width, height) = self.size()?;
        if x >= width || y >= height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let pixel_index = (y as usize)
            .checked_mul(width as usize)
            .and_then(|row| row.checked_add(x as usize))
            .ok_or_else(|| PilError::DimensionError("pixel index overflow".into()))?;
        self.putdata_value_at(pixel_index, &PutDataValue::Number(value), 1.0, 0.0)
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
                self.prepare_palette_float_components(&values)
            }
            PutPixelValue::Components(values) if matches!(values.len(), 3 | 4) => {
                self.prepare_palette_integer_components(&values, reserved)
            }
            value => Ok(value),
        }
    }

    fn prepare_palette_float_components(&self, values: &[f64]) -> Result<PutPixelValue, PilError> {
        // Python palette keys compare equal integer and float components
        // before bytes(color) validates a newly allocated entry.
        let rgba = self.palette_mode() == Some("RGBA");
        let palette = if rgba {
            self.getpalette_rgba()
        } else {
            self.extract_palette()
        }
        .unwrap_or_default();
        let mut color = values.to_vec();
        if rgba && color.len() == 3 {
            color.push(255.0);
        }
        if let Some(index) = palette
            .chunks_exact(if rgba { 4 } else { 3 })
            .position(|entry| {
                entry.len() == color.len()
                    && entry
                        .iter()
                        .zip(&color)
                        .all(|(&byte, &value)| f64::from(byte) == value)
            })
        {
            return Ok(if self.known_mode_identity() == Some(crate::raster::Mode::ImagePaletteAlpha8) {
                PutPixelValue::Components(vec![index as i64, 255])
            } else {
                PutPixelValue::Integer(index as i64)
            });
        }
        let channels = if rgba { 4 } else { 3 };
        if self.known_mode_identity() == Some(crate::raster::Mode::ImagePaletteAlpha8) && palette.len() / channels >= 256 {
            return Err(PilError::ValueError(
                "cannot allocate more than 256 colors".into(),
            ));
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
                self.putpalette(&palette, "RGB")?;
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
        let mut next = entries;
        while next < 256 && reserved.contains(&(next as u32)) {
            next += 1;
        }
        let index = if next < 256 {
            next
        } else if !pa {
            let mut used = [false; 256];
            for index in self.tobytes()? {
                used[usize::from(index)] = true;
            }
            (0..256)
                .rev()
                .find(|&index| !used[index] && !reserved.contains(&(index as u32)))
                .ok_or_else(|| {
                    PilError::ValueError("cannot allocate more than 256 colors".into())
                })?
        } else {
            return Err(PilError::ValueError(
                "cannot allocate more than 256 colors".into(),
            ));
        };
        if index >= entries {
            palette.extend_from_slice(&color);
        } else {
            palette[index * 4..index * 4 + 4].copy_from_slice(&color);
        }
        self.putpalette(&palette, "RGBA")?;
        Ok(index as u8)
    }

    /// Applies Pillow's public `putpixel` scalar/tuple normalization.
    pub fn putpixel_value(&mut self, x: u32, y: u32, value: PutPixelValue) -> Result<(), PilError> {
        let value = self.prepare_putpixel_palette(value, &[])?;
        let mode = self.mode()?;
        match value {
            PutPixelValue::Integer(value) => {
                if is_l16_mode(&mode) {
                    return self.putpixel_l16(x, y, value);
                }
                if mode == "I" {
                    // getink narrows the signed integer before storage. Going
                    // through f64 first loses low bits and saturates on cast.
                    return self.putpixel_mode_scalar(x, y, f64::from(value as i32), &mode);
                }
                if mode == "F" {
                    return self.putpixel_mode_scalar(x, y, value as f64, &mode);
                }
                if matches!(mode.as_str(), "1" | "L" | "P") {
                    return self.putpixel_mode(x, y, value.clamp(0, 255) as u8, &mode);
                }
                // Multiband integer ink is ABGR, including alpha in bits
                // 24..31 for two-band images. A singleton tuple takes this
                // same path; its member is not a clipped first component.
                let bytes = value.to_le_bytes();
                self.putpixel(x, y, bytes[0], bytes[1], bytes[2], bytes[3])
            }
            PutPixelValue::Float(value) => {
                if mode == "F" {
                    self.putpixel_mode_scalar(x, y, value, &mode)
                } else {
                    Err(if mode.len() == 1 {
                        PilError::TypeError("color must be int or single-element tuple".into())
                    } else {
                        PilError::TypeError("color must be int or tuple".into())
                    })
                }
            }
            PutPixelValue::Invalid => Err(if mode.len() == 1 {
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
                let bands = pillow_band_count(&mode);
                let palette_color =
                    matches!(mode.as_str(), "P" | "PA") && matches!(values.len(), 3 | 4);
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
                    [value, alpha] => self.putpixel(x, y, *value, 0, 0, *alpha),
                    [r, g, b] => self.putpixel(x, y, *r, *g, *b, 255),
                    [r, g, b, a] => self.putpixel(x, y, *r, *g, *b, *a),
                    _ => Err(PilError::InternalError(
                        "validated putpixel arity changed".into(),
                    )),
                }
            }
            PutPixelValue::FloatComponents(values) => match values.as_slice() {
                [value] if mode == "F" => self.putpixel_mode_scalar(x, y, *value, &mode),
                _ => Err(if mode.len() == 1 {
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

#[cfg(test)]
mod getpixel_rgb_fast_path_tests {
    use super::{Image, PilError};

    #[test]
    fn fast_path_reads_only_loaded_native_rgb_pixels() -> Result<(), PilError> {
        let rgb = Image::new(2, 1, "RGB", (17, 83, 211, 255))?;
        assert_eq!(rgb.getpixel_rgb_if_loaded(1, 0)?, Some([17, 83, 211]));
        assert!(matches!(
            rgb.getpixel_rgb_if_loaded(2, 0),
            Err(PilError::IndexError(_))
        ));

        let gray = Image::new(1, 1, "L", (42, 0, 0, 255))?;
        assert_eq!(gray.getpixel_rgb_if_loaded(0, 0)?, None);

        let deferred = crate::ops::imageops::invert(&rgb)?;
        assert_eq!(deferred.getpixel_rgb_if_loaded(0, 0)?, None);
        Ok(())
    }
}
