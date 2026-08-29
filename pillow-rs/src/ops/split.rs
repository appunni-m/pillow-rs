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
        // PIL: P-mode has one band, and split() returns a copy preserving the
        // indexed mode and palette.  Encoded P inputs are still represented as
        // lazy Image::Bytes until their first operation, so checking only the
        // concrete Paletted variant would lose the mode and turn the result
        // into L during the ExtractBand path.
        if self.has_palette_mode() {
            let mut band = self.materialized_branch()?;
            // Pillow-derived images do not retain the source container's
            // ``format`` field, even when the split band stays indexed.
            match &mut band {
                Image::Loaded(data) => data.source_format = None,
                Image::Paletted(data) => data.source_format = None,
                _ => {}
            }
            return Ok(vec![band]);
        }

        // Determine band count from the image
        let img = self.materialize()?;
        let n_bands = match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
            _ => 4, // Rgba8, Rgba16, or fallback
        };

        // Create N pipeline images, each extracting one band
        let bands: Vec<Image> = (0..n_bands)
            .map(|i| Image::push_op(self, PipelineOp::ExtractBand { index: i as u8 }))
            .collect();

        Ok(bands)
    }
}
