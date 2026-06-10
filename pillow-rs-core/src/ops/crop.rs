use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn crop(&self, _box_coords: (u32, u32, u32, u32)) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.crop".into()))
    }
}
