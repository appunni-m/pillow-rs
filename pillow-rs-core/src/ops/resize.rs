use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn resize(&self, _size: (u32, u32), _filter: Option<&str>) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.resize".into()))
    }
}
