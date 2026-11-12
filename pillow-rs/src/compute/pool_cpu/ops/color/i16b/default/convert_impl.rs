//! Direct conversions from big-endian unsigned 16-bit samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode};

type Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

pub(crate) fn execute(source: &Samples, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        return crate::compute::pool_cpu::ops::color::helpers::convert::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(|pixel| {
                [crate::compute::pool_cpu::ops::color::helpers::convert::luma16::clip(pixel[0]); 3]
            }),
        );
    }
    crate::compute::pool_cpu::ops::color::helpers::convert::luma16::execute_mode(source, target)
}
