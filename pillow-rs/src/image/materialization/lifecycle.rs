use super::{
    DynamicImage, Image, LoadedData, PaletteData, PalettedData, PilError, declared_pipeline_mode,
    expand_palette, image_from_materialized, map_codec_error, materialization_cache,
};
impl Image {
    /// Materializes the image for color-space image operations.
    ///
    /// `P` images are converted through their palette to RGB so operations such
    /// as paste, composite, and filters operate on visible colors. Use
    /// [`Image::materialize_indices`] when an operation needs palette indices.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when lazy decoding or pipeline execution fails.
    pub fn materialize_for_ops(&self) -> Result<DynamicImage, PilError> {
        // Fast path: Paletted images have direct palette→RGB conversion
        if let Some(rgb) = self.paletted_to_rgb() {
            return Ok(rgb);
        }
        // Pipeline-based P-mode images need palette→RGB conversion
        if matches!(self, Image::Pipeline { .. })
            && self.known_mode_identity() == Some(crate::raster::Mode::ImagePalette8)
        {
            let img = self.materialize()?; // Luma8 indices
            if let Some(palette) = self.palette() {
                let indices = img.to_luma8();
                let palette_alpha = self.palette_alpha().unwrap_or_default();
                return Ok(expand_palette(&indices, &palette, &palette_alpha));
            }
        }
        self.materialize()
    }

    /// Convert Paletted image to RGB for rendering/saving. Returns None for non-Paletted.
    pub(crate) fn paletted_to_rgb(&self) -> Option<DynamicImage> {
        if let Image::Paletted(data) = self {
            Some(expand_palette(
                &data.indices.to_luma8(),
                &data.palette,
                &data.palette_alpha,
            ))
        } else {
            None
        }
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pass@migration-parity-be5b7f3169b142d3abf46796c30837c4; benchmark=pending
    fn pipeline_to_loaded(
        image_handle: &Image,
        source: &Image,
        ops: &super::super::PipelineOps,
        mode: &super::super::PipelineModeCache,
        format: Option<image_slash_star::ImageFormat>,
        palette: &Option<Vec<u8>>,
        palette_data: &Option<PaletteData>,
        palette_alpha: &Option<Vec<u8>>,
    ) -> Result<Image, PilError> {
        let image = image_handle.materialized_shared()?;
        if let Some(output) = ops.palette_output() {
            let source_format = image_handle.source_format();
            let info = image_handle.image_info();
            let exif = image_handle.exif_metadata();
            return match output.mode {
                crate::raster::Mode::ImagePalette8
                    if matches!(
                        output.wrapper_metadata,
                        crate::compute::pool_cpu::ops::color::ops::ConvertWrapperMetadata::PreserveSourceLoaded
                    ) =>
                {
                    Ok(Image::Loaded(LoadedData {
                        image,
                        known_zero_fill: false,
                        palette: output.palette.clone(),
                        palette_data: output.palette_data.clone(),
                        palette_alpha: output.palette_alpha.clone(),
                        source_format,
                        info,
                        exif,
                    }))
                }
                crate::raster::Mode::ImagePalette8 => {
                    let retained_palette = output.palette.clone().ok_or_else(|| {
                        PilError::InternalError("prepared P conversion omitted RGB entries".into())
                    })?;
                    Ok(Image::Paletted(PalettedData {
                        indices: DynamicImage::ImagePalette8(image.to_luma8()),
                        palette_attached: true,
                        palette: retained_palette,
                        palette_data: output.palette_data.clone().unwrap_or_else(|| {
                            crate::image::PaletteData::from_alpha(
                                output.palette_alpha.as_deref().unwrap_or_default(),
                            )
                        }),
                        palette_alpha: output.palette_alpha.clone().unwrap_or_default(),
                        source_format,
                        info,
                        exif,
                        materialized: materialization_cache(),
                    }))
                }
                crate::raster::Mode::ImagePaletteAlpha8 => Ok(Image::Loaded(LoadedData {
                    image,
                    known_zero_fill: false,
                    palette: output.palette.clone(),
                    palette_data: output.palette_data.clone(),
                    palette_alpha: output.palette_alpha.clone(),
                    source_format,
                    info,
                    exif,
                })),
                _ => Err(PilError::InternalError(
                    "cached palette conversion has a non-palette output mode".into(),
                )),
            };
        }

        if declared_pipeline_mode(mode) == Some(crate::raster::Mode::ImagePalette8) {
            return Ok(Image::Paletted(PalettedData {
                indices: DynamicImage::ImagePalette8(image.to_luma8()),
                palette_attached: palette.is_some(),
                palette: palette.clone().unwrap_or_default(),
                palette_data: palette_data
                    .clone()
                    .or_else(|| source.palette_data().cloned())
                    .unwrap_or_else(|| {
                        crate::image::PaletteData::from_alpha(
                            palette_alpha.as_deref().unwrap_or_default(),
                        )
                    }),
                palette_alpha: palette_alpha.clone().unwrap_or_default(),
                source_format: format,
                info: source.image_info(),
                exif: source.exif_metadata(),
                materialized: materialization_cache(),
            }));
        }

        Ok(Image::Loaded(LoadedData {
            image,
            known_zero_fill: false,
            palette: palette.clone(),
            palette_data: palette_data
                .clone()
                .or_else(|| source.palette_data().cloned()),
            palette_alpha: palette_alpha.clone(),
            source_format: format,
            info: source.image_info(),
            exif: source.exif_metadata(),
        }))
    }

    /// Forces lazy decoding and pipeline execution.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    // VERIFY: source=reviewed@2026-11-25; parity=pass@migration-parity-be5b7f3169b142d3abf46796c30837c4; benchmark=pending
    pub fn load(&mut self) -> Result<(), PilError> {
        let loaded = match &*self {
            Image::Loaded(_) | Image::Paletted(_) => return Ok(()),
            Image::Bytes { format, info, .. } => image_from_materialized(
                self.materialized_shared()?,
                *format,
                info.clone(),
                self.exif_metadata(),
            )?,
            Image::Pipeline {
                source,
                ops,
                mode,
                format,
                palette,
                palette_data,
                palette_alpha,
                ..
            } => Self::pipeline_to_loaded(
                self,
                source,
                ops,
                mode,
                *format,
                palette,
                palette_data,
                palette_alpha,
            )?,
        };
        *self = loaded;
        Ok(())
    }

    /// Forces a lazy image through the architecture captured by its caller.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn load_with_execution_context(
        &mut self,
        context: crate::compute::ExecutionContext,
    ) -> Result<(), PilError> {
        let mut routed = self.clone().with_execution_context(context);
        routed.load()?;
        *self = routed;
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    /// Returns whether this handle can reuse successfully materialized pixels.
    pub fn is_materialized(&self) -> bool {
        match self {
            Image::Loaded(_) | Image::Paletted(_) => true,
            Image::Bytes { materialized, .. } | Image::Pipeline { materialized, .. } => {
                matches!(materialized.get(), Some(Ok(_)))
            }
        }
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    /// Returns whether this image still has an unopened encoded source and no core.
    ///
    /// Empty pipeline layers used only to carry metadata retain the source's
    /// unopened state. Pipelines with pixel operations do not.
    #[doc(hidden)]
    pub fn is_unloaded_encoded_source(&self) -> bool {
        match self {
            Image::Bytes { materialized, .. } => materialized.get().is_none(),
            Image::Pipeline {
                source,
                ops,
                materialized,
                ..
            } => {
                ops.is_empty()
                    && materialized.get().is_none()
                    && source.is_unloaded_encoded_source()
            }
            Image::Loaded(_) | Image::Paletted(_) => false,
        }
    }

    /// Fully validates an encoded or deferred image without changing its state.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when decoding or pipeline execution fails.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn verify(&self) -> Result<(), PilError> {
        match self {
            Image::Bytes { source, .. } => source
                .verify()
                .map_err(|error| map_codec_error(error, "memory"))?,
            Image::Pipeline {
                source,
                ops,
                mode,
                backend,
                palette,
                palette_alpha,
                palette_data,
                ..
            } => {
                let _origin_scope = ops.telemetry_origin_scope();
                Self::evaluate_pipeline_uncached(
                    source,
                    ops,
                    &declared_pipeline_mode(mode),
                    *backend,
                    palette,
                    palette_alpha,
                    palette_data,
                    ops.preserves_palette(),
                    ops.execution_context(),
                )?;
            }
            Image::Loaded(_) | Image::Paletted(_) => {}
        }
        Ok(())
    }
}
