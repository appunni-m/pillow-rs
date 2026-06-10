use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn rotate(
        &self,
        _angle: f64,
        _expand: bool,
        _fillcolor: Option<(u8, u8, u8, u8)>,
    ) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.rotate".into()))
    }
}
