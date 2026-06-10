use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn paste(
        &mut self,
        _source_r: u8,
        _source_g: u8,
        _source_b: u8,
        _source_a: u8,
        _box_coords: Option<(i32, i32, i32, i32)>,
        _mask: Option<&Image>,
    ) -> Result<(), PilError> {
        Err(PilError::NotImplementedError("Image.paste".into()))
    }
}
