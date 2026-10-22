//! Draw operation registration.
use super::prelude::{
    PilError, PipelineOp, Registry, op_draw_arc, op_draw_chord, op_draw_circle, op_draw_ellipse,
    op_draw_line, op_draw_pieslice, op_draw_point, op_draw_polygon, op_draw_rectangle,
    op_draw_rounded_rect,
};
fn register_part_1(m: &mut Registry) {
    // The exact Pillow scan conversion remains a scalar geometry/control
    // step; the GPU entry uploads its resulting packed canvas and performs the
    // native data-plane copy in draw.wgsl.
    draw_entry!(m, "DrawLine", |img, op, mode| {
        if let PipelineOp::DrawLine {
            x0,
            y0,
            x1,
            y1,
            fill,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_line(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *fill,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawLine".into()))
        }
    });

    draw_entry!(m, "DrawRectangle", |img, op, mode| {
        if let PipelineOp::DrawRectangle {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_rectangle(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawRectangle".into()))
        }
    });
}

fn register_part_2(m: &mut Registry) {
    draw_entry!(m, "DrawRoundedRect", |img, op, mode| {
        if let PipelineOp::DrawRoundedRect {
            x0,
            y0,
            x1,
            y1,
            radius,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_rounded_rect(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *radius,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawRoundedRect".into()))
        }
    });

    draw_entry!(m, "DrawEllipse", |img, op, mode| {
        if let PipelineOp::DrawEllipse {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_ellipse(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawEllipse".into()))
        }
    });
}

fn register_part_3(m: &mut Registry) {
    draw_entry!(m, "DrawCircle", |img, op, mode| {
        if let PipelineOp::DrawCircle {
            cx,
            cy,
            radius,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_circle(
                img,
                *cx,
                *cy,
                *radius,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawCircle".into()))
        }
    });

    draw_entry!(m, "DrawPolygon", |img, op, mode| {
        if let PipelineOp::DrawPolygon {
            points,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_polygon(img, points, *fill, *outline, *width, *alpha_blend_rgb, mode)
        } else {
            Err(PilError::ValueError("expected DrawPolygon".into()))
        }
    });

    draw_entry!(m, "DrawArc", |img, op, mode| {
        if let PipelineOp::DrawArc {
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            fill,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_arc(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *start,
                *end,
                *fill,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawArc".into()))
        }
    });
}

fn register_part_4(m: &mut Registry) {
    draw_entry!(m, "DrawChord", |img, op, mode| {
        if let PipelineOp::DrawChord {
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_chord(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *start,
                *end,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawChord".into()))
        }
    });

    draw_entry!(m, "DrawPieslice", |img, op, mode| {
        if let PipelineOp::DrawPieslice {
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            fill,
            outline,
            width,
            alpha_blend_rgb,
        } = op
        {
            op_draw_pieslice(
                img,
                *x0,
                *y0,
                *x1,
                *y1,
                *start,
                *end,
                *fill,
                *outline,
                *width,
                *alpha_blend_rgb,
                mode,
            )
        } else {
            Err(PilError::ValueError("expected DrawPieslice".into()))
        }
    });

    draw_entry!(m, "DrawPoint", |img, op, mode| {
        if let PipelineOp::DrawPoint {
            points,
            fill,
            alpha_blend_rgb,
        } = op
        {
            op_draw_point(img, points, *fill, *alpha_blend_rgb, mode)
        } else {
            Err(PilError::ValueError("expected DrawPoint".into()))
        }
    });
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    register_part_2(m);
    register_part_3(m);
    register_part_4(m);
    Ok(())
}
