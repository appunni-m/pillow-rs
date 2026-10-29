use crate::raster::DynamicImage;

pub(crate) fn execute(image: &super::super::Input) -> DynamicImage {
    let (width, height) = image.dimensions();
    let output = super::super::super::helpers::pixel::flip_vertical_bytes::<3>(
        image.as_raw(), width, height,
    )
    .and_then(|pixels| super::super::Input::from_raw(width, height, pixels))
    .unwrap_or_else(|| super::super::super::helpers::pixel::flip_vertical_samples(image));
    DynamicImage::ImageYCbCr8(output)
}
