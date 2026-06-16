//! Channel split operations — pipelined via ExtractBand.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Image {
    /// Split the image into individual bands (pipelined via ExtractBand).
    /// Each output band is a lazy Pipeline image that extracts one channel on materialize.
    pub fn split(&self) -> Result<Vec<Image>, PilError> {
        // PIL: P-mode has 1 band → return a copy preserving mode + palette
        if let Image::Paletted(data) = self {
            return Ok(vec![Image::Paletted(data.clone())]);
        }

        // Determine band count from the image
        let img = self.materialize()?;
        let n_bands = match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => 1,
            image::ColorType::La8 | image::ColorType::La16 => 2,
            image::ColorType::Rgb8 | image::ColorType::Rgb16 => 3,
            _ => 4, // Rgba8, Rgba16, or fallback
        };

        // Create N pipeline images, each extracting one band
        let bands: Vec<Image> = (0..n_bands)
            .map(|i| {
                Image::push_op(
                    self,
                    PipelineOp::ExtractBand { index: i as u8 },
                )
            })
            .collect();

        Ok(bands)
    }
}
