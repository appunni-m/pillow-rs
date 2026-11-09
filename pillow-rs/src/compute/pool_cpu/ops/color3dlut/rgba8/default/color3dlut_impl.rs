//! Scalar Color3DLut kernel binding for `ImageRgba8` source mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) fn execute(
    image: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(image) = image else {
        return Err(PilError::InternalError(
            "RGBA Color3DLut received another raster mode".into(),
        ));
    };
    super::super::super::helpers::lut::apply_native_rgba_samples(
        image,
        size,
        table,
        channels,
        target_mode,
    )
}
