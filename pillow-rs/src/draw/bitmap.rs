use super::Draw;
use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::ops::draw::BitmapMask;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationRouteInputs, StrategySelection,
};
use crate::error::PilError;
use crate::image::Image;
use crate::ops::paste::PasteSource;
use crate::raster::{DynamicImage, Mode};

impl Draw {
    /// Draws a bitmap mask at `(x, y)` using `fill`.
    ///
    /// Accepted mask modes are `1`, `L`, `RGBA`, and `RGBa`. Coverage and
    /// destination writes retain their mode-specific native raster paths.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the bitmap mode is not a valid
    /// mask mode, and propagates mode, size, or pixel-data errors.
    pub fn bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        fill: Option<(u8, u8, u8, u8)>,
    ) -> Result<(), PilError> {
        let color = fill.unwrap_or((255, 255, 255, 255));
        let bmp_mode = bitmap.mode_identity()?;
        if !matches!(
            bmp_mode,
            Mode::One | Mode::ImageLuma8 | Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8
        ) {
            return Err(PilError::ValueError("bad transparency mask".to_string()));
        }
        let (bmp_w, bmp_h) = bitmap.size()?;
        let mode = self.effective_mode();
        // Pending Image pipelines keep their exact output mode on the wrapper.
        let raster_mode = self.image.mode_identity()?;
        let (requested_backend, context) = self.cpu_execution_context()?;
        let route_inputs = OperationRouteInputs {
            requested_backend,
            width: bmp_w,
            height: bmp_h,
            loaded_image: None,
            byte_storage_length_matches_pixel_count: None,
            draw_context_mode: Some(mode),
            bitmap_mask_mode: Some(bmp_mode),
        };
        let paste_route = crate::compute::pool_cpu::ops::draw::ops::SUPPORT
            .conditional_pipeline_child(raster_mode, route_inputs);
        if paste_route == Some(OperationId::Paste) {
            // A constant-color bitmap with a compatible mask has the same
            // byte operation contract as masked Paste for these native modes. Reuse the
            // existing selected-backend kernels instead of running this
            // destination loop on the host under a SIMD or GPU profile.
            // The 16x16 L benchmark remains about 2 us slower end-to-end on
            // SIMD than CPU because fixed pipeline and terminal-observation
            // costs dominate such a small blend. Keep the requested backend
            // route instead of disguising that case as CPU execution.
            let source = match mode {
                Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => {
                    PasteSource::Scalar(color.0)
                }
                Mode::ImageLumaA8 => PasteSource::LumaAlpha(color.0, color.3),
                _ => PasteSource::Rgba(color.0, color.1, color.2, color.3),
            };
            self.image
                .paste_bitmap_solid_masked_at(source, (x, y), bitmap)?;
            return Ok(());
        }

        let telemetry =
            crate::compute::begin_eager_cpu_operation_telemetry("DrawBitmap", requested_backend);
        let result = self.bitmap_cpu(
            x,
            y,
            bitmap,
            bmp_mode,
            bmp_w,
            bmp_h,
            color,
            mode,
            raster_mode,
            &context,
        );
        match result {
            Ok(selection) => {
                let output_bytes = self
                    .image
                    .materialized_shared()
                    .map_or(0, |image| image.as_bytes().len());
                crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                    telemetry,
                    output_bytes,
                    "DrawBitmap",
                    selection,
                );
                Ok(())
            }
            Err(error) => {
                crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                Err(error)
            }
        }
    }

    fn bitmap_cpu(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        bmp_mode: Mode,
        bmp_w: u32,
        bmp_h: u32,
        color: (u8, u8, u8, u8),
        mode: Mode,
        raster_mode: Mode,
        context: &ExecutionContext,
    ) -> Result<StrategySelection, PilError> {
        let palette = if matches!(raster_mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            && self.image.has_nonempty_palette()
        {
            crate::compute::registry::support::PaletteKnowledge::Present
        } else {
            crate::compute::registry::support::PaletteKnowledge::Absent
        };
        let support = crate::compute::pool_cpu::ops::draw::ops::SUPPORT.for_mode(raster_mode);
        let mut selection = match support {
            ModeSupport::Supported { .. } => Some(
                crate::compute::pool_cpu::ops::draw::ops::select_bitmap_strategy(
                    support,
                    context,
                    raster_mode,
                    mode,
                    bmp_mode,
                    palette,
                )?,
            ),
            ModeSupport::Rejected { .. } => None,
        };
        if let Some(prepared) = selection {
            crate::compute::pool_cpu::ops::draw::ops::validate_direct_selection(
                prepared,
                raster_mode,
            )?;
        }
        if self.try_native_rgb_bitmap(
            x,
            y,
            bitmap,
            bmp_mode,
            bmp_w,
            bmp_h,
            color,
            mode,
            raster_mode,
        )? {
            return selection.ok_or_else(|| {
                PilError::InternalError(
                    "Draw Bitmap fast path executed without a supported mode strategy".into(),
                )
            });
        }
        if self.try_native_cmyk_l_bitmap(
            x,
            y,
            bitmap,
            bmp_mode,
            bmp_w,
            bmp_h,
            color,
            raster_mode,
        )? {
            return selection.ok_or_else(|| {
                PilError::InternalError(
                    "Draw Bitmap fast path executed without a supported mode strategy".into(),
                )
            });
        }

        let mask_image = bitmap.materialized_shared()?;
        let mask = BitmapMask::new(bmp_mode, bmp_w, mask_image.as_bytes());
        if selection.is_none() {
            selection = Some(
                crate::compute::pool_cpu::ops::draw::ops::select_bitmap_strategy(
                    support,
                    context,
                    raster_mode,
                    mode,
                    bmp_mode,
                    palette,
                )?,
            );
        }
        let prepared = selection.ok_or_else(|| {
            PilError::InternalError("Draw Bitmap mode did not prepare a strategy".into())
        })?;
        crate::compute::pool_cpu::ops::draw::ops::validate_direct_selection(prepared, raster_mode)?;
        let selected_mode = prepared.implementation.mode;
        let paletted = if selected_mode == Mode::ImagePalette8 {
            self.image
                .palette()
                .filter(|palette| !palette.is_empty())
                .map(|palette| {
                    (
                        palette,
                        self.image.palette_alpha().unwrap_or_default(),
                        self.image.source_format(),
                        self.image.image_info(),
                        self.image.exif_metadata(),
                    )
                })
        } else {
            None
        };
        let image = self.image.materialize()?;
        let output = crate::compute::pool_cpu::ops::draw::ops::apply_bitmap(
            prepared,
            image,
            x,
            y,
            bmp_w,
            bmp_h,
            &mask,
            color,
        )?;
        if selected_mode == Mode::ImagePaletteAlpha8 {
            self.replace_palette_alpha_output(output)?;
        } else if let Some((palette, palette_alpha, source_format, info, exif)) = paletted {
            let DynamicImage::ImagePalette8(indices) = output else {
                return Err(PilError::InternalError(
                    "Draw Bitmap P strategy returned another raster mode".into(),
                ));
            };
            self.image = Image::Paletted(crate::image::PalettedData {
                indices,
                palette,
                palette_alpha,
                source_format,
                info,
                exif,
                materialized: crate::image::materialization_cache(),
            });
        } else {
            self.image = Image::from_dynamic(output);
        }
        Ok(prepared)
    }
    fn try_native_rgb_bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        bmp_mode: Mode,
        bmp_w: u32,
        bmp_h: u32,
        color: (u8, u8, u8, u8),
        mode: Mode,
        raster_mode: Mode,
    ) -> Result<bool, PilError> {
        // Ordinary RGB context and explicit RGBA context over an RGB image
        // both write the same three-byte destination. With RGB input alpha
        // fixed at 255, the visible channels use this native coverage blend.
        let rgb_target = raster_mode == Mode::ImageRgb8
            && (mode == Mode::ImageRgb8 || (mode == Mode::ImageRgba8 && self.alpha_blend_rgb()));
        if !rgb_target {
            return Ok(false);
        }
        let image = self.image.materialized_shared()?;
        let mask = bitmap.materialized_shared()?;
        let Some(output) = crate::compute::pool_cpu::ops::draw::rgb8::ops::try_native_bitmap(
            image.as_ref(),
            mask.as_ref(),
            bmp_mode,
            bmp_w,
            bmp_h,
            x,
            y,
            color,
        )? else {
            return Ok(false);
        };
        self.image = Image::from_dynamic(output);
        Ok(true)
    }

    fn try_native_cmyk_l_bitmap(
        &mut self,
        x: i32,
        y: i32,
        bitmap: &Image,
        bmp_mode: Mode,
        bmp_w: u32,
        bmp_h: u32,
        color: (u8, u8, u8, u8),
        raster_mode: Mode,
    ) -> Result<bool, PilError> {
        // CMYK pixels use the same four-byte carrier as RGBA in this image
        // model. Keep those bytes in place while drawing: materializing an
        // owned image and then calling `to_rgba8()` makes two full-frame
        // copies even though this path performs no color conversion.
        if raster_mode == Mode::ImageCmyk8
            && bmp_mode == Mode::ImageLuma8
        {
            let img = self.image.materialized_shared()?;
            let mask = bitmap.materialized_shared()?;
            if let Some(output) = crate::compute::pool_cpu::ops::draw::cmyk8::ops::apply_luma_mask(
                img.as_ref(),
                mask.as_ref(),
                x,
                y,
                bmp_w,
                bmp_h,
                color,
            ) {
                self.image = Image::from_dynamic(output);
                return Ok(true);
            }
        }
        Ok(false)
    }


}
