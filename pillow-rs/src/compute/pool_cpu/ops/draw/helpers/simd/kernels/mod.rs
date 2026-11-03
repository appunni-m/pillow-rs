mod shapes;
mod span_writer;
mod spans;

pub(super) use self::shapes::{
    simd_draw_arc_native, simd_draw_chord_native, simd_draw_ellipse_native,
    simd_draw_pieslice_native, simd_draw_polygon_native, simd_draw_rounded_rect_native,
};
pub(super) use self::spans::{
    simd_draw_line_native, simd_draw_points_native, simd_draw_rectangle_native,
};
use self::span_writer::SimdDrawSpanWriter;
