//! Image quantization — reduce color palette.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Image {
    /// Reduce the number of colors in the image using median cut or NeuQuant.
    /// PIL-compatible: `quantize(colors=256, method=None, kmeans=0, palette=None, dither=1)`.
    pub fn quantize(
        &self,
        colors: u32,
        _kmeans: u32,
        _palette: Option<&Image>,
        _dither: bool,
    ) -> Result<Image, PilError> {
        let colors = colors.clamp(2, 256);
        Ok(Image::push_op(
            self,
            PipelineOp::Quantize { colors, dither: _dither },
        ))
    }
}
