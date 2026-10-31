//! Serial conversion from bilevel samples using Pillow's expanded-byte rules.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

pub(crate) fn execute(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    let samples = source
        .pixels()
        .map(|pixel| if pixel[0] == 0 { 0 } else { u8::MAX });
    super::super::super::helpers::luma8::execute(
        source.width(),
        source.height(),
        target,
        samples,
    )
}

pub(crate) fn execute_palette_target(
    source: &GrayImage,
    target: Mode,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    let samples = source
        .pixels()
        .map(|pixel| if pixel[0] == 0 { 0 } else { u8::MAX });
    super::super::super::helpers::luma8::execute_palette_target(
        source.width(),
        source.height(),
        target,
        samples,
    )
}
