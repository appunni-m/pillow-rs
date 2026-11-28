use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &super::super::Input,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    let output = super::super::super::helpers::pixel::transpose_samples(image, method);
    Ok(DynamicImage::ImageLumaA16(output))
}
