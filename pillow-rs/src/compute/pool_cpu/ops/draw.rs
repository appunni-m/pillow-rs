//! ImageDraw CPU operations — direct drawing on RgbaImage canvas.
//! These are called by the pipeline executor when a DrawXxx PipelineOp
//! is encountered. They implement the same algorithms as the Draw methods
//! in draw/mod.rs, but operate directly on DynamicImage to avoid circular
//! recursion (Draw methods now push PipelineOps).
//!
//! P-mode (palette-indexed) images are handled specially: drawing preserves
//! the Luma8 index buffer and writes palette index values directly, matching
//! PIL's behavior (fill colors use their R channel as the palette index).

use crate::draw::{bresenham_line, plot, scanline_polygon_fill};
use crate::error::PilError;
use image_slash_star::{DynamicImage, GrayImage, Rgba, RgbaImage};
/// Helper: draw on an image, preserving P-mode (Luma8) when possible.
/// For P-mode (`mode == Some("P")` with Luma8 input), converts to RGBA
/// temporarily for drawing, then converts back to Luma8 by taking the
/// R channel (all channels remain equal for P-mode drawing, so this is
/// lossless and preserves palette index values).
/// For all other modes, works on RGBA as before.
fn draw_preserve_p_mode<F>(img: &DynamicImage, mode: Option<&str>, draw_fn: F) -> DynamicImage
where
    F: Fn(&mut RgbaImage),
{
    // Detect mode from actual image type (LA is native, not explicit_mode)
    let is_la = matches!(img, DynamicImage::ImageLumaA8(_)) || (mode == Some("LA"));
    let is_p_mode = matches!(img, DynamicImage::ImageLuma8(_)) && mode == Some("P");
    let mut canvas = img.to_rgba8();
    draw_fn(&mut canvas);
    if is_p_mode {
        // Convert back to Luma8 by extracting R channel (R=G=B for P-mode indices)
        let (w, h) = canvas.dimensions();
        DynamicImage::ImageLuma8(GrayImage::from_fn(w, h, |x, y| {
            image_slash_star::Luma([canvas.get_pixel(x, y)[0]])
        }))
    } else if is_la {
        // Convert back to LumaA8 by extracting R (luma) and A (alpha)
        let (w, h) = canvas.dimensions();
        DynamicImage::ImageLumaA8(image_slash_star::GrayAlphaImage::from_fn(w, h, |x, y| {
            let px = canvas.get_pixel(x, y);
            image_slash_star::LumaA([px[0], px[3]])
        }))
    } else {
        DynamicImage::ImageRgba8(canvas)
    }
}

/// Draw a line directly on a canvas (Bresenham).
fn draw_line_on_canvas(
    canvas: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: (u8, u8, u8, u8),
    width: u32,
) {
    let (w, h) = (canvas.width(), canvas.height());
    let max_w = w.min(h).min(100);
    let width = if width > max_w {
        log::warn!("draw_line: width {} clamped to {}", width, max_w);
        max_w
    } else {
        width
    };
    if width <= 1 {
        bresenham_line(canvas, x0, y0, x1, y1, fill, w, h, false);
    } else {
        let half = (width as i32) / 2;
        for offset in -half..=half {
            bresenham_line(canvas, x0 + offset, y0, x1 + offset, y1, fill, w, h, false);
            bresenham_line(canvas, x0, y0 + offset, x1, y1 + offset, fill, w, h, false);
        }
    }
}

/// Draw a rectangle directly on a canvas.
fn draw_rect_on_canvas(
    canvas: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
) {
    let (img_w, img_h) = (canvas.width(), canvas.height());

    let x0 = x0.clamp(0, img_w as i32 - 1);
    let y0 = y0.clamp(0, img_h as i32 - 1);
    let x1 = x1.clamp(0, img_w as i32);
    let y1 = y1.clamp(0, img_h as i32);

    // Clamp outline width to prevent CPU DoS from attacker-controlled width
    let max_w = img_w.min(img_h).min(100);
    let width = if width > max_w {
        log::warn!("draw_rect: outline width {} clamped to {}", width, max_w);
        max_w
    } else {
        width
    };

    // Fill (inclusive range, matching PIL)
    if let Some(fc) = fill {
        for py in y0..=y1 {
            for px in x0..=x1 {
                if px >= 0 && py >= 0 && (px as u32) < img_w && (py as u32) < img_h {
                    canvas.put_pixel(px as u32, py as u32, Rgba([fc.0, fc.1, fc.2, fc.3]));
                }
            }
        }
    }

    // Outline
    if let Some(oc) = outline {
        for w in 0..width as i32 {
            // Top
            for px in x0 - w..=x1 + w {
                plot(canvas, px, y0 - w, oc, img_w, img_h, false);
                plot(canvas, px, y1 + w, oc, img_w, img_h, false);
            }
            // Sides
            for py in y0 - w..=y1 + w {
                plot(canvas, x0 - w, py, oc, img_w, img_h, false);
                plot(canvas, x1 + w, py, oc, img_w, img_h, false);
            }
        }
    }
}

/// Draw an ellipse directly on a canvas using Bresenham quarter-ellipse.
fn draw_ellipse_on_canvas(
    canvas: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
) {
    let (img_w, img_h) = (canvas.width(), canvas.height());

    let rx = ((x1 - x0) as f64 / 2.0).abs();
    let ry = ((y1 - y0) as f64 / 2.0).abs();
    if rx < 1.0 || ry < 1.0 {
        return;
    }

    let a = x1 - x0;
    let b = y1 - y0;
    if a <= 0 || b <= 0 {
        return;
    }
    let cx_i = (x0 + x1) / 2;

    let ex = a % 2;
    let ey = b;
    let a2 = a as i64 * a as i64;
    let b2 = b as i64 * b as i64;
    let a2b2 = a2 * b2;
    let quarter_delta = |x: i64, y: i64| -> i64 { (a2 * y * y + b2 * x * x - a2b2).abs() };

    // Fill using PIL's exact Bresenham quarter-ellipse algorithm.
    if let Some(fc) = fill {
        let mut pr = a as i64;
        let mut py = 0i64;
        let mut qx = a;
        let mut qy = b % 2;
        let mut finished = false;
        while !finished {
            let y_pos = y0 + ((py + b as i64) / 2) as i32;
            let y_neg = y0 + ((-py + b as i64) / 2) as i32;
            let xb = (pr / 2) as i32;
            let left = (cx_i - xb).max(x0).max(0);
            let right = (cx_i + xb).min(x1).min(img_w as i32 - 1);
            for &y_img in &[y_pos, y_neg] {
                if y_img >= y0 && y_img <= y1 && xb > 0 {
                    for x in left..=right {
                        if x >= 0 && y_img >= 0 && (x as u32) < img_w && (y_img as u32) < img_h {
                            canvas.put_pixel(
                                x as u32,
                                y_img as u32,
                                Rgba([fc.0, fc.1, fc.2, fc.3]),
                            );
                        }
                    }
                }
            }
            loop {
                if qx < 0 {
                    finished = true;
                    break;
                }
                if qx == ex && qy == ey {
                    finished = true;
                    break;
                }
                let mut nx = qx;
                let mut ny = qy + 2;
                let mut ndelta = quarter_delta(nx as i64, ny as i64);
                if qx > 1 {
                    let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                    if ndelta > d1 {
                        nx = qx - 2;
                        ny = qy + 2;
                        ndelta = d1;
                    }
                    let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                    if ndelta > d2 {
                        nx = qx - 2;
                        ny = qy;
                    }
                }
                if ny > ey {
                    finished = true;
                    break;
                }
                if ny as i64 > py {
                    pr = nx as i64;
                    py = ny as i64;
                    qx = nx;
                    qy = ny;
                    break;
                }
                qx = nx;
                qy = ny;
            }
        }
    }

    // Outline via Bresenham boundary + edge detection
    if let Some(oc) = outline {
        let mut filled = vec![false; (img_w * img_h) as usize];
        let mut qx = a;
        let mut qy = b % 2;
        let mut pr = a as i64;
        let mut py = 0i64;
        let mut finished = false;
        while !finished {
            let y_pos = y0 + ((py + b as i64) / 2) as i32;
            let y_neg = y0 + ((-py + b as i64) / 2) as i32;
            let xb = (pr / 2) as i32;
            if xb > 0 {
                let left = (cx_i - xb).max(x0).max(0);
                let right = (cx_i + xb).min(x1).min(img_w as i32 - 1);
                for &y_img in &[y_pos, y_neg] {
                    if y_img >= y0 && y_img <= y1 && y_img >= 0 && (y_img as u32) < img_h {
                        for x in left..=right {
                            filled[(y_img as usize) * (img_w as usize) + (x as usize)] = true;
                        }
                    }
                }
            }
            loop {
                if qx < 0 {
                    finished = true;
                    break;
                }
                if qx == ex && qy == ey {
                    finished = true;
                    break;
                }
                let mut nx = qx;
                let mut ny = qy + 2;
                let mut ndelta = quarter_delta(nx as i64, ny as i64);
                if qx > 1 {
                    let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                    if ndelta > d1 {
                        nx = qx - 2;
                        ny = qy + 2;
                        ndelta = d1;
                    }
                    let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                    if ndelta > d2 {
                        nx = qx - 2;
                        ny = qy;
                    }
                }
                if ny > ey {
                    finished = true;
                    break;
                }
                if ny as i64 > py {
                    pr = nx as i64;
                    py = ny as i64;
                    qx = nx;
                    qy = ny;
                    break;
                }
                qx = nx;
                qy = ny;
            }
        }
        // Edge detection
        let iw = img_w as i32;
        let ih = img_h as i32;
        for y in 0..ih {
            for x in 0..iw {
                let idx = (y as usize) * (img_w as usize) + (x as usize);
                if !filled[idx] {
                    continue;
                }
                // Treat out-of-bounds neighbors as "filled" to avoid
                // false boundaries at image edges (clipped ellipse).
                let lf = filled[(y as usize) * (img_w as usize) + ((x - 1).max(0) as usize)];
                let rf = filled[(y as usize) * (img_w as usize) + ((x + 1).min(iw - 1) as usize)];
                let uf = filled[((y - 1).max(0) as usize) * (img_w as usize) + (x as usize)];
                let df = filled[((y + 1).min(ih - 1) as usize) * (img_w as usize) + (x as usize)];
                if !lf || !rf || !uf || !df {
                    plot(canvas, x, y, oc, img_w, img_h, false);
                }
            }
        }
    }
}

/// Draw a polygon directly on a canvas.
fn draw_polygon_on_canvas(
    canvas: &mut RgbaImage,
    points: &[(i32, i32)],
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
) {
    let (img_w, img_h) = (canvas.width(), canvas.height());

    // Outline
    if let Some(oc) = outline {
        for i in 0..points.len() {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % points.len()];
            bresenham_line(canvas, x0, y0, x1, y1, oc, img_w, img_h, false);
        }
    }

    // Fill: PIL-identical scanline algorithm
    if let Some(fc) = fill {
        scanline_polygon_fill(canvas, points, fc, img_w, img_h, false);
    }
}

/// Draw an arc (partial ellipse outline) directly on a canvas.
fn draw_arc_on_canvas(
    canvas: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: (u8, u8, u8, u8),
) {
    let (img_w, img_h) = (canvas.width(), canvas.height());
    let cx = (x0 + x1) as f64 / 2.0;
    let cy = (y0 + y1) as f64 / 2.0;
    let a = x1 - x0;
    let b = y1 - y0;
    if a <= 0 || b <= 0 {
        return;
    }

    // Normalize angles to 0..360
    let mut s = start % 360.0;
    if s < 0.0 {
        s += 360.0;
    }
    let mut e = end % 360.0;
    if e < 0.0 {
        e += 360.0;
    }
    let angle_in_range = |angle: f64| -> bool {
        let mut a = angle % 360.0;
        if a < 0.0 {
            a += 360.0;
        }
        if s <= e {
            a >= s && a <= e
        } else {
            a >= s || a <= e
        }
    };

    let cx_i = (x0 + x1) / 2;

    // Compute full ellipse fill using the Bresenham generator + edge detection
    let mut filled = vec![false; (img_w * img_h) as usize];
    let mut qx = a;
    let mut qy = b % 2;
    let ex = a % 2;
    let ey = b;
    let a2 = a as i64 * a as i64;
    let b2 = b as i64 * b as i64;
    let a2b2 = a2 * b2;
    let quarter_delta = |x: i64, y: i64| -> i64 { (a2 * y * y + b2 * x * x - a2b2).abs() };
    let mut pr = a as i64;
    let mut py = 0i64;
    let mut finished = false;

    while !finished {
        let y_pos = y0 + ((py + b as i64) / 2) as i32;
        let y_neg = y0 + ((-py + b as i64) / 2) as i32;
        let xb = (pr / 2) as i32;
        if xb > 0 {
            let left = (cx_i - xb).max(x0);
            let right = (cx_i + xb).min(x1);
            for &y_img in &[y_pos, y_neg] {
                if y_img >= y0 && y_img <= y1 {
                    for x in left..=right {
                        if x >= 0 && y_img >= 0 && (x as u32) < img_w && (y_img as u32) < img_h {
                            filled[(y_img as usize) * (img_w as usize) + (x as usize)] = true;
                        }
                    }
                }
            }
        }
        loop {
            if qx < 0 {
                finished = true;
                break;
            }
            if qx == ex && qy == ey {
                finished = true;
                break;
            }
            let mut nx = qx;
            let mut ny = qy + 2;
            let mut ndelta = quarter_delta(nx as i64, ny as i64);
            if qx > 1 {
                let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                if ndelta > d1 {
                    nx = qx - 2;
                    ny = qy + 2;
                    ndelta = d1;
                }
                let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                if ndelta > d2 {
                    nx = qx - 2;
                    ny = qy;
                }
            }
            if ny > ey {
                finished = true;
                break;
            }
            if ny as i64 > py {
                pr = nx as i64;
                py = ny as i64;
                qx = nx;
                qy = ny;
                break;
            }
            qx = nx;
            qy = ny;
        }
    }

    // Edge detection — boundary pixels filtered by angle
    let iw = img_w as i32;
    let ih = img_h as i32;
    for y in 0..ih {
        for x in 0..iw {
            let idx = (y as usize) * (img_w as usize) + (x as usize);
            if !filled[idx] {
                continue;
            }
            let left_filled = x > 0 && filled[(y as usize) * (img_w as usize) + ((x - 1) as usize)];
            let right_filled =
                x < iw - 1 && filled[(y as usize) * (img_w as usize) + ((x + 1) as usize)];
            let up_filled = y > 0 && filled[((y - 1) as usize) * (img_w as usize) + (x as usize)];
            let down_filled =
                y < ih - 1 && filled[((y + 1) as usize) * (img_w as usize) + (x as usize)];
            let is_boundary = !left_filled || !right_filled || !up_filled || !down_filled;
            if is_boundary {
                let angle = (y as f64 - cy).atan2(x as f64 - cx).to_degrees();
                if angle_in_range(angle) {
                    canvas.put_pixel(x as u32, y as u32, Rgba([fill.0, fill.1, fill.2, fill.3]));
                }
            }
        }
    }
}

/// Draw a pieslice directly on a canvas.
fn draw_pieslice_on_canvas(
    canvas: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
) {
    let (img_w, img_h) = (canvas.width(), canvas.height());
    let cx = (x0 + x1) as f64 / 2.0;
    let cy = (y0 + y1) as f64 / 2.0;
    let a = x1 - x0;
    let b = y1 - y0;
    if a <= 0 || b <= 0 {
        return;
    }

    // Normalize angles
    let mut s = start % 360.0;
    if s < 0.0 {
        s += 360.0;
    }
    let mut e = end % 360.0;
    if e < 0.0 {
        e += 360.0;
    }
    let angle_in_range = |angle: f64| -> bool {
        let mut a = angle % 360.0;
        if a < 0.0 {
            a += 360.0;
        }
        if s <= e {
            a >= s && a <= e
        } else {
            a >= s || a <= e
        }
    };

    let cx_i = (x0 + x1) / 2;
    let cy_i = (y0 + y1) / 2;

    // Bresenham quarter generator
    let mut qx = a;
    let mut qy = b % 2;
    let ex = a % 2;
    let ey = b;
    let a2 = a as i64 * a as i64;
    let b2 = b as i64 * b as i64;
    let a2b2 = a2 * b2;
    let quarter_delta = |x: i64, y: i64| -> i64 { (a2 * y * y + b2 * x * x - a2b2).abs() };
    let mut pr = a as i64;
    let mut py = 0i64;
    let mut finished = false;

    // Fill
    if let Some(fc) = fill {
        while !finished {
            let y_pos = y0 + ((py + b as i64) / 2) as i32;
            let y_neg = y0 + ((-py + b as i64) / 2) as i32;
            let xb = (pr / 2) as i32;
            if xb > 0 {
                let left = (cx_i - xb).max(x0);
                let right = (cx_i + xb).min(x1);
                for &y_img in &[y_pos, y_neg] {
                    if y_img >= y0 && y_img <= y1 {
                        for x in left..=right {
                            if x >= 0 && y_img >= 0 && (x as u32) < img_w && (y_img as u32) < img_h
                            {
                                if x == cx_i && y_img == cy_i {
                                    canvas.put_pixel(
                                        x as u32,
                                        y_img as u32,
                                        Rgba([fc.0, fc.1, fc.2, fc.3]),
                                    );
                                } else {
                                    let angle =
                                        (y_img as f64 - cy).atan2(x as f64 - cx).to_degrees();
                                    if angle_in_range(angle) {
                                        canvas.put_pixel(
                                            x as u32,
                                            y_img as u32,
                                            Rgba([fc.0, fc.1, fc.2, fc.3]),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            loop {
                if qx < 0 {
                    finished = true;
                    break;
                }
                if qx == ex && qy == ey {
                    finished = true;
                    break;
                }
                let mut nx = qx;
                let mut ny = qy + 2;
                let mut ndelta = quarter_delta(nx as i64, ny as i64);
                if qx > 1 {
                    let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                    if ndelta > d1 {
                        nx = qx - 2;
                        ny = qy + 2;
                        ndelta = d1;
                    }
                    let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                    if ndelta > d2 {
                        nx = qx - 2;
                        ny = qy;
                    }
                }
                if ny > ey {
                    finished = true;
                    break;
                }
                if ny as i64 > py {
                    pr = nx as i64;
                    py = ny as i64;
                    qx = nx;
                    qy = ny;
                    break;
                }
                qx = nx;
                qy = ny;
            }
        }
    }

    // Outline: radii + arc edge
    if let Some(oc) = outline {
        // Radius lines from center to arc endpoints
        for angle_deg in [start, end] {
            let rad = angle_deg.to_radians();
            let ax = (cx + (a as f64 / 2.0) * rad.cos()).round() as i32;
            let ay = (cy + (b as f64 / 2.0) * rad.sin()).round() as i32;
            bresenham_line(canvas, cx_i, cy_i, ax, ay, oc, img_w, img_h, false);
        }
        // Arc edge via Bresenham + edge detection
        let mut filled = vec![false; (img_w * img_h) as usize];
        let mut qx = a;
        let mut qy = b % 2;
        let mut pr = a as i64;
        let mut py = 0i64;
        let mut finished = false;
        while !finished {
            let y_pos = y0 + ((py + b as i64) / 2) as i32;
            let y_neg = y0 + ((-py + b as i64) / 2) as i32;
            let xb = (pr / 2) as i32;
            if xb > 0 {
                let left = (cx_i - xb).max(x0);
                let right = (cx_i + xb).min(x1);
                for &y_img in &[y_pos, y_neg] {
                    if y_img >= y0 && y_img <= y1 {
                        for x in left..=right {
                            if x >= 0 && y_img >= 0 && (x as u32) < img_w && (y_img as u32) < img_h
                            {
                                let angle = (y_img as f64 - cy).atan2(x as f64 - cx).to_degrees();
                                if angle_in_range(angle) {
                                    filled[(y_img as usize) * (img_w as usize) + (x as usize)] =
                                        true;
                                }
                            }
                        }
                    }
                }
            }
            loop {
                if qx < 0 {
                    finished = true;
                    break;
                }
                if qx == ex && qy == ey {
                    finished = true;
                    break;
                }
                let mut nx = qx;
                let mut ny = qy + 2;
                let mut ndelta = quarter_delta(nx as i64, ny as i64);
                if qx > 1 {
                    let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                    if ndelta > d1 {
                        nx = qx - 2;
                        ny = qy + 2;
                        ndelta = d1;
                    }
                    let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                    if ndelta > d2 {
                        nx = qx - 2;
                        ny = qy;
                    }
                }
                if ny > ey {
                    finished = true;
                    break;
                }
                if ny as i64 > py {
                    pr = nx as i64;
                    py = ny as i64;
                    qx = nx;
                    qy = ny;
                    break;
                }
                qx = nx;
                qy = ny;
            }
        }
        // Edge detection
        let iw = img_w as i32;
        let ih = img_h as i32;
        for y in 0..ih {
            for x in 0..iw {
                let idx = (y as usize) * (img_w as usize) + (x as usize);
                if !filled[idx] {
                    continue;
                }
                let left_f = x > 0 && filled[(y as usize) * (img_w as usize) + ((x - 1) as usize)];
                let right_f =
                    x < iw - 1 && filled[(y as usize) * (img_w as usize) + ((x + 1) as usize)];
                let up_f = y > 0 && filled[((y - 1) as usize) * (img_w as usize) + (x as usize)];
                let down_f =
                    y < ih - 1 && filled[((y + 1) as usize) * (img_w as usize) + (x as usize)];
                if !left_f || !right_f || !up_f || !down_f {
                    canvas.put_pixel(x as u32, y as u32, Rgba([oc.0, oc.1, oc.2, oc.3]));
                }
            }
        }
    }
}

// ── Public op_draw_* API ──

pub fn op_draw_line(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: (u8, u8, u8, u8),
    width: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_preserve_p_mode(img, _mode, |canvas| {
        draw_line_on_canvas(canvas, x0, y0, x1, y1, fill, width);
    }))
}

pub fn op_draw_rectangle(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_preserve_p_mode(img, _mode, |canvas| {
        draw_rect_on_canvas(canvas, x0, y0, x1, y1, fill, outline, width);
    }))
}

pub fn op_draw_rounded_rect(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    _precision_bits: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let r = radius.round() as i32;
    let d = r * 2;
    if d <= 0 || x1 <= x0 + 1 || y1 <= y0 + 1 {
        return op_draw_rectangle(img, x0, y0, x1, y1, fill, outline, 1, _mode);
    }

    let mut result = img.clone();

    if let Some(fc) = fill {
        // Corner pieslices
        result = op_draw_pieslice(
            &result,
            x0,
            y0,
            x0 + d,
            y0 + d,
            180.0,
            270.0,
            Some(fc),
            None,
            0,
            _mode,
        )?;
        result = op_draw_pieslice(
            &result,
            x1 - d,
            y0,
            x1,
            y0 + d,
            270.0,
            360.0,
            Some(fc),
            None,
            0,
            _mode,
        )?;
        result = op_draw_pieslice(
            &result,
            x1 - d,
            y1 - d,
            x1,
            y1,
            0.0,
            90.0,
            Some(fc),
            None,
            0,
            _mode,
        )?;
        result = op_draw_pieslice(
            &result,
            x0,
            y1 - d,
            x0 + d,
            y1,
            90.0,
            180.0,
            Some(fc),
            None,
            0,
            _mode,
        )?;
        // Center body rectangle
        result = op_draw_rectangle(&result, x0 + r, y0, x1 - r, y1, Some(fc), None, 1, _mode)?;
        // Side rectangles
        if x1 - r > x0 + r + 1 {
            result = op_draw_rectangle(
                &result,
                x0 + r + 1,
                y0,
                x1 - r - 1,
                y1,
                Some(fc),
                None,
                1,
                _mode,
            )?;
        }
        let rect_left = if x0 + r > x0 { x0 + r } else { x0 + 1 };
        if rect_left < x1 - r {
            result = op_draw_rectangle(
                &result,
                x0,
                y0 + r,
                rect_left,
                y1 - r,
                Some(fc),
                None,
                1,
                _mode,
            )?;
            result = op_draw_rectangle(
                &result,
                x1 - r,
                y0 + r,
                x1,
                y1 - r,
                Some(fc),
                None,
                1,
                _mode,
            )?;
        }
    }

    if let Some(oc) = outline {
        // Corner arcs
        result = op_draw_arc(
            &result,
            x0,
            y0,
            x0 + d,
            y0 + d,
            180.0,
            270.0,
            Some(oc),
            1,
            _mode,
        )?;
        result = op_draw_arc(
            &result,
            x1 - d,
            y0,
            x1,
            y0 + d,
            270.0,
            360.0,
            Some(oc),
            1,
            _mode,
        )?;
        result = op_draw_arc(
            &result,
            x1 - d,
            y1 - d,
            x1,
            y1,
            0.0,
            90.0,
            Some(oc),
            1,
            _mode,
        )?;
        result = op_draw_arc(
            &result,
            x0,
            y1 - d,
            x0 + d,
            y1,
            90.0,
            180.0,
            Some(oc),
            1,
            _mode,
        )?;
        // Edge lines
        result = op_draw_line(&result, x0 + r, y0, x1 - r, y0, oc, 1, _mode)?;
        result = op_draw_line(&result, x1, y0 + r, x1, y1 - r, oc, 1, _mode)?;
        result = op_draw_line(&result, x1 - r, y1, x0 + r, y1, oc, 1, _mode)?;
        result = op_draw_line(&result, x0, y1 - r, x0, y0 + r, oc, 1, _mode)?;
    }

    Ok(result)
}

pub fn op_draw_ellipse(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    _precision_bits: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_preserve_p_mode(img, _mode, |canvas| {
        draw_ellipse_on_canvas(canvas, x0, y0, x1, y1, fill, outline);
    }))
}

pub fn op_draw_circle(
    img: &DynamicImage,
    cx: i32,
    cy: i32,
    radius: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    op_draw_ellipse(
        img,
        cx - radius,
        cy - radius,
        cx + radius,
        cy + radius,
        fill,
        outline,
        width,
        _mode,
    )
}

pub fn op_draw_polygon(
    img: &DynamicImage,
    points: &[(i32, i32)],
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    _precision_bits: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let pts = points.to_vec();
    Ok(draw_preserve_p_mode(img, _mode, |canvas| {
        draw_polygon_on_canvas(canvas, &pts, fill, outline);
    }))
}

pub fn op_draw_arc(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    _precision_bits: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let fc = fill.unwrap_or((0, 0, 0, 255));
    Ok(draw_preserve_p_mode(img, _mode, |canvas| {
        draw_arc_on_canvas(canvas, x0, y0, x1, y1, start, end, fc);
    }))
}

pub fn op_draw_chord(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let Some(fc) = fill {
        op_draw_pieslice(
            img,
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            Some(fc),
            outline,
            width,
            _mode,
        )
    } else {
        op_draw_arc(
            img,
            x0,
            y0,
            x1,
            y1,
            start,
            end,
            outline.or(Some((0, 0, 0, 255))),
            width,
            _mode,
        )
    }
}

pub fn op_draw_pieslice(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    _precision_bits: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_preserve_p_mode(img, _mode, |canvas| {
        draw_pieslice_on_canvas(canvas, x0, y0, x1, y1, start, end, fill, outline);
    }))
}

pub fn op_draw_point(
    img: &DynamicImage,
    points: &[(i32, i32)],
    fill: (u8, u8, u8, u8),
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let pts = points.to_vec();
    Ok(draw_preserve_p_mode(img, _mode, |canvas| {
        let (img_w, img_h) = (canvas.width(), canvas.height());
        for &(x, y) in &pts {
            plot(canvas, x, y, fill, img_w, img_h, false);
        }
    }))
}
