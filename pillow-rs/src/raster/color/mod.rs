//! Pixel types (`Luma`, `Rgb`, etc.), color-type enums, and pixel conversions.
//!
//! This module is split by concern. `types.rs` holds the core type definitions;
//! trait implementations for `Pixel`, `FromColor`, `FromPrimitive`, `Blend`,
//! and `Invert` live in separate files.

pub(crate) mod blend;
pub(crate) mod from_color;
pub(crate) mod from_primitive;
pub(crate) mod invert;
pub(crate) mod pixel_luma;
pub(crate) mod pixel_rgb;
pub(crate) mod types;

// Re-export everything the crate expects from `color::*`
pub use self::from_color::FromColor;
pub use self::from_primitive::FromPrimitive;
pub use self::types::{ColorType, ExtendedColorType, Luma, LumaA, Rgb, Rgba};
