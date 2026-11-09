//! Exact-mode PutData value-at execution.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) fn execute_luma16_value_at(
    mode: Mode,
    image: &mut DynamicImage,
    pixel_index: usize,
    input: super::helpers::put_data_value_at::ValueAtInput,
) -> Result<(), PilError> {
    use super::helpers::put_data_value_at::ValueAtInput;

    match (mode, image, input) {
        (Mode::ImageI16, DynamicImage::ImageI16(image), ValueAtInput::Luma16(sample)) => {
            super::i16::default::put_data_impl::execute_value_at(image, pixel_index, sample)
        }
        (Mode::ImageI16L, DynamicImage::ImageI16L(image), ValueAtInput::Luma16(sample)) => {
            super::i16l::default::put_data_impl::execute_value_at(image, pixel_index, sample)
        }
        (Mode::ImageI16B, DynamicImage::ImageI16B(image), ValueAtInput::Luma16(sample)) => {
            super::i16b::default::put_data_impl::execute_value_at(image, pixel_index, sample)
        }
        (Mode::ImageI16N, DynamicImage::ImageI16N(image), ValueAtInput::Luma16(sample)) => {
            super::i16n::default::put_data_impl::execute_value_at(image, pixel_index, sample)
        }
        _ => Err(PilError::InternalError(
            "putdata value-at input does not match its selected mode".into(),
        )),
    }
}

macro_rules! native_byte_value_at_arms {
    ($mode:expr, $image:expr, $pixel_index:expr, $input:expr; $($variant:ident => $module:ident),+ $(,)?) => {
        match ($mode, $image, $input) {
            $(
                (
                    Mode::$variant,
                    DynamicImage::$variant(image),
                    super::helpers::put_data_value_at::ValueAtInput::Bytes(bytes, lanes),
                ) => super::$module::default::put_data_impl::execute_value_at(
                    image,
                    $pixel_index,
                    &bytes,
                    lanes,
                ),
            )+
            _ => Err(PilError::InternalError(
                "putdata value-at input does not match its selected mode".into(),
            )),
        }
    };
}

pub(crate) fn execute_byte_value_at(
    mode: Mode,
    image: &mut DynamicImage,
    pixel_index: usize,
    input: super::helpers::put_data_value_at::ValueAtInput,
) -> Result<(), PilError> {
    native_byte_value_at_arms!(mode, image, pixel_index, input;
        One => one,
        ImageLuma8 => luma8,
        ImageLumaA8 => luma_a8,
        ImageLaPremultiplied8 => la_premultiplied8,
        ImagePalette8 => palette8,
        ImagePaletteAlpha8 => palette_alpha8,
        ImageRgb8 => rgb8,
        ImageRgba8 => rgba8,
        ImageRgbPremultiplied8 => rgb_premultiplied8,
        ImageRgbx8 => rgbx8,
        ImageCmyk8 => cmyk8,
        ImageYCbCr8 => ycbcr8,
        ImageLab8 => lab8,
        ImageHsv8 => hsv8,
        ImageI32 => i32,
        ImageF32 => f32,
    )
}
