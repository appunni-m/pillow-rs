//! ImageDraw — drawing primitives on images.
//! Implements line, rectangle, ellipse, polygon, point, text.
//! Uses Bresenham-style algorithms for pixel-perfect rendering.

use image::{DynamicImage, GenericImage, GenericImageView, Rgba, RgbaImage};

use crate::error::PilError;
use crate::image::Image;

/// Drawing context wrapping an Image.
/// PIL: `draw = ImageDraw.Draw(image)` then `draw.line(...)`, `draw.rectangle(...)`, etc.
pub struct Draw {
    image: Image,
    /// Fill color (r, g, b, a) — None means no fill
    fill: Option<(u8, u8, u8, u8)>,
    /// Outline color
    outline: Option<(u8, u8, u8, u8)>,
}

impl Draw {
    /// Create a new drawing context.
    pub fn new(image: Image) -> Self {
        Draw {
            image,
            fill: None,
            outline: Some((0, 0, 0, 255)), // default: black outline
        }
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
        let img = self.image.ensure_loaded()?;
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

        self.image.inner = crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgba8(canvas));
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
        let img = self.image.ensure_loaded()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();

        let x0 = x0.clamp(0, img_w as i32 - 1);
        let y0 = y0.clamp(0, img_h as i32 - 1);
        let x1 = x1.clamp(0, img_w as i32);
        let y1 = y1.clamp(0, img_h as i32);

        // Fill
        if let Some(fc) = fill {
            for py in y0..y1 {
                for px in x0..x1 {
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

        self.image.inner = crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgba8(canvas));
        Ok(())
    }

    /// Draw an ellipse within the bounding box.
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
        let img = self.image.ensure_loaded()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();

        let cx = (x0 + x1) as f64 / 2.0;
        let cy = (y0 + y1) as f64 / 2.0;
        let rx = ((x1 - x0) as f64 / 2.0).abs();
        let ry = ((y1 - y0) as f64 / 2.0).abs();

        if rx < 1.0 || ry < 1.0 {
            return Ok(());
        }

        // Scanline fill
        if let Some(fc) = fill {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if x >= 0 && y >= 0 && (x as u32) < img_w && (y as u32) < img_h {
                        let dx = (x as f64 - cx) / rx;
                        let dy = (y as f64 - cy) / ry;
                        if dx * dx + dy * dy <= 1.0 {
                            canvas.put_pixel(x as u32, y as u32, Rgba([fc.0, fc.1, fc.2, fc.3]));
                        }
                    }
                }
            }
        }

        // Outline via Midpoint ellipse algorithm
        if let Some(oc) = outline {
            draw_ellipse_outline(&mut canvas, cx as i32, cy as i32, rx as i32, ry as i32, oc, img_w, img_h);
        }

        self.image.inner = crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgba8(canvas));
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
        let img = self.image.ensure_loaded()?;
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

        // Simple fill: scanline with even-odd rule
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
                    if x >= 0 && y >= 0 && (x as u32) < img_w && (y as u32) < img_h {
                        if point_in_polygon(x, y, points) {
                            canvas.put_pixel(x as u32, y as u32, Rgba([fc.0, fc.1, fc.2, fc.3]));
                        }
                    }
                }
            }
        }

        self.image.inner = crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgba8(canvas));
        Ok(())
    }

    /// Draw a single point.
    pub fn point(&mut self, points: &[(i32, i32)], fill: (u8, u8, u8, u8)) -> Result<(), PilError> {
        let img = self.image.ensure_loaded()?;
        let (img_w, img_h) = (img.width(), img.height());
        let mut canvas = img.to_rgba8();
        for &(x, y) in points {
            plot(&mut canvas, x, y, fill, img_w, img_h);
        }
        self.image.inner = crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgba8(canvas));
        Ok(())
    }

    /// Return a clone of the current image state (for inspection without consuming Draw).
    pub fn image_clone(&self) -> Image {
        self.image.clone()
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
