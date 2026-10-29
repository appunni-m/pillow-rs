use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;
use crate::error::PilError;

pub(crate) fn execute(image: &super::super::Input, method: &TransposeMethod) -> Result<DynamicImage, PilError> {
    let (width, height) = image.dimensions();
    let output = super::super::super::helpers::pixel::transpose_bytes::<1>(
        image.as_raw(), width, height, method,
    )?
    .and_then(|(out_width, out_height, pixels)| {
        super::super::Input::from_raw(out_width, out_height, pixels)
    })
    .unwrap_or_else(|| super::super::super::helpers::pixel::transpose_samples(image, method));
    Ok(DynamicImage::ImageLuma8(output))
}
