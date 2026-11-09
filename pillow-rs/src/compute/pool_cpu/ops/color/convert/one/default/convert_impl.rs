//! Serial conversion from bilevel samples using Pillow's expanded-byte rules.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

pub(crate) fn execute(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        return super::super::super::helpers::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(|pixel| {
                let sample = if pixel[0] == 0 { 0 } else { u8::MAX };
                [sample; 3]
            }),
        );
    }
    let samples = source
        .pixels()
        .map(|pixel| if pixel[0] == 0 { 0 } else { u8::MAX });
    super::super::super::helpers::luma8::execute(source.width(), source.height(), target, samples)
}

// The `1` source/target matrix uses expanded 0/255 palette values; write the
// final palette output directly from this mode-owned path.
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
