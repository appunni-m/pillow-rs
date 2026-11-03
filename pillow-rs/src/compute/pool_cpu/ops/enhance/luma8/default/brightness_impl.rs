use crate::compute::pool_cpu::ops::enhance::helpers::native;
use crate::error::PilError;
use crate::raster::GrayImage;

pub(crate) fn execute(source: &GrayImage, factor: f64) -> Result<GrayImage, PilError> {
    let alpha = factor as f32;
    if alpha == 1.0 {
        return Ok(source.clone());
    }

    let mut output = source.clone();
    let shift = match alpha {
        0.5 => Some(1),
        0.25 => Some(2),
        0.125 => Some(3),
        _ => None,
    };
    if let Some(shift) = shift {
        // These float32 factors produce the same truncating result as Pillow.
        for sample in output.as_mut() {
            *sample >>= shift;
        }
    } else {
        for sample in output.as_mut() {
            *sample = native::blend_u8(0, *sample, alpha);
        }
    }
    Ok(output)
}
