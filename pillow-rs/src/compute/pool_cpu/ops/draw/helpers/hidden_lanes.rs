use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn restore<P>(
    image: &mut ImageBuffer<P, Vec<u8>>,
    lanes: Option<HiddenPixelLanes>,
) -> Result<(), PilError>
where
    P: Pixel<Subpixel = u8>,
{
    if image.replace_hidden_pixel_lanes(lanes) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "Draw hidden pixel lane extent did not match its image".into(),
        ))
    }
}
