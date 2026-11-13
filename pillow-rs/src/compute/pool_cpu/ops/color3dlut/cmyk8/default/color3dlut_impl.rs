//! Scalar Color3DLut kernel binding for `ImageCmyk8` source mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageCmyk8(image) = image else {
        return Err(PilError::InternalError(
            "CMYK Color3DLut received another raster mode".into(),
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
