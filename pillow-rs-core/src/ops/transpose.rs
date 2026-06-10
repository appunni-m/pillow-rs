use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn transpose(&self, _method: &str) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.transpose".into()))
    }
}
