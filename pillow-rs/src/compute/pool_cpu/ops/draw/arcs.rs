use super::clipping::{
    ClipEllipseState, arc_clip_state, chord_clip_state, chord_line_clip_state,
    for_each_clipped_ellipse_span, normalize_angles, pie_clip_state, pie_side_clip_state,
};
use super::ellipse::{draw_ellipse_on_canvas, draw_ellipse_segments, draw_hline};
use crate::draw::DrawCanvas;

pub(super) fn draw_clipped_ellipse<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    a: i32,
    b: i32,
    state: ClipEllipseState,
    color: (u8, u8, u8, u8),
) {
    for_each_clipped_ellipse_span(x0, y0, a, b, state, |span_x0, span_y, span_x1| {
        draw_hline(canvas, span_x0, span_y, span_x1, color);
    });
}

pub(super) fn draw_arc_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: (u8, u8, u8, u8),
    width: u32,
) {
    let (start, end) = normalize_angles(start as f32, end as f32);
    if start + 360.0 == end {
        draw_ellipse_segments(
            canvas,
            x0,
            y0,
            x1,
            y1,
            i32::try_from(width).unwrap_or(i32::MAX),
            fill,
        );
        return;
    }
    if start == end {
        return;
    }
    let (Some(a), Some(b)) = (x1.checked_sub(x0), y1.checked_sub(y0)) else {
        return;
    };
    if a < 0 || b < 0 {
        return;
    }
    let state = arc_clip_state(a, b, i32::try_from(width).unwrap_or(i32::MAX), start, end);
    draw_clipped_ellipse(canvas, x0, y0, a, b, state, fill);
}

pub(super) fn draw_chord_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
) {
    let (start, end) = normalize_angles(start as f32, end as f32);
    if start + 360.0 == end {
        draw_ellipse_on_canvas(canvas, x0, y0, x1, y1, fill, outline, width);
        return;
    }
    if start == end {
        return;
    }
    let (Some(a), Some(b)) = (x1.checked_sub(x0), y1.checked_sub(y0)) else {
        return;
    };
    if a < 0 || b < 0 {
        return;
    }
    if let Some(fill_color) = fill {
        let state = chord_clip_state(a, b, a.saturating_add(b).saturating_add(1), start, end);
        draw_clipped_ellipse(canvas, x0, y0, a, b, state, fill_color);
    }
    if let Some(outline_color) = outline.filter(|color| Some(*color) != fill && width != 0) {
        let width = i32::try_from(width).unwrap_or(i32::MAX);
        let line_state = chord_line_clip_state(a, b, width, start, end);
        draw_clipped_ellipse(canvas, x0, y0, a, b, line_state, outline_color);
        let arc_state = chord_clip_state(a, b, width, start, end);
        draw_clipped_ellipse(canvas, x0, y0, a, b, arc_state, outline_color);
    }
}

/// Draw a pieslice directly on a canvas.
pub(super) fn draw_pieslice_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
) {
    let (start, end) = normalize_angles(start as f32, end as f32);
    if start + 360.0 == end {
        draw_ellipse_on_canvas(canvas, x0, y0, x1, y1, fill, outline, width);
        return;
    }
    if start == end {
        return;
    }
    let (Some(a), Some(b)) = (x1.checked_sub(x0), y1.checked_sub(y0)) else {
        return;
    };
    if a < 0 || b < 0 {
        return;
    }
    if let Some(fill_color) = fill {
        let state = pie_clip_state(a, b, a.saturating_add(b), start, end);
        draw_clipped_ellipse(canvas, x0, y0, a, b, state, fill_color);
    }
    if let Some(outline_color) = outline.filter(|color| Some(*color) != fill && width != 0) {
        let width = i32::try_from(width).unwrap_or(i32::MAX);
        for angle in [start, end] {
            let state = pie_side_clip_state(a, b, width, angle);
            draw_clipped_ellipse(canvas, x0, y0, a, b, state, outline_color);
        }
        let center_x = ((f64::from(x0) + f64::from(x1) - f64::from(width)) / 2.0).round() as i32;
        let center_y = ((f64::from(y0) + f64::from(y1) - f64::from(width)) / 2.0).round() as i32;
        draw_ellipse_segments(
            canvas,
            center_x,
            center_y,
            center_x.saturating_add(width - 1),
            center_y.saturating_add(width - 1),
            width.saturating_mul(2).saturating_sub(2),
            outline_color,
        );
        let state = pie_clip_state(a, b, width, start, end);
        draw_clipped_ellipse(canvas, x0, y0, a, b, state, outline_color);
    }
}
