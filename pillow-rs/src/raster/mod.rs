//! Pillow-owned in-memory raster storage and pixel operations.
//!
//! Encoded format detection, inspection, decoding, and encoding remain owned by
//! `image-slash-star`. This module contains only the materialized pixel model
//! required by Pillow operations.

pub(crate) mod buffer;
pub(crate) mod color;
pub(crate) mod dynamic;
pub(crate) mod error;
pub(crate) mod traits;

pub use self::buffer::{
    ConvertBuffer, EnumeratePixels, EnumeratePixelsMut, EnumerateRows, EnumerateRowsMut,
    GrayAlphaImage, GrayImage, ImageBuffer, Pixels, PixelsMut, Rgb32FImage, RgbImage, Rgba32FImage,
    RgbaImage, Rows, RowsMut,
};
pub use self::color::{
    ColorType, ExtendedColorType, FromColor, FromPrimitive, Luma, LumaA, Rgb, Rgba,
};
pub use self::dynamic::DynamicImage;
pub use self::error::Rect;
pub use self::traits::{
    EncodableLayout, Enlargeable, GenericImage, GenericImageView, Pixel, Primitive,
};
