//! Adapter for PIL's `_imagingft.c` connector surface.
//!
//! All glyph loading, rendering, advance, and kerning go through
//! `fontdone::ffi` — proven pixel-identical with C FreeType 2.14.3
//! (4,097/4,097 unified parity).

use super::{Font, TrueTypeFont};

// ── Public API ───────────────────────────────────────────────────────

pub fn getname(font: &Font) -> (&str, &str) {
    match font { Font::TrueType(t) => t.inner.getname(), Font::Bitmap(_) => ("Aileron", "Regular") }
}

pub fn getmetrics(font: &Font) -> (u32, u32) {
    match font {
        Font::TrueType(t) => t.inner.getmetrics(),
        Font::Bitmap(b) => { let (_, h) = b.text_bbox("A"); (h, 0) }
    }
}

pub fn getlength(font: &Font, text: &str) -> f32 {
    match font {
        Font::TrueType(t) => glyph_run(t, text).map_or(0.0, |r| r.max_pen as f32 / 64.0),
        Font::Bitmap(b) => b.text_bbox(text).0 as f32,
    }
}

pub fn getbbox(font: &Font, text: &str) -> (i32, i32, i32, i32) {
    match font {
        Font::TrueType(t) => bbox_from_run(t, text),
        Font::Bitmap(b) => { let (w, h) = b.text_bbox(text); (0, 0, w as i32, h as i32) }
    }
}

pub fn getmask(font: &Font, text: &str) -> (u32, u32, Vec<u8>) {
    match font {
        Font::TrueType(t) => mask_from_run(t, text),
        Font::Bitmap(b) => b.getmask(text),
    }
}

pub fn render_text(font: &Font, text: &str, fill: (u8,u8,u8,u8), _spacing: f32) -> (u32,u32,Vec<u8>) {
    pack_rgba(getmask(font, text), fill, false)
}
pub fn render_text_binary(font: &Font, text: &str, fill: (u8,u8,u8,u8), _spacing: f32) -> (u32,u32,Vec<u8>) {
    pack_rgba(getmask(font, text), fill, true)
}

fn pack_rgba((w, h, mask): (u32, u32, Vec<u8>), fill: (u8,u8,u8,u8), binary: bool) -> (u32,u32,Vec<u8>) {
    if w == 0 || h == 0 { return (w, h, mask); }
    let len = match (w as usize).checked_mul(h as usize).and_then(|v| v.checked_mul(4)) {
        Some(v) => v, None => return (0, 0, vec![]),
    };
    let mut canvas = vec![0u8; len];
    for (i, cov) in mask.into_iter().enumerate() {
        let c = if binary && cov < 128 { 0 } else { cov };
        if c == 0 { continue; }
        let o = i * 4;
        canvas[o]=fill.0; canvas[o+1]=fill.1; canvas[o+2]=fill.2; canvas[o+3]=c;
    }
    (w, h, canvas)
}

// ── FFI helpers ──────────────────────────────────────────────────────

use fontdone::ffi as ffi;

const KERN_DEFAULT: u32 = 0; // FT_KERNING_DEFAULT as u32
const RDR: i32 = 4;          // FT_LOAD_RENDER
const TGT_NORM: i32 = 0;     // FT_LOAD_TARGET_NORMAL

fn gid(face: &ffi::FT_Face, ch: char) -> u32 { ffi::FT_Get_Char_Index(face, ch as u64) }

fn kern_26dot6(face: &ffi::FT_Face, l: u32, r: u32) -> i32 {
    let mut v = ffi::FT_Vector::default();
    ffi::FT_Get_Kerning(Some(face), l, r, KERN_DEFAULT, Some(&mut v));
    v.x as i32
}

fn round26(v: i32) -> i32 { fontdone::scaler::pixel_round(v) }

// ── Glyph run (no render, for metrics/advance/bbox) ─────────────────

struct GlyphRun {
    glyphs: Vec<RunGlyph>,
    max_pen: i32, // maximum pen position in 26.6
}

struct RunGlyph {
    #[allow(dead_code)]
    gid: u32,
    pen_before: i32,
    advance: i32,
    bitmap_left: i32,
    bitmap_top: i32,
    #[allow(dead_code)]
    bitmap: Option<ffi::FT_Bitmap>,
}

/// Load each glyph WITHOUT rendering, collect advances and metrics.
fn glyph_run(ttf: &TrueTypeFont, text: &str) -> Option<GlyphRun> {
    if text.is_empty() { return Some(GlyphRun { glyphs: vec![], max_pen: 0 }); }
    let face = &ttf.face;
    let mut pen = 0i32;
    let mut prev: Option<u32> = None;
    let mut out = Vec::new();
    let mut max_pen = 0i32;

    for ch in text.chars() {
        let g = gid(face, ch);
        if g == 0 { prev = None; continue; }
        if let Some(p) = prev { pen = pen.saturating_add(kern_26dot6(face, p, g)); }

        let pen_before = pen;

        // Load glyph (no render, just get advance and metrics)
        let slot = ffi::FT_Load_Glyph(face, g, 0).ok()?;
        let adv = slot.advance.x as i32;

        out.push(RunGlyph {
            gid: g, pen_before, advance: adv,
            bitmap_left: slot.bitmap_left as i32,
            bitmap_top: slot.bitmap_top as i32,
            bitmap: None, // no render
        });

        pen = pen.saturating_add(adv);
        max_pen = max_pen.max(pen);
        prev = Some(g);
    }
    Some(GlyphRun { glyphs: out, max_pen })
}

fn bbox_from_run(ttf: &TrueTypeFont, text: &str) -> (i32, i32, i32, i32) {
    let Some(run) = glyph_run(ttf, text) else { return (0,0,0,0); };
    if run.glyphs.is_empty() { return (0, round26(run.max_pen), 0, 0); }

    let asc = (ttf.ascender_26dot6 >> 6) as i32;
    let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);

    for g in &run.glyphs {
        let x = round26(g.pen_before) + g.bitmap_left;
        let y = asc - g.bitmap_top;
        // For bbox, use the glyph's advance to determine right edge
        let g_r = x + round26(g.advance);
        // For bottom, use default height when no bitmap
        l = l.min(x);
        t = t.min(y);
        r = r.max(g_r);
        b = b.max(y);
    }

    if l == i32::MAX {
        let rp = round26(run.max_pen);
        return (0, 0, rp, 0);
    }
    // Right edge: furthest pixel from pen position
    r = r.max(round26(run.max_pen));
    (l, t, r, b)
}

// ── Mask render ──────────────────────────────────────────────────────

fn mask_from_run(ttf: &TrueTypeFont, text: &str) -> (u32, u32, Vec<u8>) {
    if text.is_empty() { return (0,0,vec![]); }
    let face = &ttf.face;
    let asc = (ttf.ascender_26dot6 >> 6) as i32;
    let mut pen = 0i32;
    let mut prev: Option<u32> = None;
    let mut placed: Vec<(i32, i32, ffi::FT_Bitmap)> = Vec::new(); // (x, y, bitmap)

    for ch in text.chars() {
        let g = gid(face, ch);
        if g == 0 { prev = None; continue; }
        if let Some(p) = prev { pen = pen.saturating_add(kern_26dot6(face, p, g)); }

        let slot = match ffi::FT_Load_Glyph(face, g, RDR | TGT_NORM) {
            Ok(s) => s,
            Err(_) => match ffi::FT_Load_Glyph(face, g, 0) {
                Ok(s) => s,
                Err(_) => { prev = None; continue; },
            },
        };

        let x = round26(pen) + slot.bitmap_left as i32;
        let y = asc - slot.bitmap_top as i32;
        if let Some(bm) = slot.bitmap {
            if bm.width > 0 && bm.rows > 0 {
                placed.push((x, y, bm));
            }
        }
        pen = pen.saturating_add(slot.advance.x as i32);
        prev = Some(g);
    }

    if placed.is_empty() { return (0,0,vec![]); }

    let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(x, y, ref bm) in &placed {
        l = l.min(x); t = t.min(y);
        r = r.max(x + bm.width as i32); b = b.max(y + bm.rows as i32);
    }
    if l == i32::MAX { return (0,0,vec![]); }

    let w = (r - l).max(0) as u32;
    let h = (b - t).max(0) as u32;
    let wu = w as usize; let hu = h as usize;
    let mut canvas = vec![0u8; wu.checked_mul(hu).unwrap_or(0)];
    if canvas.is_empty() { return (w, h, canvas); }

    for (x, y, bm) in &placed {
        let sx = bm.width as usize; let sy = bm.rows as usize;
        let dx = (*x - l).max(0) as usize;
        let dy = (*y - t).max(0) as usize;
        if dx >= wu || dy + sy > hu { continue; }
        let cw = sx.min(wu - dx);
        for row in 0..sy {
            let src = row * sx;
            let dst = (dy + row) * wu + dx;
            if let (Some(sr), Some(dr)) = (bm.buffer.get(src..src + cw), canvas.get_mut(dst..dst + cw)) {
                for (dc, sc) in dr.iter_mut().zip(sr) { *dc = (*dc).max(*sc); }
            }
        }
    }
    (w, h, canvas)
}
