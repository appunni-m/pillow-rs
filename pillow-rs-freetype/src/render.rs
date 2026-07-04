//! FreeType-style glyph render modes and bitmap helpers.
//!
//! Normal and LCD modes use the smooth rasterizer.  The vendored FreeType
//! configuration keeps `FT_CONFIG_OPTION_SUBPIXEL_RENDERING` disabled, so LCD
//! and LCD_V follow the Harmony renderer geometry from `ftsmooth.c`.

use crate::casts::{i32_from_usize, u32_from_usize, usize_from_i32, usize_from_i64};
use crate::error::FontError;
use crate::font::Font;
use crate::grays;
use crate::outline::Outline;
use crate::scaler;

const LCD_SUBPIXELS: [i32; 3] = [-21, 0, 21];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Normal,
    Mono,
    Lcd,
    LcdV,
}

impl RenderMode {
    pub fn fixture_name(self) -> &'static str {
        match self {
            RenderMode::Normal => "normal",
            RenderMode::Mono => "mono",
            RenderMode::Lcd => "lcd",
            RenderMode::LcdV => "lcd_v",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelMode {
    Gray,
    Mono,
    Lcd,
    LcdV,
}

impl PixelMode {
    pub fn fixture_name(self) -> &'static str {
        match self {
            PixelMode::Gray => "gray",
            PixelMode::Mono => "mono",
            PixelMode::Lcd => "lcd",
            PixelMode::LcdV => "lcd_v",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedBitmap {
    pub width: u32,
    pub rows: u32,
    pub pitch: i32,
    pub pixel_mode: PixelMode,
    pub left: i32,
    pub top: i32,
    pub buffer: Vec<u8>,
}

impl Font {
    /// Render the first character of `text` using a FreeType render mode.
    pub fn render_mode(&self, text: &str, mode: RenderMode) -> Result<RenderedBitmap, FontError> {
        let Some(ch) = text.chars().next() else {
            return Ok(RenderedBitmap {
                width: 0,
                rows: 0,
                pitch: 0,
                pixel_mode: PixelMode::Gray,
                left: 0,
                top: 0,
                buffer: Vec::new(),
            });
        };
        self.render_char_mode(ch, mode)
    }

    pub fn render_char_mode(
        &self,
        ch: char,
        mode: RenderMode,
    ) -> Result<RenderedBitmap, FontError> {
        let glyph = self.data.cmap.char_index(ch as u32).unwrap_or(0);
        let metrics_cache = self.face_globals.get_metrics(glyph);
        let scaled = if mode == RenderMode::Mono {
            scaler::scale_glyph_mono(&self.data, glyph, metrics_cache.as_ref(), self.is_italic)?
        } else {
            scaler::scale_glyph(&self.data, glyph, metrics_cache.as_ref(), self.is_italic)?
        };

        if scaled.outline.n_contours == 0 {
            return Ok(RenderedBitmap {
                width: 0,
                rows: 0,
                pitch: 0,
                pixel_mode: match mode {
                    RenderMode::Mono => PixelMode::Mono,
                    RenderMode::Lcd => PixelMode::Lcd,
                    RenderMode::LcdV => PixelMode::LcdV,
                    RenderMode::Normal => PixelMode::Gray,
                },
                left: 0,
                top: 0,
                buffer: Vec::new(),
            });
        }

        match mode {
            RenderMode::Normal => {
                render_normal(scaled.outline, scaled.bbox_x_min, scaled.bbox_y_max)
            }
            RenderMode::Mono => render_mono(scaled.outline, scaled.bbox_x_min, scaled.bbox_y_min),
            RenderMode::Lcd => render_lcd(scaled.outline, scaled.bbox_x_min, scaled.bbox_y_max),
            RenderMode::LcdV => render_lcd_v(scaled.outline, scaled.bbox_x_min, scaled.bbox_y_max),
        }
    }
}

fn render_normal(outline: Outline, left: i32, top: i32) -> Result<RenderedBitmap, FontError> {
    let raster = grays::rasterize(outline)?;
    Ok(RenderedBitmap {
        width: u32_from_usize(raster.width),
        rows: u32_from_usize(raster.height),
        pitch: i32_from_usize(raster.width),
        pixel_mode: PixelMode::Gray,
        left,
        top,
        buffer: raster.pixels,
    })
}

fn render_mono(
    mut outline: Outline,
    bbox_x_min: i32,
    bbox_y_min: i32,
) -> Result<RenderedBitmap, FontError> {
    let mono_box = mono_preset_box(&outline, bbox_x_min, bbox_y_min);
    let width = usize_from_i32(mono_box.x_max - mono_box.x_min);
    let height = usize_from_i32(mono_box.y_max - mono_box.y_min);
    translate_outline(
        &mut outline,
        (bbox_x_min - mono_box.x_min) * 64,
        (bbox_y_min - mono_box.y_min) * 64,
    );
    let buffer = rasterize_mono_center(&outline, width, height)?;
    let pitch = mono_pitch(width);
    Ok(RenderedBitmap {
        width: u32_from_usize(width),
        rows: u32_from_usize(height),
        pitch: i32_from_usize(pitch),
        pixel_mode: PixelMode::Mono,
        left: mono_box.x_min,
        top: mono_box.y_max,
        buffer,
    })
}

#[derive(Debug, Clone, Copy)]
struct PixelBox {
    x_min: i32,
    y_min: i32,
    x_max: i32,
    y_max: i32,
}

fn mono_preset_box(outline: &Outline, bbox_x_min: i32, bbox_y_min: i32) -> PixelBox {
    let mut x_min = outline.points[0].x;
    let mut y_min = outline.points[0].y;
    let mut x_max = outline.points[0].x;
    let mut y_max = outline.points[0].y;
    for point in &outline.points {
        x_min = x_min.min(point.x);
        y_min = y_min.min(point.y);
        x_max = x_max.max(point.x);
        y_max = y_max.max(point.y);
    }

    PixelBox {
        x_min: mono_round_min(bbox_x_min, x_min),
        y_min: mono_round_min(bbox_y_min, y_min),
        x_max: mono_round_max(bbox_x_min, x_max),
        y_max: mono_round_max(bbox_y_min, y_max),
    }
    .with_non_collapsed(x_min, y_min, x_max, y_max)
}

impl PixelBox {
    fn with_non_collapsed(mut self, x_min: i32, y_min: i32, x_max: i32, y_max: i32) -> Self {
        if self.x_min == self.x_max {
            if mono_collapse_bias(x_min, x_max) < 0 {
                self.x_min -= 1;
            } else {
                self.x_max += 1;
            }
        }
        if self.y_min == self.y_max {
            if mono_collapse_bias(y_min, y_max) < 0 {
                self.y_min -= 1;
            } else {
                self.y_max += 1;
            }
        }
        self
    }
}

fn mono_round_min(base: i32, value: i32) -> i32 {
    base + ((value + 31) >> 6)
}

fn mono_round_max(base: i32, value: i32) -> i32 {
    base + ((value + 32) >> 6)
}

fn mono_collapse_bias(min: i32, max: i32) -> i32 {
    (((min + 31) & 63) - 31) + (((max + 32) & 63) - 32)
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

fn rasterize_mono_center(
    outline: &Outline,
    width: usize,
    height: usize,
) -> Result<Vec<u8>, FontError> {
    let segments = flatten_outline(outline)?;
    let pitch = mono_pitch(width);
    let mut buffer = vec![0u8; pitch * height];
    for row in 0..height {
        let y = ((height - 1 - row) as i32) * 64 + 32;
        let dst_row = row * pitch;
        for x in 0..width {
            let px = (x as i32) * 64 + 32;
            if winding_contains(&segments, px, y) {
                buffer[dst_row + x / 8] |= 0x80 >> (x & 7);
            }
        }
    }
    Ok(buffer)
}

fn flatten_outline(outline: &Outline) -> Result<Vec<Segment>, FontError> {
    let mut flattener = MonoFlattener {
        segments: Vec::new(),
        current_x: 0,
        current_y: 0,
    };
    flattener.decompose(&outline.points, &outline.contours, outline.n_contours)?;
    Ok(flattener.segments)
}

struct MonoFlattener {
    segments: Vec<Segment>,
    current_x: i32,
    current_y: i32,
}

impl MonoFlattener {
    fn move_to(&mut self, x: i32, y: i32) {
        self.current_x = x;
        self.current_y = y;
    }

    fn line_to(&mut self, x: i32, y: i32) {
        if self.current_x != x || self.current_y != y {
            self.segments.push(Segment {
                x0: self.current_x,
                y0: self.current_y,
                x1: x,
                y1: y,
            });
        }
        self.current_x = x;
        self.current_y = y;
    }

    fn conic_to(&mut self, cx: i32, cy: i32, x: i32, y: i32) {
        let x0 = self.current_x;
        let y0 = self.current_y;
        let steps = conic_steps(x0, y0, cx, cy, x, y);
        for step in 1..=steps {
            let t = step as i64;
            let n = steps as i64;
            let mt = n - t;
            let nx = ((mt * mt * x0 as i64 + 2 * mt * t * cx as i64 + t * t * x as i64) / (n * n))
                as i32;
            let ny = ((mt * mt * y0 as i64 + 2 * mt * t * cy as i64 + t * t * y as i64) / (n * n))
                as i32;
            self.line_to(nx, ny);
        }
    }

    fn cubic_to(&mut self, c1x: i32, c1y: i32, c2x: i32, c2y: i32, x: i32, y: i32) {
        let x0 = self.current_x;
        let y0 = self.current_y;
        let steps = 16;
        for step in 1..=steps {
            let t = step as i64;
            let n = steps as i64;
            let mt = n - t;
            let nx = ((mt * mt * mt * x0 as i64
                + 3 * mt * mt * t * c1x as i64
                + 3 * mt * t * t * c2x as i64
                + t * t * t * x as i64)
                / (n * n * n)) as i32;
            let ny = ((mt * mt * mt * y0 as i64
                + 3 * mt * mt * t * c1y as i64
                + 3 * mt * t * t * c2y as i64
                + t * t * t * y as i64)
                / (n * n * n)) as i32;
            self.line_to(nx, ny);
        }
    }

    fn decompose(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        contours: &[i16],
        n_contours: i32,
    ) -> Result<(), FontError> {
        let mut last: i64 = -1;
        for &contour_end in contours.iter().take(usize_from_i32(n_contours)) {
            let first = usize_from_i64(last + 1);
            last = contour_end as i64;
            if last < first as i64 {
                return Err(FontError::InvalidOutline(
                    "outline: contour end before start".into(),
                ));
            }
            let limit = usize_from_i64(last);
            let mut v_start = pts[first];
            let v_last = pts[limit];
            let mut limit_eff = limit;

            let first_tag = curve_tag(pts[first].on_curve);
            if first_tag == CURVE_TAG_CUBIC {
                return Err(FontError::InvalidOutline(
                    "outline: contour starts with cubic".into(),
                ));
            }
            if first_tag == CURVE_TAG_CONIC {
                if curve_tag(pts[limit].on_curve) == CURVE_TAG_ON {
                    v_start = v_last;
                    limit_eff = limit.checked_sub(1).ok_or_else(|| {
                        FontError::InvalidOutline("outline: conic start underflow".into())
                    })?;
                } else {
                    v_start.x = (v_start.x + v_last.x) / 2;
                    v_start.y = (v_start.y + v_last.y) / 2;
                }
            }

            self.move_to(v_start.x, v_start.y);
            let start = if first_tag == CURVE_TAG_CONIC {
                if first == 0 {
                    -1
                } else {
                    i32_from_usize(first) - 1
                }
            } else {
                i32_from_usize(first)
            };
            self.walk_contour(pts, start, i32_from_usize(limit_eff), v_start)?;
        }
        Ok(())
    }

    fn walk_contour(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        mut cursor: i32,
        limit: i32,
        v_start: crate::outline::OutlinePoint,
    ) -> Result<(), FontError> {
        while cursor < limit {
            cursor += 1;
            let idx = usize_from_i32(cursor);
            match curve_tag(pts[idx].on_curve) {
                CURVE_TAG_ON => {
                    let point = pts[idx];
                    self.line_to(point.x, point.y);
                }
                CURVE_TAG_CONIC => {
                    let mut control = pts[idx];
                    loop {
                        if cursor < limit {
                            cursor += 1;
                            let next = pts[usize_from_i32(cursor)];
                            let tag = curve_tag(next.on_curve);
                            if tag == CURVE_TAG_ON {
                                self.conic_to(control.x, control.y, next.x, next.y);
                                break;
                            }
                            if tag != CURVE_TAG_CONIC {
                                return Err(FontError::InvalidOutline(
                                    "outline: expected conic tag".into(),
                                ));
                            }
                            let mid_x = (control.x + next.x) / 2;
                            let mid_y = (control.y + next.y) / 2;
                            self.conic_to(control.x, control.y, mid_x, mid_y);
                            control = next;
                            continue;
                        }
                        self.conic_to(control.x, control.y, v_start.x, v_start.y);
                        return Ok(());
                    }
                }
                CURVE_TAG_CUBIC => {
                    if cursor + 2 > limit
                        || curve_tag(pts[usize_from_i32(cursor + 1)].on_curve) != CURVE_TAG_CUBIC
                    {
                        return Err(FontError::InvalidOutline(
                            "outline: bad cubic tag sequence".into(),
                        ));
                    }
                    let control1 = pts[idx];
                    let control2 = pts[usize_from_i32(cursor + 1)];
                    cursor += 2;
                    if cursor <= limit {
                        let point = pts[usize_from_i32(cursor)];
                        self.cubic_to(
                            control1.x, control1.y, control2.x, control2.y, point.x, point.y,
                        );
                    } else {
                        self.cubic_to(
                            control1.x, control1.y, control2.x, control2.y, v_start.x, v_start.y,
                        );
                        return Ok(());
                    }
                }
                _ => unreachable!(),
            }
        }
        self.line_to(v_start.x, v_start.y);
        Ok(())
    }
}

const CURVE_TAG_ON: u8 = 1;
const CURVE_TAG_CONIC: u8 = 0;
const CURVE_TAG_CUBIC: u8 = 2;

fn curve_tag(on_curve: bool) -> u8 {
    if on_curve {
        CURVE_TAG_ON
    } else {
        CURVE_TAG_CONIC
    }
}

fn conic_steps(x0: i32, y0: i32, cx: i32, cy: i32, x1: i32, y1: i32) -> i32 {
    let dx = (x0 - 2 * cx + x1).abs();
    let dy = (y0 - 2 * cy + y1).abs();
    ((dx.max(dy) + 31) / 32).clamp(4, 24)
}

fn winding_contains(segments: &[Segment], x: i32, y: i32) -> bool {
    let mut winding = 0;
    for segment in segments {
        if segment.y0 <= y {
            if segment.y1 > y && is_left(*segment, x, y) > 0 {
                winding += 1;
            }
        } else if segment.y1 <= y && is_left(*segment, x, y) < 0 {
            winding -= 1;
        }
    }
    winding != 0
}

fn is_left(segment: Segment, x: i32, y: i32) -> i64 {
    (segment.x1 - segment.x0) as i64 * (y - segment.y0) as i64
        - (x - segment.x0) as i64 * (segment.y1 - segment.y0) as i64
}

fn render_lcd(outline: Outline, left: i32, top: i32) -> Result<RenderedBitmap, FontError> {
    let width = usize_from_i32(outline.cbox_x_max - outline.cbox_x_min + 2);
    let height = usize_from_i32(outline.cbox_y_max - outline.cbox_y_min);
    let row_width = width * 3;
    let pitch = pad_ceil(row_width, 4);
    let mut buffer = vec![0u8; pitch * height];
    for (channel, sub_x) in LCD_SUBPIXELS.iter().enumerate() {
        let mut shifted = outline.clone();
        translate_outline(&mut shifted, 64 - *sub_x, 0);
        let raster = grays::rasterize_in_box(shifted, width, height)?;
        for y in 0..height {
            let src = y * width;
            let dst = y * pitch + channel;
            for x in 0..width {
                buffer[dst + x * 3] = raster.pixels[src + x];
            }
        }
    }
    Ok(RenderedBitmap {
        width: u32_from_usize(row_width),
        rows: u32_from_usize(height),
        pitch: i32_from_usize(pitch),
        pixel_mode: PixelMode::Lcd,
        left: left - 1,
        top,
        buffer,
    })
}

fn render_lcd_v(outline: Outline, left: i32, top: i32) -> Result<RenderedBitmap, FontError> {
    let width = usize_from_i32(outline.cbox_x_max - outline.cbox_x_min);
    let height = usize_from_i32(outline.cbox_y_max - outline.cbox_y_min + 2);
    let rows = height * 3;
    let pitch = width;
    let mut buffer = vec![0u8; pitch * rows];
    for (channel, sub_x) in LCD_SUBPIXELS.iter().enumerate() {
        let mut shifted = outline.clone();
        translate_outline(&mut shifted, 0, 64 + *sub_x);
        let raster = grays::rasterize_in_box(shifted, width, height)?;
        for y in 0..height {
            let src = y * width;
            let dst = (y * 3 + channel) * pitch;
            buffer[dst..dst + width].copy_from_slice(&raster.pixels[src..src + width]);
        }
    }
    Ok(RenderedBitmap {
        width: u32_from_usize(width),
        rows: u32_from_usize(rows),
        pitch: i32_from_usize(pitch),
        pixel_mode: PixelMode::LcdV,
        left,
        top: top + 1,
        buffer,
    })
}

pub fn mono_pitch(width: usize) -> usize {
    ((width + 15) >> 4) << 1
}

pub fn pad_ceil(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

pub fn unpack_mono_row(row: &[u8], width: usize) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width);
    for x in 0..width {
        let byte = row.get(x / 8).copied().unwrap_or(0);
        pixels.push(if (byte & (0x80 >> (x & 7))) != 0 {
            255
        } else {
            0
        });
    }
    pixels
}

fn translate_outline(outline: &mut Outline, dx: i32, dy: i32) {
    for point in &mut outline.points {
        point.x += dx;
        point.y += dy;
    }
}
