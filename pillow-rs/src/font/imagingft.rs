//! Adapter for PIL's `_imagingft.c` connector surface.
//!
//! This module is intentionally separate from the high-level `Font` type. It is
//! where PIL-style text metrics, masks, and draw-facing rendering behavior are
//! compared against the version-matched Pillow oracle.

use super::Font;

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
        Font::TrueType(ttf) => ttf.inner.getbbox(text),
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
        Font::TrueType(ttf) => {
            let mask = match ttf.inner.getmask(text) {
                Ok(mask) => mask,
                Err(_) => return (0, 0, vec![]),
            };
            (mask.width, mask.height, mask.pixels)
        }
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
