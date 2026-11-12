//! Pillow-compatible color parsing and color-space conversion.
//!
//! This module accepts Rust primitives and Pillow-owned raster buffers, then
//! applies the same mode-aware color behavior that Pillow exposes through
//! `ImageColor`, `Image.new`, and `Image.convert`.
//!
//! # Input Conventions
//!
//! - Raster operations use the Pillow-owned [`crate::raster::Mode`] enum.
//!   `ImageColor.getcolor` uses [`ColorOutputMode`] because Pillow also accepts
//!   non-raster descriptors such as `I;32` and `I;16S`.
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
mod palette;
mod parse;
mod photo_ycc;
mod resolve;

pub(crate) use conversion::{convert_from_nonstandard, muldiv255, rgb_to_cmyk_inverse};
pub(crate) use grayscale::{
    cmyk_to_binary_truncate, cmyk_to_grayscale_truncate, grayscale_rgb_bytes, grayscale_rgb_pixel,
    pil_grayscale_truncate, rgb_to_luma_u8,
};
pub use palette::{
    PaletteColorInput, palette_getcolor, palette_getcolor_append, palette_getcolor_validate,
    palette_getcolor_validate_input, palette_to_text,
};
pub use parse::{color_has_explicit_alpha, parse_color_str, parse_color_str_unclamped};
pub(crate) use resolve::getcolor_for_mode;
pub use resolve::{ColorOutputMode, ColorValue, getcolor, resolve_new_color};

pub(crate) use conversion::hsv_buffer_to_rgb;
pub(crate) use conversion::{
    cmyk_pixel_to_rgb, hsv_pixel_to_rgb, rgb_buffer_to_hsv, rgb_buffer_to_ycbcr, rgb_bytes_to_hsv,
    rgb_bytes_to_ycbcr, rgb_pixels_to_hsv, rgb_pixels_to_ycbcr, ycbcr_buffer_to_rgb,
    ycbcr_pixel_to_rgb,
};
pub(crate) use photo_ycc::photo_ycc_pixel_to_rgb;
pub(crate) use grayscale::cmyk_muldiv255_byte as grayscale_cmyk_muldiv255_byte;
