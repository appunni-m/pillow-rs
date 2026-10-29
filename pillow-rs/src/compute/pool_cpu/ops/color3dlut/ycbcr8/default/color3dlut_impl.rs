//! Scalar native-sample Color3DLut for `ImageYCbCr8`.

use crate::compute::pool_cpu::ops::color3dlut::helpers::lut;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

// NO_PILLOW_SUPPORT: This applies the table to stored YCbCr samples directly,
// without converting the raster to RGB.
pub(crate) fn execute(
    image: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageYCbCr8(image) = image else {
        return Err(PilError::InternalError(
            "YCbCr Color3DLut received another raster mode".into(),
        ));
    };
    lut::apply_native_rgb_samples(
        image,
        Mode::ImageYCbCr8,
        size,
        table,
        channels,
        target_mode,
    )
}
