//! Native I sample transforms for the Default architecture.

use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn affine(image: &mut RgbaImage, scale: f64, offset: f64) {
    for pixel in image.pixels_mut() {
        let value = i32::from_le_bytes(pixel.0);
        pixel.0 = ((f64::from(value) * scale + offset) as i32).to_le_bytes();
    }
}

pub(crate) fn callable<F>(image: &mut RgbaImage, callback: &mut F) -> Result<(), PilError>
where
    F: FnMut(f64) -> Result<f64, PilError>,
{
    for pixel in image.pixels_mut() {
        let value = f64::from(i32::from_le_bytes(pixel.0));
        pixel.0 = (callback(value)? as i32).to_le_bytes();
    }
    Ok(())
}
