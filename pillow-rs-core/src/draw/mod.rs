//! ImageDraw — drawing primitives on images.
//! Implements line, rectangle, ellipse, polygon, point, text.
//! Uses Bresenham-style algorithms for pixel-perfect rendering.

use image::{DynamicImage, Rgba, RgbaImage};

use crate::error::PilError;
use crate::image::Image;

/// Drawing context wrapping an Image.
/// PIL: `draw = ImageDraw.Draw(image)` then `draw.line(...)`, `draw.rectangle(...)`, etc.
#[derive(Debug)]
pub struct Draw {
    image: Image,
    /// Original mode before draw canvas created. Used to convert back on image_clone().
    orig_mode: Option<String>,
}

impl Draw {
    /// Create a new drawing context.
    pub fn new(image: Image) -> Self {
        let mode = {
            let clone = image.clone();
            clone.mode().ok()
        };
        Draw { image, orig_mode: mode }
    }

    /// Draw a line from (x0,y0) to (x1,y1). Bresenham's algorithm.
    pub fn line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: (u8, u8, u8, u8),
        width: u32,
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (w, h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();

        if width <= 1 {
            bresenham_line(&mut canvas, x0, y0, x1, y1, fill, w, h);
        } else {
            // Thick line: draw multiple offset lines
            let half = (width as i32) / 2;
            for offset in -half..=half {
                bresenham_line(&mut canvas, x0 + offset, y0, x1 + offset, y1, fill, w, h);
                bresenham_line(&mut canvas, x0, y0 + offset, x1, y1 + offset, fill, w, h);
            }
        }

        self.image = Image::Loaded(image::DynamicImage::ImageRgba8(canvas), None);
        Ok(())
    }

    /// Draw a rectangle. Filled if fill is provided, outline otherwise.
    pub fn rectangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();

        let x0 = x0.clamp(0, img_w as i32 - 1);
        let y0 = y0.clamp(0, img_h as i32 - 1);
        let x1 = x1.clamp(0, img_w as i32);
        let y1 = y1.clamp(0, img_h as i32);

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
                    plot(&mut canvas, px, y0 - w, oc, img_w, img_h);
                    plot(&mut canvas, px, y1 + w, oc, img_w, img_h);
                }
                // Sides
                for py in y0 - w..=y1 + w {
                    plot(&mut canvas, x0 - w, py, oc, img_w, img_h);
                    plot(&mut canvas, x1 + w, py, oc, img_w, img_h);
                }
            }
        }

        self.image = Image::Loaded(image::DynamicImage::ImageRgba8(canvas), None);
        Ok(())
    }

    /// Draw an ellipse within the bounding box.
    /// Uses PIL's exact Bresenham quarter-ellipse algorithm with step-2 coordinate system.
    /// Matches PIL pixel-for-pixel by using the same `a=x1-x0, b=y1-y0` scaling.
    pub fn ellipse(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();

        let cx = (x0 + x1) as f64 / 2.0;
        let cy = (y0 + y1) as f64 / 2.0;
        let rx = ((x1 - x0) as f64 / 2.0).abs();
        let ry = ((y1 - y0) as f64 / 2.0).abs();

        if rx < 1.0 || ry < 1.0 {
            return Ok(());
        }

        // Common Bresenham ellipse parameters (used by both fill and outline)
        let a = x1 - x0;
        let b = y1 - y0;
        if a <= 0 || b <= 0 {
            return Ok(());
        }
        let cx_i = ((x0 + x1) / 2) as i32;
        let cy_i = ((y0 + y1) / 2) as i32;

        // PIL's quarter_init: start at (a, b%2), with ex=a%2, ey=b
        let ex = a % 2;
        let ey = b;
        let a2 = a as i64 * a as i64;
        let b2 = b as i64 * b as i64;
        let a2b2 = a2 * b2;
        let quarter_delta = |x: i64, y: i64| -> i64 {
            (a2 * y * y + b2 * x * x - a2b2).abs()
        };

        // Fill using PIL's exact Bresenham quarter-ellipse algorithm.
        if let Some(fc) = fill {
            // PR = previous right (outer x-bound from previous iteration)
            // PY = previous y (the Y-level from previous iteration)
            let mut pr = a as i64;
            let mut py = 0i64;
            let mut qx = a;
            let mut qy = b % 2;
            let mut finished = false;
            while !finished {
                // Positive Y: bottom half (center down to y1)
                let y_pos = y0 + ((py + b as i64) / 2) as i32;
                // Negative Y: top half (y0 up to center, reflected)
                let y_neg = y0 + ((-py + b as i64) / 2) as i32;

                // For filled ellipse: combined segments cover [-pr, pr] at each Y-level
                // x_bound = pr/2 (since quarter coordinates divide by 2 for image pixels)
                let xb = (pr / 2) as i32;
                let left = (cx_i - xb).max(x0);
                let right = (cx_i + xb).min(x1);
                for &y_img in &[y_pos, y_neg] {
                    if y_img >= y0 && y_img <= y1 && xb > 0 {
                        for x in left..=right {
                            if x >= 0 && y_img >= 0 && (x as u32) < img_w && (y_img as u32) < img_h {
                                canvas.put_pixel(x as u32, y_img as u32, Rgba([fc.0, fc.1, fc.2, fc.3]));
                            }
                        }
                    }
                }

                // Move to next quarter y-level: consume all quarter points with cy <= py
                loop {
                    if qx < 0 {
                        finished = true;
                        break;
                    }
                    if qx == ex && qy == ey {
                        finished = true;
                        break;
                    }
                    // Try 3 candidates: (qx, qy+2), (qx-2, qy+2), (qx-2, qy)
                    let mut nx = qx;
                    let mut ny = qy + 2;
                    let mut ndelta = quarter_delta(nx as i64, ny as i64);
                    if qx > 1 {
                        let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                        if ndelta > d1 {
                            nx = qx - 2; ny = qy + 2; ndelta = d1;
                        }
                        let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                        if ndelta > d2 {
                            nx = qx - 2; ny = qy;
                        }
                    }
                    if ny > ey {
                        finished = true;
                        break;
                    }
                    // If this new point is at a new y-level (beyond current py), update
                    if ny as i64 > py {
                        pr = nx as i64;
                        py = ny as i64;
                        qx = nx;
                        qy = ny;
                        break;
                    }
                    // Same y-level: consume and continue (inner loop for filled, this
                    // updates the inner boundary, which is at leftmost=0)
                    qx = nx;
                    qy = ny;
                }
            }
        }

        // Outline via Bresenham boundary + edge detection (matches PIL's ellipse outline)
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
                    if qx < 0 { finished = true; break; }
                    if qx == ex && qy == ey { finished = true; break; }
                    let mut nx = qx;
                    let mut ny = qy + 2;
                    let mut ndelta = quarter_delta(nx as i64, ny as i64);
                    if qx > 1 {
                        let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                        if ndelta > d1 { nx = qx - 2; ny = qy + 2; ndelta = d1; }
                        let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                        if ndelta > d2 { nx = qx - 2; ny = qy; }
                    }
                    if ny > ey { finished = true; break; }
                    if ny as i64 > py { pr = nx as i64; py = ny as i64; qx = nx; qy = ny; break; }
                    qx = nx; qy = ny;
                }
            }
            // Edge detection on filled ellipse to extract outline
            let iw = img_w as i32;
            let ih = img_h as i32;
            for y in 0..ih {
                for x in 0..iw {
                    let idx = (y as usize) * (img_w as usize) + (x as usize);
                    if !filled[idx] { continue; }
                    let lf = x > 0 && filled[(y as usize) * (img_w as usize) + ((x - 1) as usize)];
                    let rf = x < iw - 1 && filled[(y as usize) * (img_w as usize) + ((x + 1) as usize)];
                    let uf = y > 0 && filled[((y - 1) as usize) * (img_w as usize) + (x as usize)];
                    let df = y < ih - 1 && filled[((y + 1) as usize) * (img_w as usize) + (x as usize)];
                    if !lf || !rf || !uf || !df {
                        plot(&mut canvas, x, y, oc, img_w, img_h);
                    }
                }
            }
        }

        self.image = Image::Loaded(image::DynamicImage::ImageRgba8(canvas), None);
        Ok(())
    }

    /// Draw a polygon. Filled if fill provided.
    pub fn polygon(
        &mut self,
        points: &[(i32, i32)],
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        if points.len() < 3 {
            return Ok(());
        }
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();

        // Outline
        if let Some(oc) = outline {
            for i in 0..points.len() {
                let (x0, y0) = points[i];
                let (x1, y1) = points[(i + 1) % points.len()];
                bresenham_line(&mut canvas, x0, y0, x1, y1, oc, img_w, img_h);
            }
        }

        // Fill: even-odd rule point-in-polygon test
        if let Some(fc) = fill {
            // Find bounds
            let mut min_x = i32::MAX;
            let mut max_x = i32::MIN;
            let mut min_y = i32::MAX;
            let mut max_y = i32::MIN;
            for &(px, py) in points {
                min_x = min_x.min(px);
                max_x = max_x.max(px);
                min_y = min_y.min(py);
                max_y = max_y.max(py);
            }

            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    if x >= 0 && y >= 0 && (x as u32) < img_w && (y as u32) < img_h
                        && point_in_polygon(x, y, points) {
                            canvas.put_pixel(x as u32, y as u32, Rgba([fc.0, fc.1, fc.2, fc.3]));
                        }
                }
            }
        }

        self.image = Image::Loaded(image::DynamicImage::ImageRgba8(canvas), None);
        Ok(())
    }

    /// Draw a single point.
    pub fn point(&mut self, points: &[(i32, i32)], fill: (u8, u8, u8, u8)) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();
        for &(x, y) in points {
            plot(&mut canvas, x, y, fill, img_w, img_h);
        }
        self.image = Image::Loaded(image::DynamicImage::ImageRgba8(canvas), None);
        Ok(())
    }

    /// Return a clone of the current image state, converted back to original mode.
    pub fn image_clone(&self) -> Image {
        let img = self.image.clone();
        if let Some(ref orig) = self.orig_mode {
            if let Ok(current) = img.mode() {
                if current != *orig && *orig != "RGBA" {
                    // Convert RGBA back to RGB/L if that was the original mode
                    if let Ok(img_loaded) = img.materialize() {
                        let converted = match orig.as_str() {
                            "RGB" => DynamicImage::ImageRgb8(img_loaded.to_rgb8()),
                            "L" => DynamicImage::ImageLuma8(
                                crate::color::pil_grayscale(&img_loaded)
                            ),
                            _ => img_loaded,
                        };
                        return Image::Loaded(converted, None);
                    }
                }
            }
        }
        img
    }

    /// Draw an arc (partial ellipse outline).
    /// Uses the same Bresenham quarter-ellipse generator as the ellipse fill,
    /// then performs edge detection to find boundary pixels and filters by angle.
    pub fn arc(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, start: f64, end: f64, fill: (u8, u8, u8, u8), _width: u32) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();

        let cx = (x0 + x1) as f64 / 2.0;
        let cy = (y0 + y1) as f64 / 2.0;
        let a = x1 - x0;
        let b = y1 - y0;
        if a <= 0 || b <= 0 {
            return Ok(());
        }

        // Normalize angles to 0..360
        let mut s = start % 360.0;
        if s < 0.0 { s += 360.0; }
        let mut e = end % 360.0;
        if e < 0.0 { e += 360.0; }
        let angle_in_range = |angle: f64| -> bool {
            let mut a = angle % 360.0;
            if a < 0.0 { a += 360.0; }
            if s <= e { a >= s && a <= e } else { a >= s || a <= e }
        };

        let cx_i = ((x0 + x1) / 2) as i32;
        let cy_i = ((y0 + y1) / 2) as i32;

        // Step 1: Compute full ellipse fill using the Bresenham generator
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

            // Advance quarter generator
            loop {
                if qx < 0 { finished = true; break; }
                if qx == ex && qy == ey { finished = true; break; }
                let mut nx = qx;
                let mut ny = qy + 2;
                let mut ndelta = quarter_delta(nx as i64, ny as i64);
                if qx > 1 {
                    let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                    if ndelta > d1 { nx = qx - 2; ny = qy + 2; ndelta = d1; }
                    let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                    if ndelta > d2 { nx = qx - 2; ny = qy; }
                }
                if ny > ey { finished = true; break; }
                if ny as i64 > py {
                    pr = nx as i64;
                    py = ny as i64;
                    qx = nx; qy = ny;
                    break;
                }
                qx = nx; qy = ny;
            }
        }

        // Step 2: Edge detection — boundary pixels have at least one unfilled 4-connected neighbor
        let iw = img_w as i32;
        let ih = img_h as i32;
        for y in 0..ih {
            for x in 0..iw {
                let idx = (y as usize) * (img_w as usize) + (x as usize);
                if !filled[idx] { continue; }
                // Check 4-connected neighbors
                let left_filled = x > 0 && filled[(y as usize) * (img_w as usize) + ((x - 1) as usize)];
                let right_filled = x < iw - 1 && filled[(y as usize) * (img_w as usize) + ((x + 1) as usize)];
                let up_filled = y > 0 && filled[((y - 1) as usize) * (img_w as usize) + (x as usize)];
                let down_filled = y < ih - 1 && filled[((y + 1) as usize) * (img_w as usize) + (x as usize)];
                let is_boundary = !left_filled || !right_filled || !up_filled || !down_filled;
                if is_boundary {
                    let angle = (y as f64 - cy).atan2(x as f64 - cx).to_degrees();
                    if angle_in_range(angle) {
                        canvas.put_pixel(x as u32, y as u32, Rgba([fill.0, fill.1, fill.2, fill.3]));
                    }
                }
            }
        }

        self.image = Image::Loaded(image::DynamicImage::ImageRgba8(canvas), None);
        Ok(())
    }

    /// Draw a chord (arc + filled to center).
    pub fn chord(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, start: f64, end: f64, fill: Option<(u8, u8, u8, u8)>, outline: Option<(u8, u8, u8, u8)>, _width: u32) -> Result<(), PilError> {
        // Simplified: draw arc outline + fill by drawing pie slice
        if let Some(fc) = fill {
            self.pieslice(x0, y0, x1, y1, start, end, Some(fc), outline, 1)?;
        } else {
            self.arc(x0, y0, x1, y1, start, end, outline.unwrap_or((0, 0, 0, 255)), 1)?;
        }
        Ok(())
    }

    /// Draw a pieslice. Uses the Bresenham ellipse fill with angle clipping.
    pub fn pieslice(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, start: f64, end: f64, fill: Option<(u8, u8, u8, u8)>, outline: Option<(u8, u8, u8, u8)>, _width: u32) -> Result<(), PilError> {
        let img = self.image.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();
        let cx = (x0 + x1) as f64 / 2.0;
        let cy = (y0 + y1) as f64 / 2.0;

        let a = x1 - x0;
        let b = y1 - y0;
        if a <= 0 || b <= 0 {
            return Ok(());
        }

        // Normalize angles
        let mut s = start % 360.0;
        if s < 0.0 { s += 360.0; }
        let mut e = end % 360.0;
        if e < 0.0 { e += 360.0; }
        let angle_in_range = |angle: f64| -> bool {
            let mut a = angle % 360.0;
            if a < 0.0 { a += 360.0; }
            if s <= e { a >= s && a <= e } else { a >= s || a <= e }
        };

        let cx_i = ((x0 + x1) / 2) as i32;
        let cy_i = ((y0 + y1) / 2) as i32;

        // Use the Bresenham quarter generator for the fill, filter by angle
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
                                if x >= 0 && y_img >= 0 && (x as u32) < img_w && (y_img as u32) < img_h {
                                    // Always include the center pixel (vertex of pie wedge)
                                    if x == cx_i && y_img == cy_i {
                                        canvas.put_pixel(x as u32, y_img as u32, Rgba([fc.0, fc.1, fc.2, fc.3]));
                                    } else {
                                        let angle = (y_img as f64 - cy).atan2(x as f64 - cx).to_degrees();
                                        if angle_in_range(angle) {
                                            canvas.put_pixel(x as u32, y_img as u32, Rgba([fc.0, fc.1, fc.2, fc.3]));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Advance quarter generator
                loop {
                    if qx < 0 { finished = true; break; }
                    if qx == ex && qy == ey { finished = true; break; }
                    let mut nx = qx;
                    let mut ny = qy + 2;
                    let mut ndelta = quarter_delta(nx as i64, ny as i64);
                    if qx > 1 {
                        let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                        if ndelta > d1 { nx = qx - 2; ny = qy + 2; ndelta = d1; }
                        let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                        if ndelta > d2 { nx = qx - 2; ny = qy; }
                    }
                    if ny > ey { finished = true; break; }
                    if ny as i64 > py {
                        pr = nx as i64;
                        py = ny as i64;
                        qx = nx; qy = ny;
                        break;
                    }
                    qx = nx; qy = ny;
                }
            }
        }

        // Outline: draw radii + arc edge using Bresenham
        if let Some(oc) = outline {
            // Radius lines from center to arc endpoints
            for angle_deg in [start, end] {
                let rad = angle_deg.to_radians();
                let ax = (cx + (a as f64 / 2.0) * rad.cos()).round() as i32;
                let ay = (cy + (b as f64 / 2.0) * rad.sin()).round() as i32;
                bresenham_line(&mut canvas, cx_i, cy_i, ax, ay, oc, img_w, img_h);
            }
            // Arc edge: use the same Bresenham + edge detection approach as arc()
            // Re-use the arc filling logic but with outline color and the same angle range
            // For simplicity, draw a filled ellipse, mask to angle, then edge-detect for outline
            let mut filled = vec![false; (img_w * img_h) as usize];
            // Reset quarter generator
            qx = a; qy = b % 2; pr = a as i64; py = 0i64; finished = false;
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
                                    let angle = (y_img as f64 - cy).atan2(x as f64 - cx).to_degrees();
                                    if angle_in_range(angle) {
                                        filled[(y_img as usize) * (img_w as usize) + (x as usize)] = true;
                                    }
                                }
                            }
                        }
                    }
                }
                loop {
                    if qx < 0 { finished = true; break; }
                    if qx == ex && qy == ey { finished = true; break; }
                    let mut nx = qx;
                    let mut ny = qy + 2;
                    let mut ndelta = quarter_delta(nx as i64, ny as i64);
                    if qx > 1 {
                        let d1 = quarter_delta((qx - 2) as i64, (qy + 2) as i64);
                        if ndelta > d1 { nx = qx - 2; ny = qy + 2; ndelta = d1; }
                        let d2 = quarter_delta((qx - 2) as i64, qy as i64);
                        if ndelta > d2 { nx = qx - 2; ny = qy; }
                    }
                    if ny > ey { finished = true; break; }
                    if ny as i64 > py {
                        pr = nx as i64;
                        py = ny as i64;
                        qx = nx; qy = ny;
                        break;
                    }
                    qx = nx; qy = ny;
                }
            }
            // Edge detection
            let iw = img_w as i32;
            let ih = img_h as i32;
            for y in 0..ih {
                for x in 0..iw {
                    let idx = (y as usize) * (img_w as usize) + (x as usize);
                    if !filled[idx] { continue; }
                    let left_f = x > 0 && filled[(y as usize) * (img_w as usize) + ((x - 1) as usize)];
                    let right_f = x < iw - 1 && filled[(y as usize) * (img_w as usize) + ((x + 1) as usize)];
                    let up_f = y > 0 && filled[((y - 1) as usize) * (img_w as usize) + (x as usize)];
                    let down_f = y < ih - 1 && filled[((y + 1) as usize) * (img_w as usize) + (x as usize)];
                    if !left_f || !right_f || !up_f || !down_f {
                        canvas.put_pixel(x as u32, y as u32, Rgba([oc.0, oc.1, oc.2, oc.3]));
                    }
                }
            }
        }

        self.image = Image::Loaded(image::DynamicImage::ImageRgba8(canvas), None);
        Ok(())
    }

    /// Draw a circle.
    pub fn circle(&mut self, cx: i32, cy: i32, radius: f64, fill: Option<(u8, u8, u8, u8)>, outline: Option<(u8, u8, u8, u8)>, _width: u32) -> Result<(), PilError> {
        let r = radius as i32;
        self.ellipse(cx - r, cy - r, cx + r, cy + r, fill, outline, 1)
    }

    /// Draw a rounded rectangle. Composes corner pieslices/arcs and rectangles
    /// matching PIL's Python algorithm in ImageDraw.py.
    pub fn rounded_rectangle(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, radius: f64, fill: Option<(u8, u8, u8, u8)>, outline: Option<(u8, u8, u8, u8)>, _width: u32) -> Result<(), PilError> {
        let r = radius.round() as i32;
        let d = r * 2;
        if d <= 0 || x1 <= x0 + 1 || y1 <= y0 + 1 {
            // No corner curve, just draw rectangle
            return self.rectangle(x0, y0, x1, y1, fill, outline, 1);
        }

        // Draw filled body first, then outline on top
        if let Some(fc) = fill {
            // Corner pieslices
            self.pieslice(x0, y0, x0 + d, y0 + d, 180.0, 270.0, fill, None, 0)?;  // TL
            self.pieslice(x1 - d, y0, x1, y0 + d, 270.0, 360.0, fill, None, 0)?;  // TR
            self.pieslice(x1 - d, y1 - d, x1, y1, 0.0, 90.0, fill, None, 0)?;     // BR
            self.pieslice(x0, y1 - d, x0 + d, y1, 90.0, 180.0, fill, None, 0)?;   // BL
            // Center body rectangle
            self.rectangle(x0 + r, y0, x1 - r, y1, fill, None, 1)?;
            // Side rectangles (left/right fill between corners)
            if x1 - r - 1 >= x0 + r + 1 {
                self.rectangle(x0 + r + 1, y0, x1 - r - 1, y1, fill, None, 1)?;
            }
            let rect_left = if x0 + r > x0 { x0 + r } else { x0 + 1 };
            if rect_left < x1 - r {
                self.rectangle(x0, y0 + r, rect_left, y1 - r, fill, None, 1)?;
                self.rectangle(x1 - r, y0 + r, x1, y1 - r, fill, None, 1)?;
            }
        }

        if let Some(oc) = outline {
            // Corner arcs for outline
            self.arc(x0, y0, x0 + d, y0 + d, 180.0, 270.0, oc, 1)?;
            self.arc(x1 - d, y0, x1, y0 + d, 270.0, 360.0, oc, 1)?;
            self.arc(x1 - d, y1 - d, x1, y1, 0.0, 90.0, oc, 1)?;
            self.arc(x0, y1 - d, x0 + d, y1, 90.0, 180.0, oc, 1)?;
            // Edge lines for outline
            self.line(x0 + r, y0, x1 - r, y0, oc, 1)?;  // top
            self.line(x1, y0 + r, x1, y1 - r, oc, 1)?;  // right
            self.line(x1 - r, y1, x0 + r, y1, oc, 1)?;  // bottom
            self.line(x0, y1 - r, x0, y0 + r, oc, 1)?;   // left
        }

        Ok(())
    }

    /// Draw text at position (x, y) using a font.
    pub fn text(&mut self, x: i32, y: i32, text: &str, font: &crate::font::Font, fill: (u8, u8, u8, u8)) -> Result<(), PilError> {
        let (w, h, pixels) = font.render_text(text, fill, 0.0);
        if w == 0 || h == 0 { return Ok(()); }
        let img = self.image.materialize()?;
        let mut canvas = img.to_rgba8();
        let (img_w, img_h) = (canvas.width(), canvas.height());

        for py in 0..h {
            for px in 0..w {
                let off = ((py * w + px) * 4) as usize;
                if off + 3 < pixels.len() {
                    let sa = pixels[off + 3];
                    if sa == 0 { continue; }
                    let dx = (x as u32 + px).min(img_w - 1);
                    let dy = (y as u32 + py).min(img_h - 1);
                    if sa == 255 {
                        canvas.put_pixel(dx, dy, Rgba([pixels[off], pixels[off+1], pixels[off+2], 255]));
                    } else {
                        let dp = canvas.get_pixel(dx, dy);
                        let inv = 255u16 - sa as u16;
                        canvas.put_pixel(dx, dy, Rgba([
                            blend_u8(pixels[off], dp[0], sa, inv),
                            blend_u8(pixels[off+1], dp[1], sa, inv),
                            blend_u8(pixels[off+2], dp[2], sa, inv),
                            255,
                        ]));
                    }
                }
            }
        }
        self.image = Image::Loaded(image::DynamicImage::ImageRgba8(canvas), None);
        Ok(())
    }

    /// Consume the drawing context and return the modified image.
    pub fn into_image(self) -> Image {
        self.image
    }
}

// ── Drawing primitives ──────────────────────────────────────────────

/// Bresenham's line algorithm with clamping.
fn bresenham_line(canvas: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: (u8, u8, u8, u8), w: u32, h: u32) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        plot(canvas, x, y, color, w, h);
        if x == x1 && y == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

/// Plot a single pixel with bounds checking and alpha blending.
#[inline]
fn plot(canvas: &mut RgbaImage, x: i32, y: i32, color: (u8, u8, u8, u8), w: u32, h: u32) {
    if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    if color.3 == 255 {
        canvas.put_pixel(x, y, Rgba([color.0, color.1, color.2, 255]));
    } else {
        let existing = canvas.get_pixel(x, y);
        let a = color.3 as u16;
        let inv = 255u16 - a;
        canvas.put_pixel(x, y, Rgba([
            ((color.0 as u16 * a + existing[0] as u16 * inv) / 255) as u8,
            ((color.1 as u16 * a + existing[1] as u16 * inv) / 255) as u8,
            ((color.2 as u16 * a + existing[2] as u16 * inv) / 255) as u8,
            color.3.max(existing[3]),
        ]));
    }
}

/// Midpoint ellipse outline algorithm.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw_ellipse_outline(canvas: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, color: (u8, u8, u8, u8), w: u32, h: u32) {
    if rx <= 0 || ry <= 0 { return; }
    let rx = rx as f64;
    let ry = ry as f64;

    // Simple approach: draw at angle increments
    // Use adaptive step based on radius
    let steps = ((rx + ry) * 1.5) as i32;
    for i in 0..steps {
        let angle = 2.0 * std::f64::consts::PI * i as f64 / steps as f64;
        let x = (cx as f64 + rx * angle.cos()).round() as i32;
        let y = (cy as f64 + ry * angle.sin()).round() as i32;
        plot(canvas, x, y, color, w, h);
    }
}

#[inline]
fn blend_u8(src: u8, dst: u8, alpha: u8, inv_alpha: u16) -> u8 {
    let a = alpha as u16;
    (((src as u16 * a) + (dst as u16 * inv_alpha) + 127) / 255) as u8
}

/// Even-odd rule for point-in-polygon testing.
fn point_in_polygon(x: i32, y: i32, points: &[(i32, i32)]) -> bool {
    let mut inside = false;
    let n = points.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}
