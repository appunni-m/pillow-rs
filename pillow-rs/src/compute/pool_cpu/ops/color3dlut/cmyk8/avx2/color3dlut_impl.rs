use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) fn execute(
    image: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::color3dlut::helpers::lut_vector::apply(
        image,
        Mode::ImageCmyk8,
        size,
        table,
        channels,
        target_mode,
    )
}

