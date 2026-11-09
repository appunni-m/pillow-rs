//! Mode-aware CPU ImageChops strategies.

pub(crate) mod helpers;
pub(crate) mod ops;

mod cmyk8;
mod f32;
mod hsv8;
mod i16;
mod i16b;
mod i16l;
mod i16n;
mod i32;
mod la_premultiplied8;
mod lab8;
mod luma8;
mod luma_a16;
mod luma_a8;
mod one;
mod palette8;
mod palette_alpha8;
mod rgb16;
mod rgb32f;
mod rgb8;
mod rgb_premultiplied8;
mod rgba16;
mod rgba32f;
mod rgba8;
mod rgbx8;
mod ycbcr8;
