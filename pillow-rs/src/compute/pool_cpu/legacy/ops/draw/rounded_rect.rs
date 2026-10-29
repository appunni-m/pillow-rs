use super::arcs::{draw_arc_on_canvas, draw_pieslice_on_canvas};
use super::ellipse::draw_ellipse_on_canvas;
use super::primitives::draw_rect_on_canvas;
use crate::draw::DrawCanvas;
use crate::error::PilError;

pub(super) fn validate_rounded_rect(x0: i32, y0: i32, x1: i32, y1: i32) -> Result<(), PilError> {
    if x1 < x0 {
        return Err(PilError::ValueError(
            "x1 must be greater than or equal to x0".to_owned(),
        ));
    }
    if y1 < y0 {
        return Err(PilError::ValueError(
            "y1 must be greater than or equal to y0".to_owned(),
        ));
    }
    Ok(())
}

/// Draw a rounded rectangle into an already-owned native canvas.
///
/// Keeping the canvas ownership outside this routine is what lets a run of
/// draw operations share one full-frame clone while preserving their public
/// order and per-shape compositing behavior.
pub(super) fn draw_rounded_rect_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
) {
    let mut diameter = radius * 2.0;
    let full_x = diameter >= f64::from(x1 - x0 - 1);
    if full_x {
        diameter = f64::from(x1 - x0);
    }
    let full_y = full_x && diameter >= f64::from(y1 - y0 - 1);
    if full_y {
        diameter = f64::from(y1 - y0);
    }
    if full_x && full_y {
        draw_ellipse_on_canvas(canvas, x0, y0, x1, y1, fill, outline, width);
        return;
    }
    if diameter == 0.0 {
        draw_rect_on_canvas(canvas, x0, y0, x1, y1, fill, outline, width);
        return;
    }

    let diameter = diameter as i32;
    let radius = diameter / 2;
    let corners = rounded_rect_corners(x0, y0, x1, y1, diameter, full_x, full_y);

    if let Some(fill_color) = fill {
        draw_rounded_rect_fill(
            canvas, x0, y0, x1, y1, radius, full_x, full_y, &corners, fill_color,
        );
    }
    if let Some(outline_color) = outline.filter(|color| Some(*color) != fill && width != 0) {
        draw_rounded_rect_outline(
            canvas,
            x0,
            y0,
            x1,
            y1,
            radius,
            full_x,
            full_y,
            &corners,
            outline_color,
            width,
        );
    }
}

type RoundedCorner = (i32, i32, i32, i32, f64, f64);

fn rounded_rect_corners(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    diameter: i32,
    full_x: bool,
    full_y: bool,
) -> Vec<RoundedCorner> {
    if full_x {
        vec![
            (x0, y0, x0 + diameter, y0 + diameter, 180.0, 360.0),
            (x0, y1 - diameter, x0 + diameter, y1, 0.0, 180.0),
        ]
    } else if full_y {
        vec![
            (x0, y0, x0 + diameter, y0 + diameter, 90.0, 270.0),
            (x1 - diameter, y0, x1, y0 + diameter, 270.0, 90.0),
        ]
    } else {
        vec![
            (x0, y0, x0 + diameter, y0 + diameter, 180.0, 270.0),
            (x1 - diameter, y0, x1, y0 + diameter, 270.0, 360.0),
            (x1 - diameter, y1 - diameter, x1, y1, 0.0, 90.0),
            (x0, y1 - diameter, x0 + diameter, y1, 90.0, 180.0),
        ]
    }
}

fn draw_rounded_rect_fill<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: i32,
    full_x: bool,
    full_y: bool,
    corners: &[RoundedCorner],
    fill_color: (u8, u8, u8, u8),
) {
    for &(left, top, right, bottom, start, end) in corners {
        draw_pieslice_on_canvas(
            canvas,
            left,
            top,
            right,
            bottom,
            start,
            end,
            Some(fill_color),
            None,
            1,
        );
    }
    if full_x {
        draw_rect_on_canvas(
            canvas,
            x0,
            y0 + radius + 1,
            x1,
            y1 - radius - 1,
            Some(fill_color),
            None,
            1,
        );
    } else if x1 - radius - 1 > x0 + radius {
        draw_rect_on_canvas(
            canvas,
            x0 + radius + 1,
            y0,
            x1 - radius - 1,
            y1,
            Some(fill_color),
            None,
            1,
        );
    }
    if !full_x && !full_y {
        draw_rect_on_canvas(
            canvas,
            x0,
            y0 + radius + 1,
            x0 + radius,
            y1 - radius - 1,
            Some(fill_color),
            None,
            1,
        );
        draw_rect_on_canvas(
            canvas,
            x1 - radius,
            y0 + radius + 1,
            x1,
            y1 - radius - 1,
            Some(fill_color),
            None,
            1,
        );
    }
}

fn draw_rounded_rect_outline<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: i32,
    full_x: bool,
    full_y: bool,
    corners: &[RoundedCorner],
    outline_color: (u8, u8, u8, u8),
    width: u32,
) {
    for &(left, top, right, bottom, start, end) in corners {
        draw_arc_on_canvas(
            canvas,
            left,
            top,
            right,
            bottom,
            start,
            end,
            outline_color,
            width,
        );
    }
    let width = i32::try_from(width).unwrap_or(i32::MAX);
    if !full_x {
        draw_rect_on_canvas(
            canvas,
            x0 + radius + 1,
            y0,
            x1 - radius - 1,
            y0.saturating_add(width - 1),
            Some(outline_color),
            None,
            1,
        );
        draw_rect_on_canvas(
            canvas,
            x0 + radius + 1,
            y1.saturating_sub(width - 1),
            x1 - radius - 1,
            y1,
            Some(outline_color),
            None,
            1,
        );
    }
    if !full_y {
        draw_rect_on_canvas(
            canvas,
            x0,
            y0 + radius + 1,
            x0.saturating_add(width - 1),
            y1 - radius - 1,
            Some(outline_color),
            None,
            1,
        );
        draw_rect_on_canvas(
            canvas,
            x1.saturating_sub(width - 1),
            y0 + radius + 1,
            x1,
            y1 - radius - 1,
            Some(outline_color),
            None,
            1,
        );
    }
}
