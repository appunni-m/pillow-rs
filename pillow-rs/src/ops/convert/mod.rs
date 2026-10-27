//! Pillow-compatible image mode conversion operations.

mod binary;
mod dispatch;
mod input;
mod luma16;
mod mode;
mod nonstandard;
mod operation;
mod palette;
mod support;

#[cfg(test)]
mod tests;

pub use input::{
    ConvertModeInput, ConvertPaletteInput, DitherInput, normalize_convert_dither,
    validate_convert_dither,
};
pub use mode::parse_mode;
