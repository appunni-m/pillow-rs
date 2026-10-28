use crate::draw::{
    DrawCanvas, bresenham_line, plot, scanline_polygon_fill, wide_line_polygon_points,
};

/// Draw a line directly on a canvas (Bresenham).
pub(super) fn draw_line_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: (u8, u8, u8, u8),
    width: u32,
) {
    let (w, h) = (canvas.width(), canvas.height());
    if width <= 1 {
        bresenham_line(canvas, x0, y0, x1, y1, fill, w, h, false);
        return;
    }

    let Some(points) = wide_line_polygon_points(x0, y0, x1, y1, width) else {
        plot(canvas, x0, y0, fill, w, h, false);
        return;
    };
    scanline_polygon_fill(canvas, &points, fill, w, h, false);
}

/// Draw a rectangle directly on a canvas.
pub(super) fn draw_horizontal_span<C: DrawCanvas>(
    canvas: &mut C,
    x0: i64,
    x1: i64,
    y: i64,
    color: (u8, u8, u8, u8),
    w: u32,
    h: u32,
) {
    if y < 0 || y >= i64::from(h) || w == 0 || x1 < 0 || x0 >= i64::from(w) {
        return;
    }
    let start = x0.max(0);
    let end = x1.min(i64::from(w) - 1);
    if start > end {
        return;
    }
    for x in start..=end {
        plot(canvas, x as i32, y as i32, color, w, h, false);
    }
}

/// Draw the vertical line primitive used by Pillow's rectangle outline.
/// `ImagingDrawRectangle` intentionally omits the final endpoint of each
/// vertical line, unlike the public line wrapper which adds its endpoint.
pub(super) fn draw_vertical_segment_exclusive<C: DrawCanvas>(
    canvas: &mut C,
    x: i64,
    y0: i64,
    y1: i64,
    color: (u8, u8, u8, u8),
    w: u32,
    h: u32,
) {
    if x < 0 || x >= i64::from(w) || h == 0 {
        return;
    }
    if y0 < y1 {
        let start = y0.max(0);
        let end = y1.min(i64::from(h));
        for y in start..end {
            plot(canvas, x as i32, y as i32, color, w, h, false);
        }
    } else if y0 > y1 {
        // Pillow's ImagingDrawRectangle keeps the exclusive vertical segment
        // even when a wide stroke makes its endpoints cross.  That descending
        // path is observable on degenerate public boxes, so retain it instead
        // of treating the interval as an ordinary empty range.
        let mut y = y0.min(i64::from(h) - 1);
        let stop = y1.max(-1);
        while y > stop {
            plot(canvas, x as i32, y as i32, color, w, h, false);
            y -= 1;
        }
    }
}

pub(super) fn draw_rect_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
) {
    let (img_w, img_h) = (canvas.width(), canvas.height());
    if x1 < x0 || y1 < y0 {
        return;
    }

    // Pillow's src/libImaging/Draw.c::ImagingDrawRectangle draws the fill as
    // inclusive horizontal spans, then constructs the outline from width
    // horizontal spans plus vertical line segments.  Keeping those primitive
    // calls separate is important for degenerate boxes: a zero-width or
    // zero-height box expands only through the outline's vertical segments.
    if let Some(fc) = fill
        && img_h > 0
    {
        let start = i64::from(y0).max(0);
        let end = i64::from(y1).min(i64::from(img_h) - 1);
        for py in start..=end {
            draw_horizontal_span(canvas, i64::from(x0), i64::from(x1), py, fc, img_w, img_h);
        }
    }

    // The C primitive normalizes width zero to one. The public binding already
    // performs that normalization, but retaining it here keeps direct core
    // dispatch behavior equivalent as well.
    if let Some(oc) = outline.filter(|_| width != 0) {
        let width = i64::from(width);
        for i in 0..width {
            draw_horizontal_span(
                canvas,
                i64::from(x0),
                i64::from(x1),
                i64::from(y0).saturating_add(i),
                oc,
                img_w,
                img_h,
            );
            draw_horizontal_span(
                canvas,
                i64::from(x0),
                i64::from(x1),
                i64::from(y1).saturating_sub(i),
                oc,
                img_w,
                img_h,
            );
            draw_vertical_segment_exclusive(
                canvas,
                i64::from(x1).saturating_sub(i),
                i64::from(y0).saturating_add(width),
                i64::from(y1).saturating_sub(width).saturating_add(1),
                oc,
                img_w,
                img_h,
            );
            draw_vertical_segment_exclusive(
                canvas,
                i64::from(x0).saturating_add(i),
                i64::from(y0).saturating_add(width),
                i64::from(y1).saturating_sub(width).saturating_add(1),
                oc,
                img_w,
                img_h,
            );
        }
    } else if outline.is_some() {
        // An explicitly zero-width outline is intentionally a no-op at the
        // Python layer, matching ImageDraw.rectangle's width guard.
    }
}
