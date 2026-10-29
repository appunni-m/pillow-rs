use super::primitives::draw_line_on_canvas;
use crate::draw::{DrawCanvas, bresenham_line, scanline_polygon_fill};

pub(super) struct BinaryMask {
    width: u32,
    height: u32,
    pixels: Vec<bool>,
}

impl BinaryMask {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![false; width as usize * height as usize],
        }
    }

    fn contains(&self, x: u32, y: u32) -> bool {
        self.pixels[y as usize * self.width as usize + x as usize]
    }
}

impl DrawCanvas for BinaryMask {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn put_rgba(&mut self, x: u32, y: u32, _color: [u8; 4]) {
        self.pixels[y as usize * self.width as usize + x as usize] = true;
    }
}

pub(super) struct MaskedCanvas<'a, C> {
    canvas: &'a mut C,
    mask: &'a BinaryMask,
}

impl<C: DrawCanvas> DrawCanvas for MaskedCanvas<'_, C> {
    fn width(&self) -> u32 {
        self.canvas.width()
    }

    fn height(&self) -> u32 {
        self.canvas.height()
    }

    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if self.mask.contains(x, y) {
            self.canvas.put_rgba(x, y, color);
        }
    }
}

/// Draw a polygon directly on a canvas.
pub(super) fn draw_polygon_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    points: &[(i32, i32)],
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
) {
    let (img_w, img_h) = (canvas.width(), canvas.height());
    if points.is_empty() {
        return;
    }

    if let Some(fc) = fill {
        scanline_polygon_fill(canvas, points, fc, img_w, img_h, false);
    }

    let Some(outline_color) = outline.filter(|color| Some(*color) != fill && width != 0) else {
        return;
    };
    if width == 1 {
        for index in 0..points.len() {
            let (x0, y0) = points[index];
            let (x1, y1) = points[(index + 1) % points.len()];
            bresenham_line(canvas, x0, y0, x1, y1, outline_color, img_w, img_h, false);
        }
        return;
    }

    // ImageDraw.polygon masks a double-width outline with the filled polygon
    // so the requested stroke grows inward instead of expanding the shape.
    let mut mask = BinaryMask::new(img_w, img_h);
    scanline_polygon_fill(&mut mask, points, (255, 255, 255, 255), img_w, img_h, false);
    let mut masked = MaskedCanvas {
        canvas,
        mask: &mask,
    };
    let stroke_width = width.saturating_mul(2).saturating_sub(1);
    for index in 0..points.len() {
        let (x0, y0) = points[index];
        let (x1, y1) = points[(index + 1) % points.len()];
        draw_line_on_canvas(&mut masked, x0, y0, x1, y1, outline_color, stroke_width);
    }
}
