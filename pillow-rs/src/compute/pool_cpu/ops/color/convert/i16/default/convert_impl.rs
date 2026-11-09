//! Direct conversions from native-endian unsigned 16-bit samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode};

type Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

pub(crate) fn execute(source: &Samples, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        return super::super::super::helpers::lab_target::execute(
            source.width(),
            source.height(),
            source
                .pixels()
                .map(|pixel| [super::super::super::helpers::luma16::clip(pixel[0]); 3]),
        );
    }
    super::super::super::helpers::luma16::execute_mode(source, Mode::ImageI16, target)
}
