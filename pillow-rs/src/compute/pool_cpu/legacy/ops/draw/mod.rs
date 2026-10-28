//! ImageDraw CPU operations — direct drawing in the destination's native layout.
//! These routines are called by the pipeline executor and preserve Pillow's
//! mode-specific scan conversion and byte-layout behavior.

mod arcs;
mod canvas;
mod clipping;
mod dispatch;
mod ellipse;
mod operations;
mod polygon;
mod primitives;
mod rounded_rect;

pub(crate) use clipping::{
    ClipEllipseState, arc_clip_state, chord_clip_state, chord_line_clip_state,
    for_each_clipped_ellipse_span, normalize_angles, pie_clip_state, pie_side_clip_state,
};
pub(crate) use dispatch::{execute_draw_batch, is_draw_op};
pub(crate) use ellipse::for_each_ellipse_span;
pub(crate) use operations::{
    op_draw_arc, op_draw_chord, op_draw_circle, op_draw_ellipse, op_draw_line, op_draw_pieslice,
    op_draw_point, op_draw_polygon, op_draw_rectangle, op_draw_rounded_rect,
};
