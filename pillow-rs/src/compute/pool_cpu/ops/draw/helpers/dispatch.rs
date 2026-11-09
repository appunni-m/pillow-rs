use super::arcs::{draw_arc_on_canvas, draw_chord_on_canvas, draw_pieslice_on_canvas};
use super::canvas::ConfigureDrawCanvas;
use super::ellipse::draw_ellipse_on_canvas;
use super::polygon::draw_polygon_on_canvas;
use super::primitives::{draw_line_on_canvas, draw_rect_on_canvas};
use super::rounded_rect::draw_rounded_rect_on_canvas;
use crate::draw::{DrawCanvas, plot, validate_rounded_rect};
use crate::error::PilError;
use crate::pipeline::PipelineOp;

pub(crate) fn alpha_blend_enabled(op: &PipelineOp) -> bool {
    match op {
        PipelineOp::DrawLine {
            alpha_blend_rgb, ..
        }
        | PipelineOp::DrawRectangle {
            alpha_blend_rgb, ..
        }
        | PipelineOp::DrawRoundedRect {
            alpha_blend_rgb, ..
        }
        | PipelineOp::DrawEllipse {
            alpha_blend_rgb, ..
        }
        | PipelineOp::DrawCircle {
            alpha_blend_rgb, ..
        }
        | PipelineOp::DrawPolygon {
            alpha_blend_rgb, ..
        }
        | PipelineOp::DrawArc {
            alpha_blend_rgb, ..
        }
        | PipelineOp::DrawChord {
            alpha_blend_rgb, ..
        }
        | PipelineOp::DrawPieslice {
            alpha_blend_rgb, ..
        }
        | PipelineOp::DrawPoint {
            alpha_blend_rgb, ..
        } => *alpha_blend_rgb,
        _ => false,
    }
}

pub(crate) fn draw_op_on_canvas<C: DrawCanvas + ConfigureDrawCanvas, const FLOAT_MODE: bool>(
    canvas: &mut C,
    op: &PipelineOp,
) -> Result<(), PilError> {
    canvas.configure(op);
    if draw_arc_operation(canvas, op) {
        return Ok(());
    }
    draw_primitive_operation::<C, FLOAT_MODE>(canvas, op)
}

fn draw_arc_operation<C: DrawCanvas>(canvas: &mut C, op: &PipelineOp) -> bool {
    match op {
        PipelineOp::DrawArc {
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            fill,
            width,
            ..
        } => {
            if let Some(fill) = fill {
                draw_arc_on_canvas(canvas, *x0, *y0, *x1, *y1, *start, *end, *fill, *width);
            }
            true
        }
        PipelineOp::DrawChord {
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            fill,
            outline,
            width,
            ..
        } => {
            draw_chord_on_canvas(
                canvas, *x0, *y0, *x1, *y1, *start, *end, *fill, *outline, *width,
            );
            true
        }
        PipelineOp::DrawPieslice {
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            fill,
            outline,
            width,
            ..
        } => {
            draw_pieslice_on_canvas(
                canvas, *x0, *y0, *x1, *y1, *start, *end, *fill, *outline, *width,
            );
            true
        }
        _ => false,
    }
}

fn draw_primitive_operation<C: DrawCanvas, const FLOAT_MODE: bool>(
    canvas: &mut C,
    op: &PipelineOp,
) -> Result<(), PilError> {
    match op {
        PipelineOp::DrawLine {
            x0,
            y0,
            x1,
            y1,
            fill,
            width,
            ..
        } => draw_line_on_canvas(canvas, *x0, *y0, *x1, *y1, *fill, *width),
        PipelineOp::DrawRectangle {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            ..
        } => draw_rect_on_canvas(canvas, *x0, *y0, *x1, *y1, *fill, *outline, *width),
        PipelineOp::DrawRoundedRect {
            x0,
            y0,
            x1,
            y1,
            radius,
            fill,
            outline,
            width,
            ..
        } => {
            validate_rounded_rect(*x0, *y0, *x1, *y1)?;
            draw_rounded_rect_on_canvas(
                canvas, *x0, *y0, *x1, *y1, *radius, *fill, *outline, *width,
            );
        }
        PipelineOp::DrawEllipse {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            ..
        } => draw_ellipse_on_canvas(canvas, *x0, *y0, *x1, *y1, *fill, *outline, *width),
        PipelineOp::DrawCircle {
            cx,
            cy,
            radius,
            fill,
            outline,
            width,
            ..
        } => draw_ellipse_on_canvas(
            canvas,
            *cx - *radius,
            *cy - *radius,
            *cx + *radius,
            *cy + *radius,
            *fill,
            *outline,
            *width,
        ),
        PipelineOp::DrawPolygon {
            points,
            fill,
            outline,
            width,
            outline_width,
            ..
        } => draw_polygon_on_canvas(
            canvas,
            points,
            *fill,
            // The archived F-mode Draw path deliberately ignores polygon outline ink.
            if FLOAT_MODE { None } else { *outline },
            *width,
            *outline_width,
        ),
        PipelineOp::DrawPoint { points, fill, .. } => {
            let (img_w, img_h) = (canvas.width(), canvas.height());
            for &(x, y) in points.iter() {
                plot(canvas, x, y, *fill, img_w, img_h, false);
            }
        }
        _ => {
            return Err(PilError::ValueError(
                "non-draw operation in draw batch".to_owned(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn is_draw_op(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::DrawLine { .. }
            | PipelineOp::DrawRectangle { .. }
            | PipelineOp::DrawRoundedRect { .. }
            | PipelineOp::DrawEllipse { .. }
            | PipelineOp::DrawCircle { .. }
            | PipelineOp::DrawPolygon { .. }
            | PipelineOp::DrawArc { .. }
            | PipelineOp::DrawChord { .. }
            | PipelineOp::DrawPieslice { .. }
            | PipelineOp::DrawPoint { .. }
    )
}

pub(crate) fn validate_draw_ops(ops: &[PipelineOp]) -> Result<(), PilError> {
    for op in ops {
        if !is_draw_op(op) {
            return Err(PilError::ValueError(
                "non-draw operation in draw batch".to_owned(),
            ));
        }
        if let PipelineOp::DrawRoundedRect { x0, y0, x1, y1, .. } = op {
            validate_rounded_rect(*x0, *y0, *x1, *y1)?;
        }
    }
    Ok(())
}
