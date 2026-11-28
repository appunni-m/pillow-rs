//! Pixel access and mutation methods for [`Image`].

use crate::compute::PipelineOperationTelemetryCapture;
use crate::error::PilError;
use crate::raster::Mode;

use super::data_access::is_l16_identity;
use super::{Arc, FormattedPixelValue, Image, PaletteData, PaletteModel, PipelineOp};
use palette::write_palette_entry;

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

type PutPixelExecutionRoute = (
    Option<crate::compute::Backend>,
    crate::compute::ExecutionContext,
);
mod palette;
mod palette_components;
mod putpixel;

pub use palette_components::PaletteColorComponent;

impl Image {
    /// Returns one pixel as an RGBA tuple.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::IndexError`] when coordinates are outside the image.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
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
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    #[inline]
    pub fn getpixel_rgb_if_loaded(&self, x: u32, y: u32) -> Result<Option<[u8; 3]>, PilError> {
        self.getpixel_rgb_if_loaded_with_context(x, y, None)
    }

    /// Reads an already-loaded native RGB pixel with a wrapper-captured route.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn getpixel_rgb_if_loaded_with_context(
        &self,
        x: u32,
        y: u32,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Option<[u8; 3]>, PilError> {
        let execution_route =
            execution_context.map(|context| (self.cpu_arch_execution_route_advisory().0, context));
        self.getpixel_rgb_if_loaded_with_execution_route(x, y, execution_route)
    }

    /// Reads an already-loaded native RGB pixel with a captured backend/architecture route.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn getpixel_rgb_if_loaded_with_execution_route(
        &self,
        x: u32,
        y: u32,
        execution_route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<Option<[u8; 3]>, PilError> {
        let Image::Loaded(data) = self else {
            return Ok(None);
        };
        if data.image.mode() != crate::raster::Mode::ImageRgb8 {
            return Ok(None);
        }
        if x >= data.image.width() || y >= data.image.height() {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let (requested_backend, context) =
            execution_route.unwrap_or_else(|| self.cpu_arch_execution_route_advisory());
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
    // TODO(pillow-rs#3ptb): verify mode outputs, input faults, and exact
    // architecture/fallback receipts for GetPixel and stored PixelAccess.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn getpixel_formatted(&self, x: u32, y: u32) -> Result<FormattedPixelValue, PilError> {
        self.getpixel_formatted_with_context(x, y, None)
    }

    /// Returns one pixel using a route captured by a host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn getpixel_formatted_with_context(
        &self,
        x: u32,
        y: u32,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<FormattedPixelValue, PilError> {
        let execution_route =
            execution_context.map(|context| (self.cpu_arch_execution_route_advisory().0, context));
        self.getpixel_formatted_with_execution_route(x, y, execution_route)
    }

    /// Returns one pixel using a captured backend/architecture route.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn getpixel_formatted_with_execution_route(
        &self,
        x: u32,
        y: u32,
        execution_route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<FormattedPixelValue, PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let img = self.materialized_shared()?;
        operation_receipts.capture();
        let (width, height) = (img.width(), img.height());
        if x >= width || y >= height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let (requested_backend, context) =
            execution_route.unwrap_or_else(|| self.cpu_arch_execution_route_advisory());
        let selection =
            crate::compute::pool_cpu::ops::analysis::ops::select_get_pixel(img.mode(), &context)?;
        let value = crate::compute::pool_cpu::ops::analysis::ops::execute_get_pixel(
            img.as_ref(),
            x,
            y,
            selection,
        )?;
        let output_bytes =
            crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::formatted_pixel_bytes(
                &value,
            );
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
    /// Direct color modes use `(r, g, b, a)`; a P image with a CMYK palette
    /// interprets these bytes as native `(c, m, y, k)` palette components.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::IndexError`] when coordinates are outside the image,
    /// or another [`PilError`] when image size lookup requires decoding and that
    /// fails.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn putpixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) -> Result<(), PilError> {
        let (w, h) = self.size()?;
        if x >= w || y >= h {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let (color, palette_index, updated_palette) = if self.has_palette_mode() {
            let reserved = self.palette_reserved_indices();
            if self.palette_model() == Some(PaletteModel::Cmyk) {
                let components = [r, g, b, a].map(i64::from);
                let index = self.resolve_cmyk_palette_color(&components, &reserved)?;
                ((index, index, index, 255), true, None)
            } else {
                if a != 255 {
                    return Err(PilError::ValueError(
                        "cannot add non-opaque RGBA color to RGB palette".into(),
                    ));
                }
                let (index, palette) =
                    self.resolve_palette_color_with_reserved([r, g, b], &reserved)?;
                ((index, index, index, 255), true, palette)
            }
        } else {
            let color = if self.mode_identity()? == Mode::ImageLab8 {
                // Pillow's LAB `putpixel` accepts logical a/b samples, while
                // its raster byte representation stores each lane with the
                // signed chroma bias removed.
                (r, g.wrapping_sub(128), b.wrapping_sub(128), a)
            } else {
                (r, g, b, a)
            };
            (color, false, None)
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
            let Image::Pipeline {
                palette,
                palette_data,
                ..
            } = &mut new_self
            else {
                return Err(PilError::InternalError(
                    "putpixel did not create an operation pipeline".into(),
                ));
            };
            *palette = Some(updated_palette);
            *palette_data = Some(if matches!(self.palette_data(), Some(PaletteData::Rgba)) {
                PaletteData::Rgba
            } else {
                PaletteData::Rgb
            });
        }
        *self = new_self;
        Ok(())
    }

    /// Queues a byte pixel write under a route captured by a host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putpixel_with_execution_context(
        &mut self,
        x: u32,
        y: u32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        self.putpixel(x, y, r, g, b, a)?;
        *self = self.clone().with_execution_context(context);
        Ok(())
    }

    // Pillow reserves only the current `Image.info` values during allocation;
    // do not infer an index from a retained palette-alpha sidecar.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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

        write_palette_entry(&mut palette, 3, index, &color);
        Ok((index as u8, Some(palette)))
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
    // VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
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
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putpixel_l16(&mut self, x: u32, y: u32, value: i64) -> Result<(), PilError> {
        self.putpixel_l16_with_route(x, y, value, None)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putpixel_l16_with_route(
        &mut self,
        x: u32,
        y: u32,
        value: i64,
        route: Option<PutPixelExecutionRoute>,
    ) -> Result<(), PilError> {
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
        self.putpixel_sample_at_with_route(
            mode,
            pixel_index,
            crate::compute::pool_cpu::ops::put::ops::PutPixelValueAtInput::Luma16(value as u16),
            route,
        )
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putpixel_sample_at_with_route(
        &mut self,
        mode: Mode,
        pixel_index: usize,
        input: crate::compute::pool_cpu::ops::put::ops::PutPixelValueAtInput,
        route: Option<PutPixelExecutionRoute>,
    ) -> Result<(), PilError> {
        let (requested_backend, context) =
            route.unwrap_or_else(|| self.cpu_arch_execution_route_advisory());
        let selection =
            crate::compute::pool_cpu::ops::put::ops::select_put_pixel_value_at(mode, &context)?;

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
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putpixel_mode_scalar(&mut self, x: u32, y: u32, value: f64) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        if x >= width || y >= height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let image_mode = self.mode_identity()?;
        self.putpixel_mode_scalar_for_mode(x, y, value, image_mode, None)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putpixel_mode_scalar_for_mode(
        &mut self,
        x: u32,
        y: u32,
        value: f64,
        image_mode: Mode,
        route: Option<PutPixelExecutionRoute>,
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
        self.putpixel_sample_at_with_route(
            image_mode,
            pixel_index,
            crate::compute::pool_cpu::ops::put::ops::PutPixelValueAtInput::NumericBytes(bytes),
            route,
        )
    }
}
