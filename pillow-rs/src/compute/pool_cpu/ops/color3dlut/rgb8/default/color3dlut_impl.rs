//! Scalar Color3DLut kernel binding for `ImageRgb8` source mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) fn execute(
    image: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgb8(image) = image else {
        return Err(PilError::InternalError(
            "RGB Color3DLut received another raster mode".into(),
        ));
    };
    super::super::super::helpers::lut::apply_native_rgb_samples(
        image, size, table, channels, target_mode,
    )
}
