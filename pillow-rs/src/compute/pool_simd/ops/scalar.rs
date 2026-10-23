//! Scalar fallback implementations — safe, portable, auto-vectorization friendly.
//!
//! These are written as tight loops over u32 slices so LLVM can auto-vectorize
//! them when compiled with `-C target-cpu=native`. They also serve as the
//! reference implementation for platform-specific SIMD code.

#![allow(unused_imports)]

use super::{mode_has_alpha, mode_has_green_blue, mode_is_rgba};

mod chops;
mod convert;
mod filters;
mod histogram;
mod layout;
mod lut;
mod merge;
mod mutation;
mod point;
mod resize;
mod transform;

pub use self::chops::{
    add_modulo, blend, darker, difference, hard_light, invert_chops, lighter, logical_and,
    logical_or, logical_xor, multiply, overlay, screen, soft_light, subtract_modulo,
};
pub use self::convert::convert;
pub use self::filters::{
    box_blur, filter_3x3, filter_5x5, max_filter, median_filter, min_filter, rank_filter, sharpness,
};
pub use self::histogram::{autocontrast, equalize};
pub use self::layout::{crop, crop_border, duplicate, expand, flip, mirror, pad, transpose};
pub use self::lut::{eval, remap_palette};
pub use self::merge::merge;
pub use self::mutation::{
    alpha_composite, composite_module, paste, put_alpha, put_data, put_pixel,
};
pub use self::point::{
    brightness, color_saturation, colorize, constant, contrast, grayscale, invert, solarize,
};
pub use self::resize::{contain, cover, fit, reduce, resize, scale, thumbnail};
pub use self::transform::{rotate, transform};
