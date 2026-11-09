use super::super::{Draw, Image, Mode};
use crate::compute::pool_cpu::ops::draw::TextComposeResult;
use crate::compute::registry::support::PaletteKnowledge;
use crate::error::PilError;
use crate::raster::DynamicImage;

impl Draw {
    /// Replaces the wrapper raster while retaining source Image metadata.
    pub(super) fn apply_text_output(
        &mut self,
        mode: Mode,
        palette_state: PaletteKnowledge,
        output: TextComposeResult,
    ) -> Result<(), PilError> {
        match output {
            TextComposeResult::NotApplicable => Err(PilError::ValueError("mode mismatch".into())),
            TextComposeResult::Unchanged => Ok(()),
            TextComposeResult::Updated(image)
                if mode == Mode::ImagePalette8 && palette_state == PaletteKnowledge::Present =>
            {
                self.replace_paletted_text_output(image)
            }
            TextComposeResult::Updated(image) if mode == Mode::ImagePaletteAlpha8 => {
                self.replace_palette_alpha_output(image)
            }
            TextComposeResult::Updated(image) => {
                self.replace_draw_output(image);
                Ok(())
            }
        }
    }

    fn replace_paletted_text_output(&mut self, image: DynamicImage) -> Result<(), PilError> {
        let DynamicImage::ImagePalette8(indices) = image else {
            return Err(PilError::InternalError(
                "Draw Text P strategy returned another raster mode".into(),
            ));
        };
        let palette = self
            .image
            .palette()
            .filter(|palette| !palette.is_empty())
            .ok_or_else(|| {
                PilError::InternalError("prepared Draw Text palette disappeared".into())
            })?;
        let palette_alpha = self.image.palette_alpha().unwrap_or_default();
        let source_format = self.image.source_format();
        let info = self.image.image_info();
        let exif = self.image.exif_metadata();
        self.image = Image::Paletted(crate::image::PalettedData {
            indices: DynamicImage::ImagePalette8(indices),
            palette,
            palette_alpha,
            source_format,
            info,
            exif,
            materialized: crate::image::materialization_cache(),
        });
        Ok(())
    }
}
