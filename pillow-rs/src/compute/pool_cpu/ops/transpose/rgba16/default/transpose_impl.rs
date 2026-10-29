use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;
use crate::error::PilError;

pub(crate) fn execute(image: &super::super::Input, method: &TransposeMethod) -> Result<DynamicImage, PilError> {
    let output = super::super::super::helpers::pixel::transpose_samples(image, method);
    Ok(DynamicImage::ImageRgba16(output))
}
