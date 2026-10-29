//! Generic native-format resize paths for modes without a typed CPU kernel.

use crate::image::Mode;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{pil_resize, pil_resize_boxed};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

pub(in crate::compute::pool_cpu::ops::resize) fn resize(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
    mode: Mode,
) -> DynamicImage {
    preserve_mode(
        image,
        pil_resize(image, width, height, *filter, mode.pillow_name()),
    )
}

pub(in crate::compute::pool_cpu::ops::resize) fn resize_boxed(
    image: &DynamicImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
    mode: Mode,
) -> DynamicImage {
    resize_boxed_with_name(image, width, height, bounds, filter, mode.pillow_name())
}

pub(in crate::compute::pool_cpu::ops::resize) fn resize_boxed_raw(
    image: &DynamicImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> DynamicImage {
    resize_boxed_with_name(image, width, height, bounds, filter, None)
}

fn resize_boxed_with_name(
    image: &DynamicImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
    mode: Option<&str>,
) -> DynamicImage {
    pil_resize_boxed(
        image,
        width,
        height,
        bounds.0,
        bounds.1,
        bounds.2,
        bounds.3,
        filter,
        mode,
    )
}
