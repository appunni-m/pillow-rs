use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn split(&self) -> Result<Vec<Image>, PilError> {
        Err(PilError::NotImplementedError("Image.split".into()))
    }

    pub fn getbands(&self) -> Result<Vec<String>, PilError> {
        Err(PilError::NotImplementedError("Image.getbands".into()))
    }
}
