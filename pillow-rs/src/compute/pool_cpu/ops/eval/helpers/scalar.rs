//! Shared native u16 sample mapping for the four exact Pillow I;16 identities.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Luma};

pub(crate) type U16LumaImage = ImageBuffer<Luma<u16>, Vec<u16>>;

pub(crate) fn map_u16<F>(image: &mut U16LumaImage, mut map: F) -> Result<(), PilError>
where
    F: FnMut(u16) -> Result<u16, PilError>,
{
    for pixel in image.pixels_mut() {
        pixel.0[0] = map(pixel.0[0])?;
    }
    Ok(())
}
