//! RGB output materialization for the shared four-byte Color3DLUT kernel.

use crate::raster::{RgbImage, RgbaImage};

pub(in crate::compute::pool_cpu::ops::color3dlut) fn project_rgb(rgba: RgbaImage) -> RgbImage {
    let (width, height) = rgba.dimensions();
    RgbImage::from_fn(width, height, |x, y| {
        let pixel = rgba.get_pixel(x, y);
        crate::raster::Rgb([pixel[0], pixel[1], pixel[2]])
    })
}
