//! Native LAB8 identity conversion; cross-space conversion is explicitly pending.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, GenericImageView, Mode, RgbImage};

pub(crate) fn execute(source: &RgbImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLab8 => Ok(DynamicImage::ImageLab8(source.clone())),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::super::helpers::luma16::from_luma8_samples(
                source.width(),
                source.height(),
                target,
                source.pixels().map(|pixel| {
                    crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2])
                }),
            )
        }
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

pub(crate) fn execute_palette_target(
    source: &RgbImage,
    target: Mode,
    dither: Option<DitherMethod>,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    if !matches!(target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let metadata = if target == Mode::ImagePaletteAlpha8 {
        super::super::super::ops::ConvertWrapperMetadata::ExifOnly
    } else {
        super::super::super::ops::ConvertWrapperMetadata::PreserveSource
    };
    crate::compute::pool_cpu::ops::color::convert::helpers::palette::web_palette_from_pixels(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| pixel.0),
        !matches!(dither, Some(DitherMethod::None)),
        target,
        metadata,
    )
}
