//! Native I;16B sample transforms for the Default architecture.

use crate::compute::pool_cpu::ops::eval::helpers::scalar::{self, U16LumaImage};
use crate::error::PilError;

pub(crate) fn affine(image: &mut U16LumaImage, scale: f64, offset: f64) -> Result<(), PilError> {
    scalar::map_u16(
        image,
        |value| Ok((f64::from(value) * scale + offset) as u16),
    )
}

pub(crate) fn callable<F>(image: &mut U16LumaImage, callback: &mut F) -> Result<(), PilError>
where
    F: FnMut(f64) -> Result<f64, PilError>,
{
    scalar::map_u16(image, |value| {
        callback(f64::from(value)).map(|sample| sample as u16)
    })
}
