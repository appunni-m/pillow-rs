//! Shared drawing geometry and native pixel writers.

mod arcs;
pub(crate) mod bitmap_mask;
pub(crate) mod bitmap_region;
pub(crate) mod gray_alpha_bitmap;
pub(crate) mod gray_alpha_text;
pub(crate) mod hidden_lanes;
pub(crate) mod i16_text;
pub(crate) mod i16_bitmap;
pub(crate) mod numeric_text;
pub(crate) mod raw_four_band_bitmap;
pub(crate) mod three_band_bitmap;
pub(crate) mod three_band_text;
pub(crate) mod text_result;
pub(crate) mod text_mask;
pub(crate) mod text_coords;
mod canvas;
mod clipping;
mod dispatch;
mod ellipse;
mod polygon;
mod primitives;
mod rounded_rect;
pub(crate) mod simd;
pub(crate) mod strategy;

mod native;

pub(crate) use canvas::{ConfigureDrawCanvas, Luma16Canvas, NativeByteCanvas};
pub(crate) use dispatch::{draw_op_on_canvas, is_draw_op, validate_draw_ops};
pub(crate) use native::{execute_byte_buffer, execute_float_buffer, execute_luma16_buffer};
