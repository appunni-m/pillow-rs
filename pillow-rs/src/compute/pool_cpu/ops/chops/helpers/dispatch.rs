//! Direct dispatch from the prepared Chops mode and strategy selection.

use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::super::ops::ChopsRequest;

pub(crate) fn execute(
    mode: Mode,
    image: &DynamicImage,
    request: ChopsRequest,
    other: Option<&DynamicImage>,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    macro_rules! execute_mode {
        ($owner:ident) => {
            super::super::$owner::ops::execute(image, request, other, selection)
        };
    }

    match mode {
        Mode::One => execute_mode!(one),
        Mode::ImageLuma8 => execute_mode!(luma8),
        Mode::ImageLumaA8 => execute_mode!(luma_a8),
        Mode::ImageLaPremultiplied8 => execute_mode!(la_premultiplied8),
        Mode::ImagePalette8 => execute_mode!(palette8),
        Mode::ImagePaletteAlpha8 => execute_mode!(palette_alpha8),
        Mode::ImageRgb8 => execute_mode!(rgb8),
        Mode::ImageRgba8 => execute_mode!(rgba8),
        Mode::ImageRgbPremultiplied8 => execute_mode!(rgb_premultiplied8),
        Mode::ImageRgbx8 => execute_mode!(rgbx8),
        Mode::ImageCmyk8 => execute_mode!(cmyk8),
        Mode::ImageYCbCr8 => execute_mode!(ycbcr8),
        Mode::ImageLab8 => execute_mode!(lab8),
        Mode::ImageHsv8 => execute_mode!(hsv8),
        Mode::ImageI32 => execute_mode!(i32),
        Mode::ImageF32 => execute_mode!(f32),
        Mode::ImageI16 => execute_mode!(i16),
        Mode::ImageI16L => execute_mode!(i16l),
        Mode::ImageI16B => execute_mode!(i16b),
        Mode::ImageI16N => execute_mode!(i16n),
        Mode::ImageLumaA16 => execute_mode!(luma_a16),
        Mode::ImageRgb16 => execute_mode!(rgb16),
        Mode::ImageRgba16 => execute_mode!(rgba16),
        Mode::ImageRgb32F => execute_mode!(rgb32f),
        Mode::ImageRgba32F => execute_mode!(rgba32f),
    }
}

pub(crate) fn execute_in_place(
    mode: Mode,
    image: &mut DynamicImage,
    request: ChopsRequest,
    other: &DynamicImage,
    selection: &StrategySelection,
) -> Result<(), PilError> {
    macro_rules! execute_mode {
        ($owner:ident) => {
            super::super::$owner::ops::execute_in_place(image, request, other, selection)
        };
    }

    match mode {
        Mode::One => execute_mode!(one),
        Mode::ImageLuma8 => execute_mode!(luma8),
        Mode::ImageLumaA8 => execute_mode!(luma_a8),
        Mode::ImagePalette8 => execute_mode!(palette8),
        Mode::ImagePaletteAlpha8 => execute_mode!(palette_alpha8),
        Mode::ImageRgb8 => execute_mode!(rgb8),
        Mode::ImageRgba8 => execute_mode!(rgba8),
        Mode::ImageRgbPremultiplied8 => execute_mode!(rgb_premultiplied8),
        Mode::ImageRgbx8 => execute_mode!(rgbx8),
        Mode::ImageCmyk8 => execute_mode!(cmyk8),
        Mode::ImageYCbCr8 => execute_mode!(ycbcr8),
        Mode::ImageHsv8 => execute_mode!(hsv8),
        _ => Err(PilError::InternalError(
            "Chops Add/Sub plan selected an unsupported native mode".into(),
        )),
    }
}
