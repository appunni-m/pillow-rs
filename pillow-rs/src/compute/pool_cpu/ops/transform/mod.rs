//! Mode-aware CPU Transform strategies.

mod helpers;
pub(crate) mod ops;
pub(crate) mod one;
pub(crate) mod luma8;
pub(crate) mod luma_a8;
pub(crate) mod la_premultiplied8;
pub(crate) mod palette8;
pub(crate) mod palette_alpha8;
pub(crate) mod rgb8;
pub(crate) mod rgba8;
pub(crate) mod rgb_premultiplied8;
pub(crate) mod rgbx8;
pub(crate) mod cmyk8;
pub(crate) mod ycbcr8;
pub(crate) mod lab8;
pub(crate) mod hsv8;
pub(crate) mod i32;
pub(crate) mod f32;
pub(crate) mod i16;
pub(crate) mod i16l;
pub(crate) mod i16b;
pub(crate) mod i16n;
