use std::sync::Arc;

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::{Image, LoadedData, PalettedData, materialization_cache};

impl Image {
    /// Builds a materialized image from a mode-preserving callback result.
    ///
    /// Callback operations retain palette and wrapper metadata while dropping
    /// the encoded source format, matching Pillow's `Image._new` path.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn from_derived_dynamic(&self, image: DynamicImage) -> Result<Image, PilError> {
        let mode = self.mode_identity()?;
        if image.mode() != mode {
            return Err(PilError::InternalError(
                "mode-preserving callback changed its raster identity".into(),
            ));
        }
        self.from_mode_changed_dynamic(image)
    }

    /// Builds a materialized result with a new mode and Pillow `_new` metadata.
    /// Preserve `info`, EXIF, and a palette only for indexed P/PA outputs;
    /// operation-created images do not retain the encoded source format.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn from_mode_changed_dynamic(&self, image: DynamicImage) -> Result<Image, PilError> {
        let mode = image.mode();
        let info = self.image_info();
        let exif = self.exif_metadata();
        let palette = matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            .then(|| self.palette().unwrap_or_default());
        let palette_alpha =
            (mode == Mode::ImagePaletteAlpha8).then(|| self.palette_alpha().unwrap_or_default());
        if mode == Mode::ImagePalette8 {
            let DynamicImage::ImagePalette8(_) = &image else {
                return Err(PilError::InternalError(
                    "mode-changing P result has a non-index carrier".into(),
                ));
            };
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
        if mode == Mode::ImagePaletteAlpha8
            && !matches!(&image, DynamicImage::ImagePaletteAlpha8(_))
        {
            return Err(PilError::InternalError(
                "mode-changing PA result has a non-palette carrier".into(),
            ));
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
