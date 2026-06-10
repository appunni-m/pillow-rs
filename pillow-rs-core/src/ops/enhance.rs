use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn enhance_brightness(&self, _factor: f64) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError(
            "ImageEnhance.Brightness".into(),
        ))
    }

    pub fn enhance_contrast(&self, _factor: f64) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError(
            "ImageEnhance.Contrast".into(),
        ))
    }

    pub fn enhance_color(&self, _factor: f64) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("ImageEnhance.Color".into()))
    }

    pub fn enhance_sharpness(&self, _factor: f64) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError(
            "ImageEnhance.Sharpness".into(),
        ))
    }
}
