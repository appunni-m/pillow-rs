//! Pillow `ImageEnhance`-style adjustment methods.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

fn validate_mode(image: &Image, palette_rejects: bool) -> Result<(), PilError> {
    let mode = image.mode()?;
    if mode == "1" {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    if palette_rejects && mode == "P" {
        return Err(PilError::ValueError("cannot filter palette images".into()));
    }
    if mode == "P" {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    Ok(())
}

impl Image {
    /// Adjusts brightness by `factor`.
    ///
    /// `1.0` is unchanged and `0.0` produces black.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn enhance_brightness(&self, factor: f64) -> Result<Image, PilError> {
        validate_mode(self, false)?;
        Ok(Image::push_op(self, PipelineOp::Brightness { factor }))
    }

    /// Adjusts contrast by `factor`.
    ///
    /// `1.0` is unchanged and `0.0` produces a solid gray image.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn enhance_contrast(&self, factor: f64) -> Result<Image, PilError> {
        validate_mode(self, false)?;
        Ok(Image::push_op(self, PipelineOp::Contrast { factor }))
    }

    /// Adjusts color saturation by `factor`.
    ///
    /// `1.0` is unchanged and `0.0` produces grayscale.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn enhance_color(&self, factor: f64) -> Result<Image, PilError> {
        validate_mode(self, false)?;
        Ok(Image::push_op(self, PipelineOp::ColorSaturation { factor }))
    }

    /// Adjusts sharpness by `factor`.
    ///
    /// `1.0` is unchanged, values below `1.0` blur, and values above `1.0`
    /// sharpen.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn enhance_sharpness(&self, factor: f64) -> Result<Image, PilError> {
        validate_mode(self, true)?;
        Ok(Image::push_op(self, PipelineOp::Sharpness { factor }))
    }
}
