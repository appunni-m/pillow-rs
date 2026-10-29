//! Serial bytewise inversion for this exact image mode.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(input: &super::super::Input) -> Result<DynamicImage, PilError> {
    let (width, height) = input.dimensions();
    let pixels = super::super::super::helpers::pixel::invert_bytes(input.as_raw());
    let output = super::super::Input::from_raw(width, height, pixels)
        .ok_or_else(|| PilError::InternalError("invert LA buffer shape mismatch".into()))?;
    Ok(DynamicImage::ImageLaPremultiplied8(output))
}
