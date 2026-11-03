use super::{
    DynamicImage, Image, LoadedData, PalettedData, PilError, declared_pipeline_mode,
    expand_palette,
    image_from_materialized, map_codec_error, materialization_cache,
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
        if matches!(self, Image::Pipeline { .. }) && self.known_mode_identity() == Some(crate::raster::Mode::ImagePalette8) {
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

    fn pipeline_to_loaded(
        image_handle: &Image,
        source: &Image,
        ops: &super::super::PipelineOps,
        mode: &super::super::PipelineModeCache,
        format: Option<image_slash_star::ImageFormat>,
        palette: &Option<Vec<u8>>,
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
                        crate::compute::pool_cpu::ops::color::convert::ops::ConvertWrapperMetadata::PreserveSourceLoaded
                    ) =>
                {
                    Ok(Image::Loaded(LoadedData {
                        image,
                        known_zero_fill: false,
                        palette: output.palette.clone(),
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
                        palette: retained_palette,
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
            let Some(palette) = palette else {
                return Err(PilError::PaletteError(
                    "P-mode pipeline has no retained palette".to_owned(),
                ));
            };
            return Ok(Image::Paletted(PalettedData {
                indices: DynamicImage::ImagePalette8(image.to_luma8()),
                palette: palette.clone(),
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
                palette_alpha,
                ..
            } => Self::pipeline_to_loaded(self, source, ops, mode, *format, palette, palette_alpha)?,
        };
        *self = loaded;
        Ok(())
    }

    /// Returns whether this handle can reuse successfully materialized pixels.
    pub fn is_materialized(&self) -> bool {
        match self {
            Image::Loaded(_) | Image::Paletted(_) => true,
            Image::Bytes { materialized, .. } | Image::Pipeline { materialized, .. } => {
                matches!(materialized.get(), Some(Ok(_)))
            }
        }
    }

    /// Fully validates an encoded or deferred image without changing its state.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when decoding or pipeline execution fails.
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
                    ops.preserves_palette(),
                )?;
            }
            Image::Loaded(_) | Image::Paletted(_) => {}
        }
        Ok(())
    }
}
