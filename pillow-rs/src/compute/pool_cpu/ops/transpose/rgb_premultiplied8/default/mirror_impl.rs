use crate::raster::DynamicImage;

pub(crate) fn execute(image: &super::super::Input) -> DynamicImage {
    let (width, height) = image.dimensions();
    let output = super::super::super::helpers::pixel::flip_horizontal_bytes::<4>(
        image.as_raw(), width, height,
    )
    .and_then(|pixels| super::super::Input::from_raw(width, height, pixels))
    .unwrap_or_else(|| super::super::super::helpers::pixel::flip_horizontal_samples(image));
    DynamicImage::ImageRgbPremultiplied8(output)
}
