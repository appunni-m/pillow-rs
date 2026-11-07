use crate::error::PilError;
use crate::raster::DynamicImage;

// Pillow's public Image.filter splits multiband input into L bands, applies
// ModeFilter to each, then merges; this mode-owned kernel fuses those passes.
pub(crate) fn execute(
    image: &DynamicImage,
    size: u32,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgbx8(input) = image else {
        return Err(PilError::InternalError(
            "ModeFilter mode strategy received a mismatched raster variant".into(),
        ));
    };
    crate::compute::pool_cpu::ops::filter::helpers::mode_filter::execute::<_, 4>(input, size)
        .map(DynamicImage::ImageRgbx8)
}
