//! Mode 1 resize and its Pillow-compatible binary thresholding step.

use crate::image::preserve_mode;
use crate::ops::pil_resize::pil_resize;
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

pub(in crate::compute::pool_cpu::ops::resize) fn resize_one(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
) -> DynamicImage {
    // Pillow uses the BOX filter for mode 1 when NEAREST is requested, then
    // thresholds the resampled byte back to binary at 128.
    let resized = pil_resize(image, width, height, *filter, Some("1")).to_luma8();
    let (width, height) = resized.dimensions();
    let mut output = crate::raster::GrayImage::new(width, height);
    for (output_pixel, input_pixel) in output.pixels_mut().zip(resized.pixels()) {
        output_pixel[0] = if input_pixel[0] >= 128 { 255 } else { 0 };
    }
    preserve_mode(image, DynamicImage::ImageLuma8(output))
}
