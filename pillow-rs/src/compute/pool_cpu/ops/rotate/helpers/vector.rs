//! Selects the exact native-byte vector kernel after strategy admission.

use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn execute<P, const CHANNELS: usize, const FORCE_NEAREST: bool, const ALPHA: bool>(
    image: &ImageBuffer<P, Vec<u8>>,
    request: RotateRequest,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    if FORCE_NEAREST || request.nearest {
        super::vector_nearest::execute::<P, CHANNELS>(image, request)
    } else {
        super::vector_bilinear::execute::<P, CHANNELS, ALPHA>(image, request)
    }
}
