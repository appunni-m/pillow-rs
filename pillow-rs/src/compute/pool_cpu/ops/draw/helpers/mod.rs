//! Shared drawing geometry and native pixel writers.

mod arcs;
pub(crate) mod bitmap_mask;
pub(crate) mod bitmap_region;
mod canvas;
mod clipping;
mod dispatch;
mod ellipse;
pub(crate) mod gray_alpha_bitmap;
pub(crate) mod gray_alpha_text;
pub(crate) mod hidden_lanes;
pub(crate) mod i16_bitmap;
pub(crate) mod i16_text;
pub(crate) mod numeric_text;
mod polygon;
mod primitives;
pub(crate) mod raw_four_band_bitmap;
mod rounded_rect;
pub(crate) mod simd;
pub(crate) mod strategy;
pub(crate) mod support;
pub(crate) mod text_coords;
pub(crate) mod text_mask;
pub(crate) mod text_result;
pub(crate) mod three_band_bitmap;
pub(crate) mod three_band_text;

mod native;

pub(crate) use canvas::{Luma16Canvas, NativeByteCanvas};
pub(crate) use dispatch::{draw_op_on_canvas, is_draw_op, validate_draw_ops};
pub(crate) use native::{execute_byte_buffer, execute_float_buffer, execute_luma16_buffer};
