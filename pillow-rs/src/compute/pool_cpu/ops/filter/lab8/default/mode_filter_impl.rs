use crate::error::PilError;
use crate::raster::DynamicImage;

// NO_PILLOW_SUPPORT: Pillow ModeFilter is single-band; this exact-mode
// core strategy applies the native-byte algorithm to every mode channel.
pub(crate) fn execute(
    image: &DynamicImage,
    size: u32,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLab8(input) = image else {
        return Err(PilError::InternalError(
            "ModeFilter mode strategy received a mismatched raster variant".into(),
        ));
    };
    crate::compute::pool_cpu::ops::filter::helpers::mode_filter::execute::<_, 3>(input, size)
        .map(DynamicImage::ImageLab8)
}
