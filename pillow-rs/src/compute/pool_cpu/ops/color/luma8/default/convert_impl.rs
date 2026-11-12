//! Serial conversions from native L samples without RGB expansion.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

pub(crate) fn execute(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        return crate::compute::pool_cpu::ops::color::helpers::convert::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(|pixel| [pixel[0]; 3]),
        );
    }
    crate::compute::pool_cpu::ops::color::helpers::convert::luma8::execute(
        source.width(),
        source.height(),
        target,
        source.pixels().map(|pixel| pixel[0]),
    )
}

pub(crate) fn execute_palette_target(
    source: &GrayImage,
    target: Mode,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    crate::compute::pool_cpu::ops::color::helpers::convert::luma8::execute_palette_target(
        source.width(),
        source.height(),
        target,
        source.pixels().map(|pixel| pixel[0]),
    )
}
