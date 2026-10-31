use crate::compute::pool_cpu::ops::analysis::helpers::get_data as helpers;
use crate::error::PilError;
use crate::raster::{Rgba32FImage};
use crate::raster::FromPrimitive;
use crate::raster::Mode;

// NO_PILLOW_SUPPORT: this internal high-depth raster exposes native samples through GetData.
pub(crate) fn get_data(image: &Rgba32FImage) -> Result<Vec<u8>, PilError> {
    helpers::narrow_native_samples(
        image.as_raw(),
        image.width(),
        image.height(),
        4,
        Mode::ImageRgba32F,
        u8::from_primitive,
    )
}

// NO_PILLOW_SUPPORT: preserve this internal mode's native sample layout for band reads.
pub(crate) fn write_band(
    image: &Rgba32FImage,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    helpers::write_native_sample_band(
        image.as_raw(),
        image.width(),
        image.height(),
        4,
        band,
        Mode::ImageRgba32F,
        output,
        u8::from_primitive,
    )
}
