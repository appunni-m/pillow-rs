//! Mode-aware CPU ImageChops strategies.

pub(crate) mod helpers;
pub(crate) mod ops;

mod one;
mod luma8;
mod luma_a8;
mod la_premultiplied8;
mod palette8;
mod palette_alpha8;
mod rgb8;
mod rgba8;
mod rgb_premultiplied8;
mod rgbx8;
mod cmyk8;
mod ycbcr8;
mod lab8;
mod hsv8;
mod i32;
mod f32;
mod i16;
mod i16l;
mod i16b;
mod i16n;
mod luma_a16;
mod rgb16;
mod rgba16;
mod rgb32f;
mod rgba32f;
