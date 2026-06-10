use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn convert(
        &self,
        _mode: &str,
        _matrix: Option<Vec<f64>>,
        _dither: Option<&str>,
        _palette: Option<&str>,
        _colors: Option<u32>,
    ) -> Result<Image, PilError> {
        Err(PilError::NotImplementedError("Image.convert".into()))
    }
}
