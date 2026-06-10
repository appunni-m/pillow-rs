use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn filter(&self, _filter_type: &str) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.filter".into()))
    }
}
