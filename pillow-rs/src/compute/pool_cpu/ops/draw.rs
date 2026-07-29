//! ImageDraw CPU operations — direct drawing in the destination's native layout.
//! These are called by the pipeline executor when a DrawXxx PipelineOp
//! is encountered. They implement the same algorithms as the Draw methods
//! in draw/mod.rs, but operate directly on DynamicImage to avoid circular
//! recursion (Draw methods now push PipelineOps).
//!
//! P-mode (palette-indexed) images are handled specially: drawing preserves
//! the Luma8 index buffer and writes palette index values directly, matching
//! PIL's behavior (fill colors use their R channel as the palette index).

use crate::draw::{DrawCanvas, bresenham_line, plot, round_down, round_up, scanline_polygon_fill};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

enum NativeDrawCanvas {
    L(GrayImage),
    LA(GrayAlphaImage),
    RGB(RgbImage),
    RGBA(RgbaImage),
}

impl NativeDrawCanvas {
    fn from_image(image: &DynamicImage) -> Self {
        match image {
            DynamicImage::ImageLuma8(pixels) => Self::L(pixels.clone()),
            DynamicImage::ImageLumaA8(pixels) => Self::LA(pixels.clone()),
            DynamicImage::ImageRgb8(pixels) => Self::RGB(pixels.clone()),
            DynamicImage::ImageRgba8(pixels) => Self::RGBA(pixels.clone()),
            _ => Self::RGBA(image.to_rgba8()),
        }
    }

    fn into_image(self) -> DynamicImage {
        match self {
            Self::L(pixels) => DynamicImage::ImageLuma8(pixels),
            Self::LA(pixels) => DynamicImage::ImageLumaA8(pixels),
            Self::RGB(pixels) => DynamicImage::ImageRgb8(pixels),
            Self::RGBA(pixels) => DynamicImage::ImageRgba8(pixels),
        }
    }
}

impl DrawCanvas for NativeDrawCanvas {
    fn width(&self) -> u32 {
        match self {
            Self::L(pixels) => pixels.width(),
            Self::LA(pixels) => pixels.width(),
            Self::RGB(pixels) => pixels.width(),
            Self::RGBA(pixels) => pixels.width(),
        }
    }

    fn height(&self) -> u32 {
        match self {
            Self::L(pixels) => pixels.height(),
            Self::LA(pixels) => pixels.height(),
            Self::RGB(pixels) => pixels.height(),
            Self::RGBA(pixels) => pixels.height(),
        }
    }

    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]) {
        match self {
            Self::L(pixels) => {
                pixels.put_pixel(x, y, crate::raster::Luma([color[0]]));
            }
            Self::LA(pixels) => {
                pixels.put_pixel(x, y, crate::raster::LumaA([color[0], color[3]]));
            }
            Self::RGB(pixels) => {
                pixels.put_pixel(x, y, crate::raster::Rgb([color[0], color[1], color[2]]));
            }
            Self::RGBA(pixels) => {
                pixels.put_pixel(x, y, crate::raster::Rgba(color));
            }
        }
    }
}

/// Draw directly in the destination's native byte layout.
fn draw_native<F>(img: &DynamicImage, draw_fn: F) -> DynamicImage
where
    F: Fn(&mut NativeDrawCanvas),
{
    let mut canvas = NativeDrawCanvas::from_image(img);
    draw_fn(&mut canvas);
    canvas.into_image()
}

/// Draw a line directly on a canvas (Bresenham).
fn draw_line_on_canvas<C: DrawCanvas>(
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

    let delta_x = i64::from(x1) - i64::from(x0);
    let delta_y = i64::from(y1) - i64::from(y0);
    if delta_x == 0 && delta_y == 0 {
        plot(canvas, x0, y0, fill, w, h, false);
        return;
    }

    // Pillow src/libImaging/Draw.c::ImagingDrawWideLine constructs an
    // asymmetric four-edge polygon for even widths. The two rounded ratios
    // are what keep an even requested width from becoming one pixel too wide.
    let squared_length = delta_x
        .saturating_mul(delta_x)
        .saturating_add(delta_y.saturating_mul(delta_y));
    let length = (squared_length as f64).sqrt();
    let half_width = f64::from(width.saturating_sub(1)) / 2.0;
    let ratio_max = f64::from(round_up(half_width)) / length;
    let ratio_min = f64::from(round_down(half_width)) / length;
    let offset_x_min = round_down(ratio_min * delta_y as f64);
    let offset_x_max = round_down(ratio_max * delta_y as f64);
    let offset_y_min = round_down(ratio_min * delta_x as f64);
    let offset_y_max = round_down(ratio_max * delta_x as f64);
    let points = [
        (
            x0.saturating_sub(offset_x_min),
            y0.saturating_add(offset_y_max),
        ),
        (
            x1.saturating_sub(offset_x_min),
            y1.saturating_add(offset_y_max),
        ),
        (
            x1.saturating_add(offset_x_max),
            y1.saturating_sub(offset_y_min),
        ),
        (
            x0.saturating_add(offset_x_max),
            y0.saturating_sub(offset_y_min),
        ),
    ];
    scanline_polygon_fill(canvas, &points, fill, w, h, false);
}

/// Draw a rectangle directly on a canvas.
fn draw_rect_on_canvas<C: DrawCanvas>(
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

    let visible_left = x0.max(0) as u32;
    let visible_top = y0.max(0) as u32;
    let visible_right = x1.min(img_w.saturating_sub(1) as i32);
    let visible_bottom = y1.min(img_h.saturating_sub(1) as i32);
    if visible_right < 0 || visible_bottom < 0 || visible_left >= img_w || visible_top >= img_h {
        return;
    }

    // Pillow's ImagingDrawRectangle treats both ends as inclusive.
    if let Some(fc) = fill {
        for py in visible_top..=visible_bottom as u32 {
            for px in visible_left..=visible_right as u32 {
                canvas.put_rgba(px, py, [fc.0, fc.1, fc.2, fc.3]);
            }
        }
    }

    // libImaging draws each additional outline ring inward. Computing the
    // distance from every visible pixel to the original inclusive edge is
    // equivalent to the C ring loop, while bounding work by the image size
    // even when callers provide very large coordinates or widths.
    if let Some(oc) = outline.filter(|_| width != 0) {
        let width = i64::from(width);
        for py in visible_top..=visible_bottom as u32 {
            for px in visible_left..=visible_right as u32 {
                let px = i64::from(px);
                let py = i64::from(py);
                let edge_distance = (px - i64::from(x0))
                    .min(i64::from(x1) - px)
                    .min(py - i64::from(y0))
                    .min(i64::from(y1) - py);
                if edge_distance < width {
                    canvas.put_rgba(px as u32, py as u32, [oc.0, oc.1, oc.2, oc.3]);
                }
            }
        }
    }
}

#[derive(Clone)]
struct QuarterState {
    cx: i32,
    cy: i32,
    ex: i32,
    ey: i32,
    a2: i128,
    b2: i128,
    a2b2: i128,
    finished: bool,
}

impl QuarterState {
    fn new(a: i32, b: i32) -> Self {
        if a < 0 || b < 0 {
            return Self {
                cx: 0,
                cy: 0,
                ex: 0,
                ey: 0,
                a2: 0,
                b2: 0,
                a2b2: 0,
                finished: true,
            };
        }
        let a2 = i128::from(a) * i128::from(a);
        let b2 = i128::from(b) * i128::from(b);
        Self {
            cx: a,
            cy: b % 2,
            ex: a % 2,
            ey: b,
            a2,
            b2,
            a2b2: a2 * b2,
            finished: false,
        }
    }

    fn delta(&self, x: i128, y: i128) -> i128 {
        (self.a2 * y * y + self.b2 * x * x - self.a2b2).abs()
    }

    fn next(&mut self) -> Option<(i32, i32)> {
        if self.finished {
            return None;
        }
        let point = (self.cx, self.cy);
        if self.cx == self.ex && self.cy == self.ey {
            self.finished = true;
            return Some(point);
        }

        let mut next_x = self.cx;
        let mut next_y = self.cy.saturating_add(2);
        let mut next_delta = self.delta(i128::from(next_x), i128::from(next_y));
        if next_x > 1 {
            let diagonal_delta = self.delta(
                i128::from(self.cx.saturating_sub(2)),
                i128::from(self.cy.saturating_add(2)),
            );
            if next_delta > diagonal_delta {
                next_x = self.cx.saturating_sub(2);
                next_y = self.cy.saturating_add(2);
                next_delta = diagonal_delta;
            }
            let horizontal_delta =
                self.delta(i128::from(self.cx.saturating_sub(2)), i128::from(self.cy));
            if next_delta > horizontal_delta {
                next_x = self.cx.saturating_sub(2);
                next_y = self.cy;
            }
        }
        self.cx = next_x;
        self.cy = next_y;
        Some(point)
    }
}

struct EllipseState {
    outer: QuarterState,
    inner: QuarterState,
    previous_y: i32,
    previous_left: i32,
    previous_right: i32,
    buffer: Vec<(i32, i32, i32)>,
    finished: bool,
    leftmost: i32,
}

impl EllipseState {
    fn new(a: i32, b: i32, width: i32) -> Self {
        let mut outer = QuarterState::new(a, b);
        let leftmost = a % 2;
        let first = if width < 1 { None } else { outer.next() };
        if let Some((previous_right, previous_y)) = first {
            let inset = width.saturating_sub(1).saturating_mul(2);
            Self {
                outer,
                inner: QuarterState::new(a.saturating_sub(inset), b.saturating_sub(inset)),
                previous_y,
                previous_left: leftmost,
                previous_right,
                buffer: Vec::with_capacity(4),
                finished: false,
                leftmost,
            }
        } else {
            Self {
                outer,
                inner: QuarterState::new(-1, -1),
                previous_y: 0,
                previous_left: 0,
                previous_right: 0,
                buffer: Vec::with_capacity(4),
                finished: true,
                leftmost,
            }
        }
    }

    fn next(&mut self) -> Option<(i32, i32, i32)> {
        if self.buffer.is_empty() {
            if self.finished {
                return None;
            }
            let y = self.previous_y;
            let mut left = self.previous_left;
            let right = self.previous_right;

            loop {
                match self.outer.next() {
                    Some((_, next_y)) if next_y <= y => {}
                    Some((x, next_y)) => {
                        self.previous_right = x;
                        self.previous_y = next_y;
                        break;
                    }
                    None => {
                        self.finished = true;
                        break;
                    }
                }
            }

            let mut next_inner = None;
            loop {
                match self.inner.next() {
                    Some((x, next_y)) if next_y <= y => left = x,
                    Some((x, _)) => {
                        next_inner = Some(x);
                        break;
                    }
                    None => break,
                }
            }
            self.previous_left = next_inner.unwrap_or(self.leftmost);

            let has_right_segment = left > 0 || left < right;
            if has_right_segment && y > 0 {
                self.buffer
                    .push((if left == 0 { 2 } else { left }, y, right));
            }
            if y > 0 {
                self.buffer.push((-right, y, -left));
            }
            if has_right_segment {
                self.buffer
                    .push((if left == 0 { 2 } else { left }, -y, right));
            }
            self.buffer.push((-right, -y, -left));
        }
        self.buffer.pop()
    }
}

fn draw_hline<C: DrawCanvas>(canvas: &mut C, x0: i32, y: i32, x1: i32, color: (u8, u8, u8, u8)) {
    let width = canvas.width();
    let height = canvas.height();
    if width == 0 || height == 0 || y < 0 || y >= height as i32 {
        return;
    }
    let left = x0.max(0);
    let right = x1.min(width as i32 - 1);
    if left > right {
        return;
    }
    for x in left..=right {
        canvas.put_rgba(x as u32, y as u32, [color.0, color.1, color.2, color.3]);
    }
}

fn draw_ellipse_segments<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    width: i32,
    color: (u8, u8, u8, u8),
) {
    let Some(a) = x1.checked_sub(x0) else {
        return;
    };
    let Some(b) = y1.checked_sub(y0) else {
        return;
    };
    if a < 0 || b < 0 {
        return;
    }
    let mut state = EllipseState::new(a, b, width);
    while let Some((segment_x0, segment_y, segment_x1)) = state.next() {
        let map_coordinate = |origin: i32, offset: i32, diameter: i32| {
            (i64::from(origin) + (i64::from(offset) + i64::from(diameter)) / 2)
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
        };
        draw_hline(
            canvas,
            map_coordinate(x0, segment_x0, a),
            map_coordinate(y0, segment_y, b),
            map_coordinate(x0, segment_x1, a),
            color,
        );
    }
}

/// Draw an ellipse using Pillow 12.2's `ellipse_state` scan conversion.
fn draw_ellipse_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
) {
    let Some(a) = x1.checked_sub(x0) else {
        return;
    };
    let Some(b) = y1.checked_sub(y0) else {
        return;
    };
    if let Some(fill_color) = fill {
        draw_ellipse_segments(canvas, x0, y0, x1, y1, a.saturating_add(b), fill_color);
    }
    if let Some(outline_color) = outline.filter(|color| Some(*color) != fill && width != 0) {
        draw_ellipse_segments(
            canvas,
            x0,
            y0,
            x1,
            y1,
            i32::try_from(width).unwrap_or(i32::MAX),
            outline_color,
        );
    }
}

struct BinaryMask {
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

struct MaskedCanvas<'a, C> {
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
fn draw_polygon_on_canvas<C: DrawCanvas>(
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

#[derive(Clone)]
enum ClipNode {
    Clip { a: f64, b: f64, c: f64 },
    And(Box<ClipNode>, Box<ClipNode>),
    Or(Box<ClipNode>, Box<ClipNode>),
}

impl ClipNode {
    fn transpose(&mut self) {
        match self {
            Self::Clip { a, b, .. } => std::mem::swap(a, b),
            Self::And(left, right) | Self::Or(left, right) => {
                left.transpose();
                right.transpose();
            }
        }
    }
}

fn union_intervals(mut intervals: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    intervals.sort_unstable();
    let mut merged: Vec<(i32, i32)> = Vec::with_capacity(intervals.len());
    for (left, right) in intervals {
        if let Some(last) = merged.last_mut()
            && left <= last.1.saturating_add(1)
        {
            last.1 = last.1.max(right);
        } else {
            merged.push((left, right));
        }
    }
    merged
}

fn intersect_intervals(left: &[(i32, i32)], right: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut intersections = Vec::new();
    for &(left_start, left_end) in left {
        for &(right_start, right_end) in right {
            let start = left_start.max(right_start);
            let end = left_end.min(right_end);
            if start <= end {
                intersections.push((start, end));
            }
        }
    }
    union_intervals(intersections)
}

fn clip_intervals(node: Option<&ClipNode>, mut x0: i32, y: i32, mut x1: i32) -> Vec<(i32, i32)> {
    let Some(node) = node else {
        return vec![(x0, x1)];
    };
    match node {
        ClipNode::Clip { a, b, c } => {
            const EPSILON: f64 = 1e-9;
            let y = f64::from(y);
            if a.abs() < EPSILON {
                if b * y + c < -EPSILON {
                    return Vec::new();
                }
            } else {
                let intersection_x = -(b * y + c) / a;
                if a * f64::from(x0) + b * y + c < EPSILON {
                    x0 = f64::from(x0).max(intersection_x).round() as i32;
                }
                if a * f64::from(x1) + b * y + c < EPSILON {
                    x1 = f64::from(x1).min(intersection_x).round() as i32;
                }
            }
            (x0 <= x1).then_some((x0, x1)).into_iter().collect()
        }
        ClipNode::And(left, right) => {
            let left = clip_intervals(Some(left), x0, y, x1);
            let right = clip_intervals(Some(right), x0, y, x1);
            intersect_intervals(&left, &right)
        }
        ClipNode::Or(left, right) => {
            let mut intervals = clip_intervals(Some(left), x0, y, x1);
            intervals.extend(clip_intervals(Some(right), x0, y, x1));
            union_intervals(intervals)
        }
    }
}

struct ClipEllipseState {
    ellipse: EllipseState,
    root: Option<ClipNode>,
    buffer: std::collections::VecDeque<(i32, i32, i32)>,
}

impl ClipEllipseState {
    fn new(ellipse: EllipseState, root: Option<ClipNode>) -> Self {
        Self {
            ellipse,
            root,
            buffer: std::collections::VecDeque::new(),
        }
    }

    fn next(&mut self) -> Option<(i32, i32, i32)> {
        while self.buffer.is_empty() {
            let (x0, y, x1) = self.ellipse.next()?;
            for (left, right) in clip_intervals(self.root.as_ref(), x0, y, x1) {
                self.buffer.push_back((left, y, right));
            }
        }
        self.buffer.pop_front()
    }
}

fn normalize_angles(mut start: f32, mut end: f32) -> (f32, f32) {
    if end - start >= 360.0 {
        return (0.0, 360.0);
    }
    start = if start < 0.0 {
        360.0 - ((-start) % 360.0)
    } else {
        start
    } % 360.0;
    end = start
        + if end < start {
            360.0 - ((start - end) % 360.0)
        } else {
            (end - start) % 360.0
        };
    (start, end)
}

fn clip(a: f64, b: f64, c: f64) -> ClipNode {
    ClipNode::Clip { a, b, c }
}

fn and(left: ClipNode, right: ClipNode) -> ClipNode {
    ClipNode::And(Box::new(left), Box::new(right))
}

fn or(left: ClipNode, right: ClipNode) -> ClipNode {
    ClipNode::Or(Box::new(left), Box::new(right))
}

fn arc_clip_state(a: i32, b: i32, width: i32, start: f32, end: f32) -> ClipEllipseState {
    if a < b {
        let mut state = arc_clip_state(b, a, width, 90.0 - end, 90.0 - start);
        state.ellipse = EllipseState::new(a, b, width);
        if let Some(root) = state.root.as_mut() {
            root.transpose();
        }
        return state;
    }

    let (start, end) = normalize_angles(start, end);
    let root = if end == start + 360.0 {
        None
    } else {
        let start_radians = f64::from(start).to_radians();
        let end_radians = f64::from(end).to_radians();
        let axis_delta = (i64::from(a) * i64::from(a) - i64::from(b) * i64::from(b)) as f64;
        let left = clip(
            -f64::from(a) * start_radians.sin(),
            f64::from(b) * start_radians.cos(),
            axis_delta * (f64::from(start) * std::f64::consts::PI / 90.0).sin() / 2.0,
        );
        let right = clip(
            f64::from(a) * end_radians.sin(),
            -f64::from(b) * end_radians.cos(),
            -axis_delta * (f64::from(end) * std::f64::consts::PI / 90.0).sin() / 2.0,
        );
        if start % 180.0 == 0.0 || end % 180.0 == 0.0 {
            Some(if end - start < 180.0 {
                and(left, right)
            } else {
                or(left, right)
            })
        } else if (((start / 180.0) as i32 + (end / 180.0) as i32) % 2) == 1 {
            let start_half_plane = clip(
                0.0,
                if ((start / 180.0) as i32) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                },
                0.0,
            );
            let end_half_plane = clip(
                0.0,
                if ((end / 180.0) as i32) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                },
                0.0,
            );
            Some(or(and(start_half_plane, left), and(end_half_plane, right)))
        } else {
            let combine = |left, right| {
                if end - start < 180.0 {
                    and(left, right)
                } else {
                    or(left, right)
                }
            };
            Some(combine(
                combine(left, right),
                clip(
                    0.0,
                    if end < 180.0 || end > 540.0 {
                        1.0
                    } else {
                        -1.0
                    },
                    0.0,
                ),
            ))
        }
    };
    ClipEllipseState::new(EllipseState::new(a, b, width), root)
}

fn chord_clip_state(a: i32, b: i32, width: i32, start: f32, end: f32) -> ClipEllipseState {
    let start = f64::from(start).to_radians();
    let end = f64::from(end).to_radians();
    let start_x = f64::from(a) * start.cos();
    let end_x = f64::from(a) * end.cos();
    let start_y = f64::from(b) * start.sin();
    let end_y = f64::from(b) * end.sin();
    let line_a = end_y - start_y;
    let line_b = start_x - end_x;
    let line_c = -(line_a * start_x + line_b * start_y);
    ClipEllipseState::new(
        EllipseState::new(a, b, width),
        Some(clip(line_a, line_b, line_c)),
    )
}

fn chord_line_clip_state(a: i32, b: i32, width: i32, start: f32, end: f32) -> ClipEllipseState {
    let start = f64::from(start).to_radians();
    let end = f64::from(end).to_radians();
    let start_x = f64::from(a) * start.cos();
    let end_x = f64::from(a) * end.cos();
    let start_y = f64::from(b) * start.sin();
    let end_y = f64::from(b) * end.sin();
    let line_a = end_y - start_y;
    let line_b = start_x - end_x;
    let line_c = -(line_a * start_x + line_b * start_y);
    let opposite_c = 2.0 * f64::from(width) * (line_a * line_a + line_b * line_b).sqrt() - line_c;
    ClipEllipseState::new(
        EllipseState::new(a, b, a.saturating_add(b).saturating_add(1)),
        Some(and(
            clip(line_a, line_b, line_c),
            clip(-line_a, -line_b, opposite_c),
        )),
    )
}

fn pie_side_clip_state(a: i32, b: i32, width: i32, angle: f32) -> ClipEllipseState {
    let angle = f64::from(angle).to_radians();
    let x = f64::from(a) * angle.cos();
    let y = f64::from(b) * angle.sin();
    let line_a = -y;
    let line_b = x;
    let line_c = f64::from(width) * (line_a * line_a + line_b * line_b).sqrt();
    ClipEllipseState::new(
        EllipseState::new(a, b, a.saturating_add(b).saturating_add(1)),
        Some(and(
            and(clip(line_a, line_b, line_c), clip(-line_a, -line_b, line_c)),
            clip(line_b, -line_a, 0.0),
        )),
    )
}

fn pie_clip_state(a: i32, b: i32, width: i32, start: f32, end: f32) -> ClipEllipseState {
    let start_radians = f64::from(start).to_radians();
    let end_radians = f64::from(end).to_radians();
    let start_x = f64::from(a) * start_radians.cos();
    let end_x = f64::from(a) * end_radians.cos();
    let start_y = f64::from(b) * start_radians.sin();
    let end_y = f64::from(b) * end_radians.sin();
    let left = clip(-start_y, start_x, 0.0);
    let right = clip(end_y, -end_x, 0.0);
    let mut root = if end - start < 180.0 {
        and(left, right)
    } else {
        or(left, right)
    };
    if end - start < 90.0 {
        root = and(
            root,
            clip((start_x + end_x) / 2.0, (start_y + end_y) / 2.0, 0.0),
        );
    }
    ClipEllipseState::new(EllipseState::new(a, b, width), Some(root))
}

fn draw_clipped_ellipse<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    a: i32,
    b: i32,
    mut state: ClipEllipseState,
    color: (u8, u8, u8, u8),
) {
    while let Some((segment_x0, segment_y, segment_x1)) = state.next() {
        let map_coordinate = |origin: i32, offset: i32, diameter: i32| {
            (i64::from(origin) + (i64::from(offset) + i64::from(diameter)) / 2)
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
        };
        draw_hline(
            canvas,
            map_coordinate(x0, segment_x0, a),
            map_coordinate(y0, segment_y, b),
            map_coordinate(x0, segment_x1, a),
            color,
        );
    }
}

fn draw_arc_on_canvas<C: DrawCanvas>(
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

fn draw_chord_on_canvas<C: DrawCanvas>(
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
fn draw_pieslice_on_canvas<C: DrawCanvas>(
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
    Ok(draw_native(img, |canvas| {
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
    Ok(draw_native(img, |canvas| {
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
    width: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
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

    Ok(draw_native(img, |canvas| {
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
        let corners = if full_x {
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
        };

        if let Some(fill_color) = fill {
            for &(left, top, right, bottom, start, end) in &corners {
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
            } else if x1 - radius - 1 >= x0 + radius + 1 {
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

        if let Some(outline_color) = outline.filter(|color| Some(*color) != fill && width != 0) {
            for &(left, top, right, bottom, start, end) in &corners {
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
    }))
}

pub fn op_draw_ellipse(
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
    Ok(draw_native(img, |canvas| {
        draw_ellipse_on_canvas(canvas, x0, y0, x1, y1, fill, outline, width);
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
    width: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let pts = points.to_vec();
    Ok(draw_native(img, |canvas| {
        draw_polygon_on_canvas(canvas, &pts, fill, outline, width);
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
    width: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let Some(fc) = fill else {
        return Ok(img.clone());
    };
    Ok(draw_native(img, |canvas| {
        draw_arc_on_canvas(canvas, x0, y0, x1, y1, start, end, fc, width);
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
    Ok(draw_native(img, |canvas| {
        draw_chord_on_canvas(canvas, x0, y0, x1, y1, start, end, fill, outline, width);
    }))
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
    width: u32,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    Ok(draw_native(img, |canvas| {
        draw_pieslice_on_canvas(canvas, x0, y0, x1, y1, start, end, fill, outline, width);
    }))
}

pub fn op_draw_point(
    img: &DynamicImage,
    points: &[(i32, i32)],
    fill: (u8, u8, u8, u8),
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let pts = points.to_vec();
    Ok(draw_native(img, |canvas| {
        let (img_w, img_h) = (canvas.width(), canvas.height());
        for &(x, y) in &pts {
            plot(canvas, x, y, fill, img_w, img_h, false);
        }
    }))
}
