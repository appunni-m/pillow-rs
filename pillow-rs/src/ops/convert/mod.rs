//! Pillow-compatible image mode conversion operations.

mod binary;
mod dispatch;
mod input;
mod luma16;
mod mode;
mod operation;
mod palette;

pub(crate) fn palette_alpha_for_lazy_convert(image: &crate::image::Image) -> Option<Vec<u8>> {
    palette::palette_alpha_for_convert(image)
}

pub use input::{
    ConvertModeInput, ConvertPaletteInput, DitherInput, normalize_convert_dither,
    validate_convert_dither,
};
