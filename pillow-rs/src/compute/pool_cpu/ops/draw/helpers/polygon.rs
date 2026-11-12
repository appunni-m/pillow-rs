use super::primitives::draw_line_on_canvas;
use crate::draw::{
    DrawCanvas, bresenham_line, plot, scanline_polygon_fill, wide_line_polygon_points_signed,
};

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
// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
pub(super) fn draw_polygon_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    points: &[(i32, i32)],
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    outline_width: Option<i32>,
) {
    let (img_w, img_h) = (canvas.width(), canvas.height());
    if points.is_empty() {
        return;
    }

    if let Some(fc) = fill {
        scanline_polygon_fill(canvas, points, fc, img_w, img_h, false);
    }

    let Some(outline_color) =
        outline.filter(|color| Some(*color) != fill && (width != 0 || outline_width.is_some()))
    else {
        return;
    };
    if let Some(outline_width) = outline_width {
        draw_polygon_outline_with_signed_width(
            canvas,
            points,
            outline_color,
            outline_width,
            img_w,
            img_h,
        );
        return;
    }
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
    let stroke_width = width.saturating_mul(2).saturating_sub(1);
    for index in 0..points.len() {
        let (x0, y0) = points[index];
        let (x1, y1) = points[(index + 1) % points.len()];
        if x0 == x1 && y0 == y1 {
            plot(canvas, x0, y0, outline_color, img_w, img_h, false);
            continue;
        }
        let mut masked = MaskedCanvas {
            canvas,
            mask: &mask,
        };
        draw_line_on_canvas(&mut masked, x0, y0, x1, y1, outline_color, stroke_width);
    }
}

fn draw_polygon_outline_with_signed_width<C: DrawCanvas>(
    canvas: &mut C,
    points: &[(i32, i32)],
    color: (u8, u8, u8, u8),
    width: i32,
    image_width: u32,
    image_height: u32,
) {
    if width == 1 {
        for index in 0..points.len() {
            let (x0, y0) = points[index];
            let (x1, y1) = points[(index + 1) % points.len()];
            bresenham_line(
                canvas,
                x0,
                y0,
                x1,
                y1,
                color,
                image_width,
                image_height,
                false,
            );
        }
        return;
    }

    let mut mask = BinaryMask::new(image_width, image_height);
    scanline_polygon_fill(
        &mut mask,
        points,
        (255, 255, 255, 255),
        image_width,
        image_height,
        false,
    );
    for index in 0..points.len() {
        let (x0, y0) = points[index];
        let (x1, y1) = points[(index + 1) % points.len()];
        let Some(stroke_points) = wide_line_polygon_points_signed(x0, y0, x1, y1, width) else {
            plot(canvas, x0, y0, color, image_width, image_height, false);
            continue;
        };
        let mut masked = MaskedCanvas {
            canvas,
            mask: &mask,
        };
        scanline_polygon_fill(
            &mut masked,
            &stroke_points,
            color,
            image_width,
            image_height,
            false,
        );
    }
}
