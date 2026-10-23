//! Pillow-compatible color parsing and color-space conversion.
//!
//! This module accepts Rust primitives and Pillow-owned raster buffers, then
//! applies the same mode-aware color behavior that Pillow exposes through
//! `ImageColor`, `Image.new`, and `Image.convert`.
//!
//! # Input Conventions
//!
//! - Mode values are Pillow mode strings such as `"L"`, `"RGB"`, `"RGBA"`,
//!   `"CMYK"`, `"HSV"`, `"YCbCr"`, `"I"`, and `"F"`.
//! - Multi-byte scalar modes are represented in little-endian byte order inside
//!   RGBA-like storage when this crate needs to carry them through
//!   Pillow raster buffers.
//! - CMYK is stored in an RGBA buffer as `C`, `M`, `Y`, `K` channels.
//! - HSV and YCbCr are stored in RGB buffers using their channel names in order.
//!
//! # Output Conventions
//!
//! Conversion helpers return concrete Pillow image buffers or
//! [`crate::raster::DynamicImage`] values with tightly packed rows. Color
//! resolver helpers return `(r, g, b, a)` tuples even when the destination mode
//! will later use only one or two channels.

mod conversion;
mod grayscale;
mod mode;
mod palette;
mod parse;
mod resolve;

#[cfg(test)]
mod tests;

pub(crate) use conversion::{
    cmyk_to_rgb, convert_from_nonstandard, f_to_i, f_to_l, f_to_rgb, hsv_to_rgb, i_to_f, i_to_l,
    i_to_rgb, luma_to_ycbcr, muldiv255, rgb_to_cmyk_inverse, rgb_to_hsv, rgb_to_ycbcr,
};
pub(crate) use grayscale::{
    cmyk_to_binary_truncate, cmyk_to_grayscale, cmyk_to_grayscale_truncate, pil_grayscale,
    pil_grayscale_alpha, pil_grayscale_truncate, rgb_to_luma_u8,
};
#[cfg(target_arch = "x86_64")]
pub(crate) use grayscale::{
    gather_interleaved_channel_ssse3_bytes, gather_interleaved_channel_ssse3_into,
};
pub(crate) use mode::color_type_to_mode;
pub use palette::{
    PaletteColorInput, palette_getcolor, palette_getcolor_append, palette_getcolor_validate,
    palette_getcolor_validate_input, palette_to_text,
};
pub use parse::{color_has_explicit_alpha, parse_color_str, parse_color_str_unclamped};
pub use resolve::{ColorValue, getcolor, resolve_new_color};
