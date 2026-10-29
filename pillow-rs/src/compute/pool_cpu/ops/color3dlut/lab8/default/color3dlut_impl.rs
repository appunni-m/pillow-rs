//! Scalar native-sample Color3DLut for `ImageLab8`.

use crate::compute::pool_cpu::ops::color3dlut::helpers::lut;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

// NO_PILLOW_SUPPORT: This applies the table to stored LAB samples directly,
// without converting the raster to RGB.
pub(crate) fn execute(
    image: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLab8(image) = image else {
        return Err(PilError::InternalError(
            "LAB Color3DLut received another raster mode".into(),
        ));
    };
    lut::apply_native_rgb_samples(
        image,
        Mode::ImageLab8,
        size,
        table,
        channels,
        target_mode,
    )
}

