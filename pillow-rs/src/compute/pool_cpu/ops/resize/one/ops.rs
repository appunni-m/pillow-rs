//! `1`-mode orchestration, including Pillow's binary thresholding behavior.

use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

pub(crate) use super::kernels::default::resize_one;

pub(super) fn resize(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
) -> DynamicImage {
    resize_one(image, width, height, filter)
}
