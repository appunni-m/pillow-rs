//! Adapter for PIL's `_imagingft.c` connector surface.
//!
//! This module is intentionally separate from the high-level `Font` type. It is
//! where PIL-style text metrics, masks, and draw-facing rendering behavior are
//! compared against the version-matched Pillow oracle.

use super::Font;

#[derive(Debug, Clone)]
struct TextMask {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct PositionedGlyph {
    bbox_x: i32,
    mask_x: i32,
    bbox: (i32, i32, i32, i32),
}

/// Returns `(family, style)` font names.
///
/// Bitmap fallback fonts report the fixed name used by the bundled default
/// glyph set.
pub fn getname(font: &Font) -> (&str, &str) {
    match font {
        Font::TrueType(ttf) => ttf.inner.getname(),
        Font::Bitmap(_) => ("Aileron", "Regular"),
    }
}

/// Returns `(ascent, descent)` in pixels.
pub fn getmetrics(font: &Font) -> (u32, u32) {
    match font {
        Font::TrueType(ttf) => ttf.inner.getmetrics(),
        Font::Bitmap(bf) => {
            let (_, height) = bf.text_bbox("A");
            (height, 0)
        }
    }
}

/// Returns text advance in pixels.
pub fn getlength(font: &Font, text: &str) -> f32 {
    match font {
        Font::TrueType(ttf) => ttf.inner.getlength(text),
        Font::Bitmap(bf) => bf.text_bbox(text).0 as f32,
    }
}

/// Returns Pillow-style text bbox `(left, top, right, bottom)`.
pub fn getbbox(font: &Font, text: &str) -> (i32, i32, i32, i32) {
    match font {
        Font::TrueType(ttf) => layout_bbox(ttf, text),
        Font::Bitmap(bf) => {
            let (width, height) = bf.text_bbox(text);
            (0, 0, width as i32, height as i32)
        }
    }
}

/// Renders text as an `L`-mode alpha mask.
///
/// This mirrors Pillow `FreeTypeFont.getmask`.
///
/// # Returns
///
/// `(width, height, mask_bytes)` with one coverage byte per pixel.
pub fn getmask(font: &Font, text: &str) -> (u32, u32, Vec<u8>) {
    match font {
        Font::TrueType(ttf) => match layout_mask(ttf, text) {
            Some(mask) => (mask.width, mask.height, mask.pixels),
            None => (0, 0, vec![]),
        },
        Font::Bitmap(bf) => bf.getmask(text),
    }
}

/// Renders text to RGBA bytes.
///
/// `fill` is copied into RGB channels and glyph coverage controls alpha.
///
/// # Returns
///
/// `(width, height, rgba_bytes)` with tightly packed RGBA pixels.
pub fn render_text(
    font: &Font,
    text: &str,
    fill: (u8, u8, u8, u8),
    spacing: f32,
) -> (u32, u32, Vec<u8>) {
    render_text_impl(font, text, fill, spacing, false)
}

/// Renders text with Pillow `fontmode="1"` binary coverage.
///
/// Coverage values are thresholded to binary mask output before RGBA packing.
pub fn render_text_binary(
    font: &Font,
    text: &str,
    fill: (u8, u8, u8, u8),
    spacing: f32,
) -> (u32, u32, Vec<u8>) {
    render_text_impl(font, text, fill, spacing, true)
}

fn render_text_impl(
    font: &Font,
    text: &str,
    fill: (u8, u8, u8, u8),
    spacing: f32,
    binary: bool,
) -> (u32, u32, Vec<u8>) {
    match font {
        Font::TrueType(_) => {
            let (w, h, mask) = getmask(font, text);
            if w == 0 || h == 0 {
                return (w, h, mask);
            }

            let Some(len) = (w as usize)
                .checked_mul(h as usize)
                .and_then(|v| v.checked_mul(4))
            else {
                return (0, 0, vec![]);
            };
            let mut canvas = vec![0u8; len];
            for (index, cov) in mask.into_iter().enumerate() {
                let effective_cov = if binary && cov < 128 { 0 } else { cov };
                if effective_cov == 0 {
                    continue;
                }
                let dst_off = index * 4;
                canvas[dst_off] = fill.0;
                canvas[dst_off + 1] = fill.1;
                canvas[dst_off + 2] = fill.2;
                canvas[dst_off + 3] = effective_cov;
            }

            (w, h, canvas)
        }
        Font::Bitmap(bf) => {
            if binary {
                bf.render_text_binary(text, fill, spacing)
            } else {
                bf.render_text(text, fill, spacing)
            }
        }
    }
}

fn layout_bbox(ttf: &super::TrueTypeFont, text: &str) -> (i32, i32, i32, i32) {
    let glyphs = positioned_glyphs(ttf, text);
    if glyphs.is_empty() {
        return (0, 0, 0, 0);
    }
    let (left, top, right, bottom) = glyphs.into_iter().fold(
        (i32::MAX, i32::MAX, i32::MIN, i32::MIN),
        |(left, top, right, bottom), glyph| {
            (
                left.min(glyph.bbox_x + glyph.bbox.0),
                top.min(glyph.bbox.1),
                right.max(glyph.bbox_x + glyph.bbox.2),
                bottom.max(glyph.bbox.3),
            )
        },
    );

    // PIL's `_imagingft` connector exposes a text-layout bbox, while
    // `freetype` exposes FreeType glyph-slot boxes.  For positive
    // origin multi-glyph text, the connector lets the run advance define the
    // right edge when it extends farther than rendered ink.  Single glyphs and
    // negative-left runs keep their ink bbox, matching the generated oracle.
    let has_multiple_glyphs = text.chars().nth(1).is_some();
    let right = if has_multiple_glyphs && left >= 0 {
        let advance_26dot6 = (ttf.inner.getlength(text) * 64.0).round() as i32;
        right.max(freetype::scaler::pixel_round(advance_26dot6))
    } else {
        right
    };
    (left, top, right, bottom)
}

fn layout_mask(ttf: &super::TrueTypeFont, text: &str) -> Option<TextMask> {
    let glyphs = positioned_glyphs(ttf, text);
    if glyphs.is_empty() {
        return Some(TextMask {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        });
    }

    let (left, top, right, bottom) = layout_bbox(ttf, text);
    let width = u32::try_from((right - left).max(0)).ok()?;
    let height = u32::try_from((bottom - top).max(0)).ok()?;
    let width_usize = width as usize;
    let height_usize = height as usize;
    let mut pixels = vec![0u8; width_usize.checked_mul(height_usize)?];

    for (ch, glyph) in text.chars().zip(glyphs) {
        let mask = ttf.inner.getmask(&ch.to_string()).ok()?;
        if mask.width == 0 || mask.height == 0 {
            continue;
        }
        let dst_x = usize::try_from(glyph.mask_x + glyph.bbox.0 - left).ok()?;
        let dst_y = usize::try_from(glyph.bbox.1 - top).ok()?;
        let src_width = mask.width as usize;
        let src_height = mask.height as usize;
        if dst_x >= width_usize || dst_y + src_height > height_usize {
            return None;
        }
        let copy_width = src_width.min(width_usize - dst_x);
        for y in 0..src_height {
            let src = y.checked_mul(src_width)?;
            let dst = (dst_y + y).checked_mul(width_usize)?.checked_add(dst_x)?;
            let src_row = mask.pixels.get(src..src + copy_width)?;
            let dst_row = pixels.get_mut(dst..dst + copy_width)?;
            for (dst, src) in dst_row.iter_mut().zip(src_row) {
                *dst = (*dst).max(*src);
            }
        }
    }

    Some(TextMask {
        width,
        height,
        pixels,
    })
}

fn positioned_glyphs(ttf: &super::TrueTypeFont, text: &str) -> Vec<PositionedGlyph> {
    let mut glyphs = Vec::new();
    let mut cursor_26dot6 = 0i32;
    let mut previous = None;
    for ch in text.chars() {
        if let Some(previous) = previous {
            cursor_26dot6 += ttf.inner.getkerning(previous, ch);
        }
        let bbox = ttf.inner.getbbox(&ch.to_string());
        glyphs.push(PositionedGlyph {
            bbox_x: freetype::scaler::pixel_floor(cursor_26dot6),
            mask_x: freetype::scaler::pixel_round(cursor_26dot6),
            bbox,
        });
        cursor_26dot6 +=
            i32::try_from((ttf.inner.getlength(&ch.to_string()) * 64.0).round() as i64)
                .unwrap_or(0);
        previous = Some(ch);
    }
    glyphs
}
