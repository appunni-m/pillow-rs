//! Identity-only reducer for Pillow's bilevel `1` mode.
//!
//! Pillow copies full-image factor-one requests and crops factor-one sub-boxes.
//! Its native reduction kernel rejects non-identity requests for this mode.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(input: &crate::raster::GrayImage) -> Result<DynamicImage, PilError> {
    Ok(DynamicImage::One(input.clone()))
}
