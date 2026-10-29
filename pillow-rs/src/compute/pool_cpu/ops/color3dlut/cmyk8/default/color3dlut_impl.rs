//! Scalar Color3DLut kernel binding for `ImageCmyk8` source mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) fn execute(
    image: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    super::super::super::helpers::lut::apply(
        image, Mode::ImageCmyk8, size, table, channels, target_mode,
    )
}
