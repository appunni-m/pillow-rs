use std::sync::Arc;

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::{Image, LoadedData, PalettedData, materialization_cache};

impl Image {
    /// Builds a materialized image from a mode-preserving callback result.
    ///
    /// Callback operations retain palette and wrapper metadata while dropping
    /// the encoded source format, matching Pillow's `Image._new` path.
    pub(crate) fn from_derived_dynamic(&self, image: DynamicImage) -> Result<Image, PilError> {
        let mode = self.mode_identity()?;
        if image.mode() != mode {
            return Err(PilError::InternalError(
                "mode-preserving callback changed its raster identity".into(),
            ));
        }
        let info = self.image_info();
        let exif = self.exif_metadata();
        let palette = self.palette();
        let palette_alpha = self.palette_alpha();
        if mode == Mode::ImagePalette8 {
            if !matches!(&image, DynamicImage::ImagePalette8(_)) {
                return Err(PilError::InternalError(
                    "palette callback result has a non-index carrier".into(),
                ));
            }
            return Ok(Image::Paletted(PalettedData {
                indices: image,
                palette: palette.unwrap_or_default(),
                palette_alpha: palette_alpha.unwrap_or_default(),
                source_format: None,
                info,
                exif,
                materialized: materialization_cache(),
            }));
        }
        Ok(Image::Loaded(LoadedData {
            image: Arc::new(image),
            known_zero_fill: false,
            palette,
            palette_alpha,
            source_format: None,
            info,
            exif,
        }))
    }
}
