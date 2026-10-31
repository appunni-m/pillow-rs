use super::DrawLayout;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

fn has_vectorized_byte_rows(image: &DynamicImage, channels: usize) -> bool {
    image.height() != 0
        && image
            .width()
            .checked_mul(channels as u32)
            .is_some_and(|row_bytes| row_bytes >= 16)
}

fn line_bounds_intersect(width: u32, height: u32, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let min_x = i64::from(x0.min(x1));
    let max_x = i64::from(x0.max(x1));
    let min_y = i64::from(y0.min(y1));
    let max_y = i64::from(y0.max(y1));
    width != 0
        && height != 0
        && max_x >= 0
        && max_y >= 0
        && min_x < i64::from(width)
        && min_y < i64::from(height)
}

pub(super) fn valid_draw_rectangle(x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    x1 >= x0 && y1 >= y0
}

pub(super) fn has_visible_draw_rectangle(
    width: u32,
    height: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    outline_width: u32,
) -> bool {
    if !valid_draw_rectangle(x0, y0, x1, y1) {
        return false;
    }
    let has_ink = fill.is_some() || (outline.is_some() && outline_width != 0);
    has_ink
        && i64::from(x1) >= 0
        && i64::from(y1) >= 0
        && i64::from(x0) < i64::from(width)
        && i64::from(y0) < i64::from(height)
}

pub(super) fn has_visible_draw_point(width: u32, height: u32, points: &[(i32, i32)]) -> bool {
    points.iter().any(|&(x, y)| {
        x >= 0 && y >= 0 && i64::from(x) < i64::from(width) && i64::from(y) < i64::from(height)
    })
}

pub(super) fn supports_for_image(
    image: &DynamicImage,
    op: &PipelineOp,
    layout: DrawLayout,
) -> bool {
    let width = image.width();
    let height = image.height();
    let vector_rows = has_vectorized_byte_rows(image, layout.channels);
    match op {
        PipelineOp::DrawLine { x0, y0, x1, y1, .. } => {
            vector_rows && line_bounds_intersect(width, height, *x0, *y0, *x1, *y1)
        }
        PipelineOp::DrawPoint { points, .. } => {
            (points.is_empty() || vector_rows)
                && (!has_visible_draw_point(width, height, points) || vector_rows)
        }
        PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. }
        | PipelineOp::DrawPolygon { .. } => vector_rows,
        PipelineOp::DrawRoundedRect {
            x0,
            y0,
            x1,
            y1,
            radius,
            ..
        } => vector_rows && x1 >= x0 && y1 >= y0 && radius.is_finite() && *radius >= 0.0,
        PipelineOp::DrawRectangle {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width: outline_width,
            ..
        } => {
            (width == 0 || height == 0 || vector_rows)
                && valid_draw_rectangle(*x0, *y0, *x1, *y1)
                && (has_visible_draw_rectangle(
                    width,
                    height,
                    *x0,
                    *y0,
                    *x1,
                    *y1,
                    *fill,
                    *outline,
                    *outline_width,
                ) || fill.is_none() && (outline.is_none() || *outline_width == 0)
                    || i64::from(*x1) < 0
                    || i64::from(*y1) < 0
                    || i64::from(*x0) >= i64::from(width)
                    || i64::from(*y0) >= i64::from(height))
        }
        _ => false,
    }
}
