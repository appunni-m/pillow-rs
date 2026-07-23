//! Channel split operations.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Image {
    /// Splits the image into one `L` image per band.
    ///
    /// `P` images return a single paletted clone. Other modes return lazy
    /// pipeline images that extract one channel when materialized.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization is needed to determine band
    /// count and that materialization fails.
    pub fn split(&self) -> Result<Vec<Image>, PilError> {
        // PIL: P-mode has 1 band → return a copy preserving mode + palette
        if let Image::Paletted(data) = self {
            return Ok(vec![Image::Paletted(data.clone())]);
        }

        // Determine band count from the image
        let img = self.materialize()?;
        let n_bands = match img.color() {
            image_slash_star::ColorType::L8 | image_slash_star::ColorType::L16 => 1,
            image_slash_star::ColorType::La8 | image_slash_star::ColorType::La16 => 2,
            image_slash_star::ColorType::Rgb8 | image_slash_star::ColorType::Rgb16 => 3,
            _ => 4, // Rgba8, Rgba16, or fallback
        };

        // Create N pipeline images, each extracting one band
        let bands: Vec<Image> = (0..n_bands)
            .map(|i| Image::push_op(self, PipelineOp::ExtractBand { index: i as u8 }))
            .collect();

        Ok(bands)
    }
}
