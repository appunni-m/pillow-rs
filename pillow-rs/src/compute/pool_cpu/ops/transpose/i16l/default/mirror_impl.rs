use crate::raster::DynamicImage;

pub(crate) fn execute(image: &super::super::Input) -> DynamicImage {
    let output = super::super::super::helpers::pixel::flip_horizontal_samples(image);
    DynamicImage::ImageI16L(output)
}
