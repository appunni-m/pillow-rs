//! ImageOps CPU operations extracted from image.rs execute_op().
//! These implement PIL-compatible image operations: autocontrast, equalize,
//! invert, flip, mirror, posterize, solarize, grayscale, colorize,
//! contain, cover, fit, pad, scale, expand, and crop border.

// TODO(pillow-rs#3ptb): Move the remaining ImageOps families out of this shared
// facade and declare each family's exhaustive Mode strategy at its entry point.

use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

/// Python 3's round() (banker's rounding): rounds half to even.
/// This matches PIL's behavior: round(12.5) -> 12, round(13.5) -> 14.
fn bankers_round(x: f64) -> f64 {
    let floor = x.floor();
    let frac = x - floor;
    if frac == 0.5 {
        if floor % 2.0 == 0.0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        (x + 0.5).floor()
    }
}

use super::geometry::execute_resize_in_context;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::pil_resize;

use crate::ops::pil_resize::pil_resize_luma_i32_into_window;
use crate::ops::pil_resize::pil_resize_rgbx_into_window;
use crate::pipeline::ResampleFilter;
use wide::u8x16;

mod colorize;
mod geometry;
mod histogram;
mod pad;
mod pixel;

pub(crate) use colorize::{colorize_lut, gather_native_channel, op_colorize};
pub(crate) use geometry::{
    op_contain, op_cover, op_crop_border, op_fit, op_fit_in_context, op_scale,
};
pub(crate) use histogram::{autocontrast_lut, equalize_lut};
pub(crate) use pad::{op_pad, op_pad_in_context};
pub(crate) use pixel::{
    op_autocontrast, op_equalize, op_equalize_with_mask, op_flip, op_invert, op_mirror,
    op_posterize, op_solarize,
};
