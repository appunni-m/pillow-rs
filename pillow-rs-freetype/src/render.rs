//! FreeType-style glyph render modes and bitmap helpers.
//!
//! Normal and LCD modes use the smooth rasterizer.  The vendored FreeType
//! configuration keeps `FT_CONFIG_OPTION_SUBPIXEL_RENDERING` disabled, so LCD
//! and LCD_V follow the Harmony renderer geometry from `ftsmooth.c`.

use crate::casts::{i32_from_usize, u32_from_usize, usize_from_i32};
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
        let scaled =
            scaler::scale_glyph(&self.data, glyph, metrics_cache.as_ref(), self.is_italic)?;

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
            RenderMode::Mono => render_mono(scaled.outline, scaled.bbox_x_min, scaled.bbox_y_max),
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

fn render_mono(outline: Outline, left: i32, top: i32) -> Result<RenderedBitmap, FontError> {
    let raster = grays::rasterize(outline)?;
    let pitch = mono_pitch(raster.width);
    let mut buffer = vec![0u8; pitch * raster.height];
    for y in 0..raster.height {
        let src_row = y * raster.width;
        let dst_row = y * pitch;
        for x in 0..raster.width {
            if raster.pixels[src_row + x] >= 128 {
                buffer[dst_row + x / 8] |= 0x80 >> (x & 7);
            }
        }
    }
    Ok(RenderedBitmap {
        width: u32_from_usize(raster.width),
        rows: u32_from_usize(raster.height),
        pitch: i32_from_usize(pitch),
        pixel_mode: PixelMode::Mono,
        left,
        top,
        buffer,
    })
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
