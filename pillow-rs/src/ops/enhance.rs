//! ImageEnhance — brightness, contrast, color, and sharpness adjustments.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Image {
    /// Adjust brightness by factor. 1.0 = unchanged, 0.0 = black.
    pub fn enhance_brightness(&self, factor: f64) -> Result<Image, PilError> {
        Ok(Image::push_op(self, PipelineOp::Brightness { factor }))
    }

    /// Adjust contrast by factor. 1.0 = unchanged, 0.0 = solid gray.
    pub fn enhance_contrast(&self, factor: f64) -> Result<Image, PilError> {
        Ok(Image::push_op(self, PipelineOp::Contrast { factor }))
    }

    /// Adjust color saturation by factor. 1.0 = unchanged, 0.0 = grayscale.
    pub fn enhance_color(&self, factor: f64) -> Result<Image, PilError> {
        Ok(Image::push_op(self, PipelineOp::ColorSaturation { factor }))
    }

    /// Adjust sharpness by factor. 1.0 = unchanged, <1.0 = blur, >1.0 = sharpen.
    pub fn enhance_sharpness(&self, factor: f64) -> Result<Image, PilError> {
        Ok(Image::push_op(self, PipelineOp::Sharpness { factor }))
    }
}
