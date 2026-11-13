use crate::compute::pool_cpu::ops::analysis::helpers::get_data as helpers;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image;
use crate::error::PilError;
use crate::raster::FromPrimitive;
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
// NO_PILLOW_SUPPORT: this internal high-depth raster exposes native samples through GetData.
pub(crate) fn get_data(image: &Rgb16Image) -> Result<Vec<u8>, PilError> {
    helpers::narrow_native_samples(
        image.as_raw(),
        image.width(),
        image.height(),
        3,
        Mode::ImageRgb16,
        u8::from_primitive,
    )
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
// NO_PILLOW_SUPPORT: preserve this internal mode's native sample layout for band reads.
pub(crate) fn write_band(
    image: &Rgb16Image,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    helpers::write_native_sample_band(
        image.as_raw(),
        image.width(),
        image.height(),
        3,
        band,
        Mode::ImageRgb16,
        output,
        u8::from_primitive,
    )
}
