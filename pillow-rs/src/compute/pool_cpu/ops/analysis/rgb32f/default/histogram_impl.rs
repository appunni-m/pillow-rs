use crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples;
use crate::error::PilError;
use crate::raster::Rgb32FImage;

// NO_PILLOW_SUPPORT: native scan preserves RGB32F's four-band result layout.
pub(crate) fn execute(
    image: &Rgb32FImage,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    Ok(histogram_samples::f32_rgb_bands_with_opaque_alpha(
        image, mask_raw,
    ))
}
