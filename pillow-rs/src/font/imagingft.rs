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
struct LayoutGlyph {
    x: i32,
    y: i32,
    mask_index: usize,
}

#[derive(Debug, Clone)]
struct TextLayout {
    bbox: (i32, i32, i32, i32),
    masks: Vec<fontdone::GlyphMask>,
    glyphs: Vec<LayoutGlyph>,
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
        Font::TrueType(ttf) => text_advance_26dot6(ttf, text) as f32 / 64.0,
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
    layout_run(ttf, text).map_or((0, 0, 0, 0), |layout| layout.bbox)
}

fn layout_mask(ttf: &super::TrueTypeFont, text: &str) -> Option<TextMask> {
    let layout = layout_run(ttf, text)?;
    let (left, top, right, bottom) = layout.bbox;
    let width = u32::try_from((right - left).max(0)).ok()?;
    let height = u32::try_from((bottom - top).max(0)).ok()?;
    let width_usize = width as usize;
    let height_usize = height as usize;
    let mut pixels = vec![0u8; width_usize.checked_mul(height_usize)?];

    for glyph in &layout.glyphs {
        let mask = layout.masks.get(glyph.mask_index)?;
        if mask.width == 0 || mask.height == 0 {
            continue;
        }
        let dst_x = usize::try_from(glyph.x - left).ok()?;
        let dst_y = usize::try_from(glyph.y - top).ok()?;
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

fn text_advance_26dot6(ttf: &super::TrueTypeFont, text: &str) -> i32 {
    let mut cursor_26dot6 = 0i32;
    let mut previous = None;
    for ch in text.chars() {
        if let Some(previous) = previous {
            cursor_26dot6 = cursor_26dot6.saturating_add(ttf.inner.getkerning(previous, ch));
        }
        cursor_26dot6 =
            cursor_26dot6.saturating_add(ttf.inner.glyph_hori_advance_26dot6(ch as u32));
        previous = Some(ch);
    }
    cursor_26dot6
}

fn layout_run(ttf: &super::TrueTypeFont, text: &str) -> Option<TextLayout> {
    if text.is_empty() {
        return Some(TextLayout {
            bbox: (0, 0, 0, 0),
            masks: Vec::new(),
            glyphs: Vec::new(),
        });
    }

    let ascent = i32::try_from(ttf.inner.getmetrics().0).ok()?;
    let mut cursor_26dot6 = 0i32;
    let mut previous = None;
    let mut left = 0i32;
    let mut top = i32::MAX;
    let mut right = i32::MIN;
    let mut bottom = i32::MIN;
    let mut masks = Vec::new();
    let mut glyphs = Vec::new();

    for ch in text.chars() {
        if let Some(previous) = previous {
            cursor_26dot6 = cursor_26dot6.saturating_add(ttf.inner.getkerning(previous, ch));
        }

        let mut encoded = [0u8; 4];
        let ch_str = ch.encode_utf8(&mut encoded);
        let mask = ttf.inner.getmask(ch_str).ok()?;
        let width = i32::try_from(mask.width).ok()?;
        let height = i32::try_from(mask.height).ok()?;
        let glyph_left = fontdone::scaler::pixel_round(cursor_26dot6).saturating_add(mask.xmin);
        let glyph_top_y = mask.ymin.saturating_add(height);
        let ink_top = ascent.saturating_sub(glyph_top_y);
        let bbox_top = ascent.saturating_sub(glyph_top_y.max(0));
        let bbox_bottom = ascent.saturating_sub(mask.ymin.min(0));

        if width > 0 && height > 0 {
            left = left.min(glyph_left);
            top = top.min(bbox_top);
            right = right.max(glyph_left.saturating_add(width));
            bottom = bottom.max(bbox_bottom);
        }

        let mask_index = masks.len();
        masks.push(mask);
        glyphs.push(LayoutGlyph {
            x: glyph_left,
            y: ink_top,
            mask_index,
        });

        cursor_26dot6 =
            cursor_26dot6.saturating_add(ttf.inner.glyph_hori_advance_26dot6(ch as u32));
        previous = Some(ch);
    }

    let rounded_advance = fontdone::scaler::pixel_round(cursor_26dot6);
    if top == i32::MAX {
        top = 0;
        right = rounded_advance;
        bottom = 0;
    } else {
        right = right.max(rounded_advance);
    }

    // PIL's `_imagingft` bbox is in text-run coordinates: y is measured down
    // from ascender space, the baseline is included in the vertical extent, x
    // includes the run origin, and bitmap placement follows the rounded 26.6
    // pen position used by FreeType's glyph cache path.
    Some(TextLayout {
        bbox: (left, top, right, bottom),
        masks,
        glyphs,
    })
}
