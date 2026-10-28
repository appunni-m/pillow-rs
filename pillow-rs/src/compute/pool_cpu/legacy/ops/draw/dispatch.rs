use super::arcs::{draw_arc_on_canvas, draw_chord_on_canvas, draw_pieslice_on_canvas};
use super::canvas::NativeDrawCanvas;
use super::ellipse::draw_ellipse_on_canvas;
use super::polygon::draw_polygon_on_canvas;
use super::primitives::{draw_line_on_canvas, draw_rect_on_canvas};
use super::rounded_rect::{draw_rounded_rect_on_canvas, validate_rounded_rect};
use crate::draw::{DrawCanvas, plot};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

pub(super) fn draw_op_on_canvas(
    canvas: &mut NativeDrawCanvas,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<(), PilError> {
    configure_draw_canvas(canvas, op, mode);
    if draw_arc_operation(canvas, op) {
        return Ok(());
    }
    draw_primitive_operation(canvas, op, mode)
}

fn configure_draw_canvas(canvas: &mut NativeDrawCanvas, op: &PipelineOp, mode: Option<&str>) {
    let alpha_blend_rgb = match op {
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
    };
    // Select the blend setting per operation so a mixed draw context retains
    // the same RGB alpha behavior as the one-operation path.
    canvas.set_alpha_blend_rgb(alpha_blend_rgb);
    let packed_ink = matches!(
        op,
        PipelineOp::DrawLine { .. } | PipelineOp::DrawPoint { .. }
    );
    canvas.set_l16_ink_order(mode, packed_ink);
}

fn draw_arc_operation(canvas: &mut NativeDrawCanvas, op: &PipelineOp) -> bool {
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

fn draw_primitive_operation(
    canvas: &mut NativeDrawCanvas,
    op: &PipelineOp,
    mode: Option<&str>,
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
            ..
        } => draw_polygon_on_canvas(
            canvas,
            points,
            *fill,
            if mode == Some("F") { None } else { *outline },
            *width,
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

/// Execute contiguous drawing operations against one native canvas.
pub(crate) fn execute_draw_batch(
    img: &DynamicImage,
    ops: &[PipelineOp],
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
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
    let mut canvas = NativeDrawCanvas::from_image(img, false, mode);
    for op in ops {
        draw_op_on_canvas(&mut canvas, op, mode)?;
    }
    Ok(canvas.into_image())
}

// ── Public op_draw_* API ──
