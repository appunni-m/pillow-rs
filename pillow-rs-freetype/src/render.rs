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
            if mode == RenderMode::Mono {
                return Ok(RenderedBitmap {
                    width: 1,
                    rows: 1,
                    pitch: 2,
                    pixel_mode: PixelMode::Mono,
                    left: 0,
                    top: 1,
                    buffer: vec![0, 0],
                });
            }
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
    let mut buffer = rasterize_mono_center(&outline, width, height)?;
    let pitch = mono_pitch(width);
    let mut rows = height;
    let mut top = mono_box.y_max;
    if should_collapse_mono_two_row_stroke(&outline, &buffer, pitch, height) {
        buffer.truncate(pitch);
        rows = 1;
        top -= 1;
    }
    Ok(RenderedBitmap {
        width: u32_from_usize(width),
        rows: u32_from_usize(rows),
        pitch: i32_from_usize(pitch),
        pixel_mode: PixelMode::Mono,
        left: mono_box.x_min,
        top,
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

fn should_collapse_mono_two_row_stroke(
    outline: &Outline,
    buffer: &[u8],
    pitch: usize,
    height: usize,
) -> bool {
    if height != 2 || buffer.len() != pitch * 2 {
        return false;
    }
    let Some((y_min, y_max)) = outline_y_range(outline) else {
        return false;
    };
    y_max - y_min < 128 && buffer[..pitch] == buffer[pitch..pitch * 2]
}

fn outline_y_range(outline: &Outline) -> Option<(i32, i32)> {
    let first = outline.points.first()?;
    let mut y_min = first.y;
    let mut y_max = first.y;
    for point in &outline.points {
        y_min = y_min.min(point.y);
        y_max = y_max.max(point.y);
    }
    Some((y_min, y_max))
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
    contour: usize,
    order: usize,
    contour_len: usize,
}

fn rasterize_mono_center(
    outline: &Outline,
    width: usize,
    height: usize,
) -> Result<Vec<u8>, FontError> {
    let segments = flatten_outline(outline)?;
    let pitch = mono_pitch(width);
    let mut buffer = vec![0u8; pitch * height];
    rasterize_mono_profiles(&segments, &mut buffer, width, height, pitch);
    rasterize_mono_horizontal_profiles(&segments, &mut buffer, width, height, pitch);
    Ok(buffer)
}

fn rasterize_mono_intersections(
    segments: &[Segment],
    width: usize,
    height: usize,
    pitch: usize,
) -> Vec<u8> {
    let mut buffer = vec![0u8; pitch * height];
    for y in 0..height {
        let scan_y = (y as i32) * 64 + 32;
        let mut left = Vec::new();
        let mut right = Vec::new();
        for segment in segments {
            if let Some(intersection) = segment_intersection(*segment, scan_y) {
                if intersection.flow_up {
                    left.push(intersection);
                } else {
                    right.push(intersection);
                }
            }
        }
        left.sort_by_key(|intersection| intersection.x);
        right.sort_by_key(|intersection| intersection.x);

        let row = height - 1 - y;
        let dst_row = row * pitch;
        for (left, right) in left.iter().zip(&right) {
            let mut x1 = left.x;
            let mut x2 = right.x;
            if x1 > x2 {
                std::mem::swap(&mut x1, &mut x2);
            }
            let e1 = pixel_ceiling(x1);
            let e2 = pixel_floor(x2);
            if e1 <= e2 {
                fill_mono_span(&mut buffer[dst_row..dst_row + pitch], width, e1, e2);
            } else {
                set_mono_dropout(
                    &mut buffer[dst_row..dst_row + pitch],
                    width,
                    left,
                    right,
                    x1,
                    x2,
                );
            }
        }
    }
    buffer
}

const MONO_FLOW_UP: u8 = 0x08;
const MONO_OVERSHOOT_TOP: u8 = 0x10;
const MONO_OVERSHOOT_BOTTOM: u8 = 0x20;
const MONO_DROPOUT: u8 = 0x40;
const MONO_DROPOUT_CONTROL: u8 = 1;

#[derive(Debug, Clone)]
struct MonoProfile {
    xs: Vec<i32>,
    start: i32,
    offset: usize,
    height: usize,
    flags: u8,
    x: i32,
    link: Option<usize>,
    next: Option<usize>,
    contour: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonoState {
    Unknown,
    Ascending,
    Descending,
}

struct MonoProfileBuilder<'a> {
    segments: &'a [Segment],
    profiles: Vec<MonoProfile>,
    current: Option<usize>,
    contour_first: Option<usize>,
    contour_profiles: Vec<usize>,
    state: MonoState,
    last_x: i32,
    last_y: i32,
    min_y: i32,
    max_y: i32,
}

fn rasterize_mono_profiles(
    segments: &[Segment],
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
) {
    if width == 0 || height == 0 || segments.is_empty() {
        return;
    }

    let profiles = MonoProfileBuilder::new(segments, 0, i32_from_usize(height) * 64).build();
    if profiles.is_empty() {
        return;
    }

    draw_mono_profile_sweep(profiles, buffer, width, height, pitch);
}

fn rasterize_mono_horizontal_profiles(
    segments: &[Segment],
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
) {
    if width == 0 || height == 0 || segments.is_empty() {
        return;
    }

    let flipped: Vec<Segment> = segments
        .iter()
        .map(|segment| Segment {
            x0: segment.y0,
            y0: segment.x0,
            x1: segment.y1,
            y1: segment.x1,
            contour: segment.contour,
            order: segment.order,
            contour_len: segment.contour_len,
        })
        .collect();
    let profiles = MonoProfileBuilder::new(&flipped, 0, i32_from_usize(width) * 64).build();
    if profiles.is_empty() {
        return;
    }

    draw_mono_horizontal_profile_sweep(profiles, buffer, width, height, pitch);
}

impl<'a> MonoProfileBuilder<'a> {
    fn new(segments: &'a [Segment], min_y: i32, max_y: i32) -> Self {
        Self {
            segments,
            profiles: Vec::new(),
            current: None,
            contour_first: None,
            contour_profiles: Vec::new(),
            state: MonoState::Unknown,
            last_x: 0,
            last_y: 0,
            min_y,
            max_y,
        }
    }

    fn build(mut self) -> Vec<MonoProfile> {
        let mut cursor = 0;
        while cursor < self.segments.len() {
            let contour = self.segments[cursor].contour;
            self.state = MonoState::Unknown;
            self.current = None;
            self.contour_first = None;
            self.contour_profiles.clear();

            self.last_x = scaled_mono_coord(self.segments[cursor].x0);
            self.last_y = scaled_mono_coord(self.segments[cursor].y0);
            while cursor < self.segments.len() && self.segments[cursor].contour == contour {
                let segment = self.segments[cursor];
                self.line_to(
                    scaled_mono_coord(segment.x1),
                    scaled_mono_coord(segment.y1),
                    contour,
                );
                cursor += 1;
            }

            if self.contour_first.is_some() {
                if self.last_y & 63 == 0 && self.last_y >= self.min_y && self.last_y <= self.max_y {
                    if let (Some(first), Some(current)) = (self.contour_first, self.current) {
                        if same_profile_flow(&self.profiles[first], &self.profiles[current]) {
                            self.profiles[current].xs.pop();
                        }
                    }
                }
                self.end_profile();
                self.link_contour_profiles();
            }
        }
        self.profiles
    }

    fn line_to(&mut self, x: i32, y: i32, contour: usize) {
        if y == self.last_y {
            self.last_x = x;
            self.last_y = y;
            return;
        }

        let state = if self.last_y < y {
            MonoState::Ascending
        } else {
            MonoState::Descending
        };
        if self.state != state {
            if self.state != MonoState::Unknown {
                self.end_profile();
            }
            self.new_profile(state, contour);
        }

        if state == MonoState::Ascending {
            let xs = line_up(self.last_x, self.last_y, x, y, self.min_y, self.max_y);
            self.push_profile_xs(xs);
        } else {
            let xs = line_down(self.last_x, self.last_y, x, y, self.min_y, self.max_y);
            self.push_profile_xs(xs);
        }

        self.last_x = x;
        self.last_y = y;
    }

    fn new_profile(&mut self, state: MonoState, contour: usize) {
        let mut flags = MONO_DROPOUT_CONTROL;
        let e = match state {
            MonoState::Ascending => {
                flags |= MONO_FLOW_UP;
                if is_bottom_overshoot(self.last_y) {
                    flags |= MONO_OVERSHOOT_BOTTOM;
                }
                mono_ceiling_fixed(self.last_y)
            }
            MonoState::Descending => {
                if is_top_overshoot(self.last_y) {
                    flags |= MONO_OVERSHOOT_TOP;
                }
                mono_floor_fixed(self.last_y)
            }
            MonoState::Unknown => unreachable!(),
        }
        .clamp(self.min_y, self.max_y);

        let mut xs = Vec::new();
        if self.last_y == e {
            xs.push(self.last_x);
        }
        let index = self.profiles.len();
        self.profiles.push(MonoProfile {
            xs,
            start: e >> 6,
            offset: 0,
            height: 0,
            flags,
            x: self.last_x,
            link: None,
            next: self.contour_first,
            contour,
        });
        if self.contour_first.is_none() {
            self.contour_first = Some(index);
        }
        self.current = Some(index);
        self.state = state;
        self.contour_profiles.push(index);
    }

    fn end_profile(&mut self) {
        let Some(index) = self.current else {
            return;
        };
        let height = self.profiles[index].xs.len();
        if height == 0 {
            return;
        }

        if self.profiles[index].flags & MONO_FLOW_UP != 0 {
            if is_top_overshoot(self.last_y) {
                self.profiles[index].flags |= MONO_OVERSHOOT_TOP;
            }
            self.profiles[index].offset = 0;
            self.profiles[index].x = self.profiles[index].xs[0];
        } else {
            if is_bottom_overshoot(self.last_y) {
                self.profiles[index].flags |= MONO_OVERSHOOT_BOTTOM;
            }
            let top = self.profiles[index].start + 1;
            self.profiles[index].start = top - i32_from_usize(height);
            self.profiles[index].offset = height - 1;
            self.profiles[index].x = self.profiles[index].xs[height - 1];
        }
        self.profiles[index].height = height;
    }

    fn link_contour_profiles(&mut self) {
        let len = self.contour_profiles.len();
        if len == 0 {
            return;
        }
        for idx in 0..len {
            let profile = self.contour_profiles[idx];
            let next = self.contour_profiles[(idx + 1) % len];
            self.profiles[profile].next = Some(next);
        }
    }

    fn push_profile_xs(&mut self, xs: Vec<i32>) {
        if let Some(index) = self.current {
            self.profiles[index].xs.extend(xs);
        }
    }
}

fn draw_mono_profile_sweep(
    mut profiles: Vec<MonoProfile>,
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
) {
    let mut waiting: Vec<usize> = (0..profiles.len())
        .filter(|&index| profiles[index].height > 0)
        .collect();
    waiting.sort_by_key(|&index| profiles[index].start);
    let Some(min_y) = waiting.first().map(|&index| profiles[index].start) else {
        return;
    };
    let max_y = profiles
        .iter()
        .filter(|profile| profile.height > 0)
        .map(|profile| profile.start + i32_from_usize(profile.height) - 1)
        .max()
        .unwrap_or(min_y);

    let mut draw_left: Vec<usize> = Vec::new();
    let mut draw_right: Vec<usize> = Vec::new();
    for y in min_y..=max_y {
        let mut index = 0;
        while index < waiting.len() {
            let profile = waiting[index];
            if profiles[profile].start == y {
                let profile = waiting.remove(index);
                if profiles[profile].flags & MONO_FLOW_UP != 0 {
                    insert_profile_sorted(&mut draw_left, profile, &profiles);
                } else {
                    insert_profile_sorted(&mut draw_right, profile, &profiles);
                }
            } else {
                index += 1;
            }
        }

        if y >= 0 && y < i32_from_usize(height) {
            let row = height - 1 - usize_from_i32(y);
            let row = &mut buffer[row * pitch..(row + 1) * pitch];
            let pair_count = draw_left.len().min(draw_right.len());
            let mut dropouts = Vec::new();
            for pair in 0..pair_count {
                let left = draw_left[pair];
                let right = draw_right[pair];
                let mut x1 = profiles[left].x;
                let mut x2 = profiles[right].x;
                if x1 > x2 {
                    std::mem::swap(&mut x1, &mut x2);
                }
                if mono_ceiling_fixed(x1) <= mono_floor_fixed(x2) {
                    fill_mono_span(row, width, x1_to_pixel_ceil(x1), x2_to_pixel_floor(x2));
                } else if should_draw_profile_dropout(&profiles, left, right, x1, x2) {
                    let drop = profile_dropout_pixels(x1, x2);
                    profiles[left].x = drop.primary;
                    profiles[right].x = drop.secondary;
                    profiles[left].flags |= MONO_DROPOUT;
                    dropouts.push((left, right));
                }
            }

            for (left, right) in dropouts {
                if profiles[left].flags & MONO_DROPOUT != 0 {
                    set_mono_dropout_pixels(row, width, profiles[left].x, profiles[right].x);
                    profiles[left].flags &= !MONO_DROPOUT;
                }
            }
        }

        increment_profiles(&mut draw_left, &mut profiles, 1);
        increment_profiles(&mut draw_right, &mut profiles, -1);
    }
}

fn draw_mono_horizontal_profile_sweep(
    mut profiles: Vec<MonoProfile>,
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
) {
    let mut waiting: Vec<usize> = (0..profiles.len())
        .filter(|&index| profiles[index].height > 0)
        .collect();
    waiting.sort_by_key(|&index| profiles[index].start);
    let Some(min_y) = waiting.first().map(|&index| profiles[index].start) else {
        return;
    };
    let max_y = profiles
        .iter()
        .filter(|profile| profile.height > 0)
        .map(|profile| profile.start + i32_from_usize(profile.height) - 1)
        .max()
        .unwrap_or(min_y);

    let mut draw_left: Vec<usize> = Vec::new();
    let mut draw_right: Vec<usize> = Vec::new();
    for y in min_y..=max_y {
        let mut index = 0;
        while index < waiting.len() {
            let profile = waiting[index];
            if profiles[profile].start == y {
                let profile = waiting.remove(index);
                if profiles[profile].flags & MONO_FLOW_UP != 0 {
                    insert_profile_sorted(&mut draw_left, profile, &profiles);
                } else {
                    insert_profile_sorted(&mut draw_right, profile, &profiles);
                }
            } else {
                index += 1;
            }
        }

        if y >= 0 && y < i32_from_usize(width) {
            let pair_count = draw_left.len().min(draw_right.len());
            let mut dropouts = Vec::new();
            for pair in 0..pair_count {
                let left = draw_left[pair];
                let right = draw_right[pair];
                let mut x1 = profiles[left].x;
                let mut x2 = profiles[right].x;
                if x1 > x2 {
                    std::mem::swap(&mut x1, &mut x2);
                }
                if mono_ceiling_fixed(x1) <= mono_floor_fixed(x2) {
                    set_mono_horizontal_span_edges(buffer, width, height, pitch, y, x1, x2);
                } else if should_draw_profile_dropout(&profiles, left, right, x1, x2) {
                    let drop = profile_dropout_pixels(x1, x2);
                    profiles[left].x = drop.primary;
                    profiles[right].x = drop.secondary;
                    profiles[left].flags |= MONO_DROPOUT;
                    dropouts.push((left, right));
                }
            }

            for (left, right) in dropouts {
                if profiles[left].flags & MONO_DROPOUT != 0 {
                    set_mono_horizontal_dropout_pixel(
                        buffer,
                        width,
                        height,
                        pitch,
                        y,
                        profiles[left].x,
                        profiles[right].x,
                    );
                    profiles[left].flags &= !MONO_DROPOUT;
                }
            }
        }

        increment_profiles(&mut draw_left, &mut profiles, 1);
        increment_profiles(&mut draw_right, &mut profiles, -1);
    }
}

fn set_mono_horizontal_span_edges(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    x: i32,
    y1: i32,
    y2: i32,
) {
    if y1 == mono_ceiling_fixed(y1) {
        set_mono_horizontal_pixel(buffer, width, height, pitch, x, y1 >> 6);
    }
    if y2 == mono_floor_fixed(y2) {
        set_mono_horizontal_pixel(buffer, width, height, pitch, x, y2 >> 6);
    }
}

fn set_mono_horizontal_dropout_pixel(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    x: i32,
    mut primary: i32,
    secondary: i32,
) {
    if primary < 0 || primary >= i32_from_usize(height) {
        primary = secondary;
    } else if secondary >= 0
        && secondary < i32_from_usize(height)
        && mono_horizontal_pixel_is_set(buffer, width, height, pitch, x, secondary)
    {
        return;
    }

    set_mono_horizontal_pixel(buffer, width, height, pitch, x, primary);
}

fn mono_horizontal_pixel_is_set(
    buffer: &[u8],
    width: usize,
    height: usize,
    pitch: usize,
    x: i32,
    y: i32,
) -> bool {
    let Some((row, col)) = mono_pixel_offset(width, height, pitch, x, y) else {
        return false;
    };
    buffer[row + col / 8] & (0x80 >> (col & 7)) != 0
}

fn set_mono_horizontal_pixel(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    x: i32,
    y: i32,
) {
    let Some((row, col)) = mono_pixel_offset(width, height, pitch, x, y) else {
        return;
    };
    buffer[row + col / 8] |= 0x80 >> (col & 7);
}

fn mono_pixel_offset(
    width: usize,
    height: usize,
    pitch: usize,
    x: i32,
    y: i32,
) -> Option<(usize, usize)> {
    if x < 0 || y < 0 || x >= i32_from_usize(width) || y >= i32_from_usize(height) {
        return None;
    }
    let row = height - 1 - usize_from_i32(y);
    Some((row * pitch, usize_from_i32(x)))
}

fn line_up(x1: i32, y1: i32, x2: i32, y2: i32, min_y: i32, max_y: i32) -> Vec<i32> {
    if y2 < min_y || y1 > max_y {
        return Vec::new();
    }
    let e2 = if y2 > max_y {
        max_y
    } else {
        mono_floor_fixed(y2)
    };
    let mut e = if y1 < min_y {
        min_y
    } else {
        mono_ceiling_fixed(y1)
    };
    if y1 == e {
        e += 64;
    }
    if e2 < e {
        return Vec::new();
    }

    let mut size = ((e2 - e) >> 6) + 1;
    let dx = x2 - x1;
    let dy = y2 - y1;
    let mut out = Vec::with_capacity(usize_from_i32(size));
    if dx == 0 {
        out.resize(usize_from_i32(size), x1);
        return out;
    }

    let ix = mul_div_trunc(e - y1, dx, dy);
    let mut x = x1 + ix;
    out.push(x);
    size -= 1;
    if size > 0 {
        let mut ax = dx * (e - y1) - dy * ix;
        let ix = mul_div_trunc(64, dx, dy);
        let mut rx = dx * 64 - dy * ix;
        let mut step = 1;
        if x2 < x {
            ax = -ax;
            rx = -rx;
            step = -1;
        }
        while size > 0 {
            x += ix;
            ax += rx;
            if ax >= dy {
                ax -= dy;
                x += step;
            }
            out.push(x);
            size -= 1;
        }
    }
    out
}

fn line_down(x1: i32, y1: i32, x2: i32, y2: i32, min_y: i32, max_y: i32) -> Vec<i32> {
    line_up(x1, -y1, x2, -y2, -max_y, -min_y)
}

fn insert_profile_sorted(list: &mut Vec<usize>, profile: usize, profiles: &[MonoProfile]) {
    let x = profiles[profile].x;
    let pos = list
        .iter()
        .position(|&current| profiles[current].x >= x)
        .unwrap_or(list.len());
    list.insert(pos, profile);
}

fn increment_profiles(list: &mut Vec<usize>, profiles: &mut [MonoProfile], flow: isize) {
    let mut index = 0;
    while index < list.len() {
        let profile = list[index];
        profiles[profile].height -= 1;
        if profiles[profile].height == 0 {
            list.remove(index);
            continue;
        }
        if flow > 0 {
            profiles[profile].offset += 1;
        } else {
            profiles[profile].offset -= 1;
        }
        profiles[profile].x = profiles[profile].xs[profiles[profile].offset];
        index += 1;
    }
    list.sort_by_key(|&profile| profiles[profile].x);
}

fn should_draw_profile_dropout(
    profiles: &[MonoProfile],
    left: usize,
    right: usize,
    x1: i32,
    x2: i32,
) -> bool {
    let control = profiles[left].flags & 7;
    if control & 2 != 0 {
        return false;
    }
    if control & 1 == 0 {
        return true;
    }
    if profiles[left].contour == profiles[right].contour {
        if profiles[left].height == 1
            && profiles[left].next == Some(right)
            && (profiles[left].flags & MONO_OVERSHOOT_TOP == 0 || x2 - x1 < 32)
        {
            return false;
        }
        if profiles[left].offset == 0
            && profiles[right].next == Some(left)
            && (profiles[left].flags & MONO_OVERSHOOT_BOTTOM == 0 || x2 - x1 < 32)
        {
            return false;
        }
    }
    true
}

struct DropoutPixels {
    primary: i32,
    secondary: i32,
}

fn profile_dropout_pixels(x1: i32, x2: i32) -> DropoutPixels {
    DropoutPixels {
        primary: x2_to_pixel_floor(x2),
        secondary: x1_to_pixel_ceil(x1),
    }
}

fn set_mono_dropout_pixels(row: &mut [u8], width: usize, mut primary: i32, secondary: i32) {
    if width == 0 {
        return;
    }
    if primary < 0 || primary >= i32_from_usize(width) {
        primary = secondary;
    } else if secondary >= 0 && secondary < i32_from_usize(width) {
        let secondary = usize_from_i32(secondary);
        if row[secondary / 8] & (0x80 >> (secondary & 7)) != 0 {
            return;
        }
    }

    if primary >= 0 && primary < i32_from_usize(width) {
        let x = usize_from_i32(primary);
        row[x / 8] |= 0x80 >> (x & 7);
    }
}

fn same_profile_flow(left: &MonoProfile, right: &MonoProfile) -> bool {
    (left.flags & MONO_FLOW_UP) == (right.flags & MONO_FLOW_UP)
}

fn scaled_mono_coord(value: i32) -> i32 {
    value - 32
}

fn mono_floor_fixed(value: i32) -> i32 {
    value & !63
}

fn mono_ceiling_fixed(value: i32) -> i32 {
    (value + 63) & !63
}

fn x1_to_pixel_ceil(value: i32) -> i32 {
    mono_ceiling_fixed(value) >> 6
}

fn x2_to_pixel_floor(value: i32) -> i32 {
    mono_floor_fixed(value) >> 6
}

fn is_bottom_overshoot(value: i32) -> bool {
    mono_ceiling_fixed(value) - value >= 32
}

fn is_top_overshoot(value: i32) -> bool {
    value - mono_floor_fixed(value) >= 32
}

fn mul_div_trunc(a: i32, b: i32, c: i32) -> i32 {
    ((a as i64 * b as i64) / c as i64) as i32
}

#[derive(Debug, Clone, Copy)]
struct Intersection {
    x: i32,
    flow_up: bool,
    contour: usize,
    order: usize,
    contour_len: usize,
}

fn segment_intersection(segment: Segment, scan_y: i32) -> Option<Intersection> {
    let flow_up;
    if segment.y0 < segment.y1 {
        if scan_y < segment.y0 || scan_y >= segment.y1 {
            return None;
        }
        flow_up = true;
    } else if segment.y1 < segment.y0 {
        if scan_y <= segment.y1 || scan_y > segment.y0 {
            return None;
        }
        flow_up = false;
    } else {
        return None;
    }

    let dx = segment.x1 - segment.x0;
    let dy = segment.y1 - segment.y0;
    let x = segment.x0 - 32 + ((scan_y - segment.y0) as i64 * dx as i64 / dy as i64) as i32;
    Some(Intersection {
        x,
        flow_up,
        contour: segment.contour,
        order: segment.order,
        contour_len: segment.contour_len,
    })
}

fn pixel_ceiling(x: i32) -> i32 {
    (x + 63) >> 6
}

fn pixel_floor(x: i32) -> i32 {
    x >> 6
}

fn fill_mono_span(row: &mut [u8], width: usize, mut x1: i32, mut x2: i32) {
    if width == 0 {
        return;
    }
    x1 = x1.max(0);
    x2 = x2.min(i32_from_usize(width - 1));
    if x1 > x2 {
        return;
    }

    for x in usize_from_i32(x1)..=usize_from_i32(x2) {
        row[x / 8] |= 0x80 >> (x & 7);
    }
}

fn set_mono_dropout(
    row: &mut [u8],
    width: usize,
    left: &Intersection,
    right: &Intersection,
    x1: i32,
    x2: i32,
) {
    if width == 0 {
        return;
    }
    if left.contour == right.contour && x2 - x1 < 32 && adjacent_in_contour(left, right) {
        return;
    }

    let mut primary = pixel_floor(x2);
    let secondary = pixel_ceiling(x1);
    if primary < 0 || primary >= i32_from_usize(width) {
        primary = secondary;
    } else if secondary >= 0 && secondary < i32_from_usize(width) {
        let secondary = usize_from_i32(secondary);
        if row[secondary / 8] & (0x80 >> (secondary & 7)) != 0 {
            return;
        }
    }

    if primary >= 0 && primary < i32_from_usize(width) {
        let x = usize_from_i32(primary);
        row[x / 8] |= 0x80 >> (x & 7);
    }
}

fn adjacent_in_contour(left: &Intersection, right: &Intersection) -> bool {
    if left.order.abs_diff(right.order) == 1 {
        return true;
    }
    left.contour_len > 1
        && left.order.min(right.order) == 0
        && left.order.max(right.order) == left.contour_len - 1
}

fn apply_horizontal_center_edges(
    segments: &[Segment],
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
) {
    for segment in segments {
        if segment.y0 != segment.y1 || (segment.y0 & 63) != 32 {
            continue;
        }

        let y = (segment.y0 - 32) >> 6;
        if y < 0 || y >= i32_from_usize(height) {
            continue;
        }

        let row = height - 1 - usize_from_i32(y);
        let row = &mut buffer[row * pitch..(row + 1) * pitch];
        let x1 = pixel_ceiling(segment.x0.min(segment.x1) - 32);
        let x2 = pixel_floor(segment.x0.max(segment.x1) - 32);
        fill_mono_span(row, width, x1, x2);
    }
}

fn flatten_outline(outline: &Outline) -> Result<Vec<Segment>, FontError> {
    let mut flattener = MonoFlattener {
        segments: Vec::new(),
        current_x: 0,
        current_y: 0,
        contour: 0,
        order: 0,
    };
    flattener.decompose(&outline.points, &outline.contours, outline.n_contours)?;
    Ok(flattener.segments)
}

struct MonoFlattener {
    segments: Vec<Segment>,
    current_x: i32,
    current_y: i32,
    contour: usize,
    order: usize,
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
                contour: self.contour,
                order: self.order,
                contour_len: 0,
            });
            self.order += 1;
        }
        self.current_x = x;
        self.current_y = y;
    }

    fn conic_to(&mut self, cx: i32, cy: i32, x: i32, y: i32) {
        let x0 = self.current_x;
        let y0 = self.current_y;
        self.flatten_conic(
            [
                Point { x: x0, y: y0 },
                Point { x: cx, y: cy },
                Point { x, y },
            ],
            0,
        );
    }

    fn cubic_to(&mut self, c1x: i32, c1y: i32, c2x: i32, c2y: i32, x: i32, y: i32) {
        let x0 = self.current_x;
        let y0 = self.current_y;
        self.flatten_cubic(
            [
                Point { x: x0, y: y0 },
                Point { x: c1x, y: c1y },
                Point { x: c2x, y: c2y },
                Point { x, y },
            ],
            0,
        );
    }

    fn flatten_conic(&mut self, points: [Point; 3], depth: u8) {
        let y_min = points[0].y.min(points[2].y);
        let y_max = points[0].y.max(points[2].y);
        let monotonic = points[1].y >= y_min && points[1].y <= y_max;
        let dx = points[2].x - points[0].x;
        let dy = points[2].y - points[0].y;
        if depth >= 32 || (monotonic && dx.abs() <= 32 && dy.abs() <= 32) {
            self.line_to(points[2].x, points[2].y);
            return;
        }

        let left_mid = midpoint(points[0], points[1]);
        let right_mid = midpoint(points[1], points[2]);
        let center = midpoint(left_mid, right_mid);
        self.flatten_conic([points[0], left_mid, center], depth + 1);
        self.flatten_conic([center, right_mid, points[2]], depth + 1);
    }

    fn flatten_cubic(&mut self, points: [Point; 4], depth: u8) {
        let y_min = points[0].y.min(points[3].y);
        let y_max = points[0].y.max(points[3].y);
        let monotonic = points[1].y >= y_min
            && points[1].y <= y_max
            && points[2].y >= y_min
            && points[2].y <= y_max;
        let dx = points[3].x - points[0].x;
        let dy = points[3].y - points[0].y;
        if depth >= 32 || (monotonic && dx.abs() <= 32 && dy.abs() <= 32) {
            self.line_to(points[3].x, points[3].y);
            return;
        }

        let p01 = midpoint(points[0], points[1]);
        let p12 = midpoint(points[1], points[2]);
        let p23 = midpoint(points[2], points[3]);
        let p012 = midpoint(p01, p12);
        let p123 = midpoint(p12, p23);
        let center = midpoint(p012, p123);
        self.flatten_cubic([points[0], p01, p012, center], depth + 1);
        self.flatten_cubic([center, p123, p23, points[3]], depth + 1);
    }

    fn decompose(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        contours: &[i16],
        n_contours: i32,
    ) -> Result<(), FontError> {
        let mut last: i64 = -1;
        for (contour, &contour_end) in contours.iter().take(usize_from_i32(n_contours)).enumerate()
        {
            self.contour = contour;
            self.order = 0;
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
            let contour_start = self.segments.len();
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
            let contour_len = self.segments.len() - contour_start;
            for segment in &mut self.segments[contour_start..] {
                segment.contour_len = contour_len;
            }
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

#[derive(Debug, Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) >> 1,
        y: (a.y + b.y) >> 1,
    }
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
