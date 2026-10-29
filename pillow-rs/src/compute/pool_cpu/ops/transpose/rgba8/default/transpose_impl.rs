use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    image: &super::super::Input,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    #[cfg(target_arch = "x86_64")]
    if let Some(output) = super::rotate270_tiled::try_execute(image, method)? {
        return Ok(DynamicImage::ImageRgba8(output));
    }

    let (width, height) = image.dimensions();
    let output = super::super::super::helpers::pixel::transpose_bytes::<4>(
        image.as_raw(),
        width,
        height,
        method,
    )?
    .and_then(|(out_width, out_height, pixels)| {
        super::super::Input::from_raw(out_width, out_height, pixels)
    })
    .unwrap_or_else(|| super::super::super::helpers::pixel::transpose_samples(image, method));
    Ok(DynamicImage::ImageRgba8(output))
}
