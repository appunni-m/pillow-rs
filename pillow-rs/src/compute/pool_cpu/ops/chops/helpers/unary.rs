//! Serial native-mode ImageChops unary strategies.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Luma, Mode};

pub(crate) fn constant(image: &DynamicImage, value: u8) -> DynamicImage {
    // NO_PILLOW_SUPPORT for internal high-depth sources: this core-only route
    // accepts representations that Pillow does not expose as modes.
    DynamicImage::ImageLuma8(GrayImage::from_pixel(
        image.width(),
        image.height(),
        Luma([value]),
    ))
}

pub(crate) fn offset(image: &DynamicImage, x: i32, y: i32) -> DynamicImage {
    // NO_PILLOW_SUPPORT for internal high-depth sources: preserve their exact
    // native representation through this core-only route.
    image.offset_with_mode(x, y)
}

pub(crate) fn duplicate(image: &DynamicImage) -> DynamicImage {
    // NO_PILLOW_SUPPORT for internal high-depth sources: preserve their exact
    // native identity through this core-only route.
    image.clone()
}

pub(crate) fn invert(image: &DynamicImage, mode: Mode) -> Result<DynamicImage, PilError> {
    // Internal high-depth identities use this direct native-byte extension;
    // they are not Pillow ImageChops mode contracts.
    let bytes = image.as_bytes().iter().map(|byte| 255 - byte).collect();
    super::output::from_raw_bytes(mode, image.width(), image.height(), bytes)
        .ok_or_else(|| PilError::InternalError("invert Chops buffer shape mismatch".into()))
}
