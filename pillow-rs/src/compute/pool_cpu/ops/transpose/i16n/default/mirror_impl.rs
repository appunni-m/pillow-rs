use crate::raster::DynamicImage;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(image: &super::super::Input) -> DynamicImage {
    let output = super::super::super::helpers::pixel::flip_horizontal_samples(image);
    DynamicImage::ImageI16N(output)
}
