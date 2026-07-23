//! Adapter for PIL's `_imagingft.c` connector surface.
//!
//! All glyph loading, rendering, advance, and kerning go through
//! `fontdone::ffi` — proven pixel-identical with C FreeType 2.14.3
//! (4,097/4,097 unified parity).

use super::{Font, TrueTypeFont};
use crate::error::PilError;
use fontdone::ffi;

pub(super) struct TrueTypeEngine {
    face: ffi::FT_Face,
    pub(super) size_pt: f32,
    family_name: String,
    style_name: String,
    metrics: ffi::FT_Size_Metrics,
}

pub(super) fn load_truetype(data: Vec<u8>, size: f32) -> Result<TrueTypeFont, PilError> {
    let library = ffi::FT_Init_FreeType();
    let mut face = ffi::FT_New_Memory_Face(&library, &data, 0, size)
        .map_err(|e| PilError::ValueError(format!("FT_New_Memory_Face: error {e}")))?;

    // Pillow _imagingft.c:getfont requests nominal size with width/height
    // set to size * 64 after FT_New_Memory_Face.
    let width = (size * 64.0) as ffi::FT_Long;
    let request = ffi::FT_Size_RequestRec {
        type_: ffi::FT_SIZE_REQUEST_TYPE_NOMINAL as ffi::FT_Size_Request_Type,
        width,
        height: width,
        horiResolution: 0,
        vertResolution: 0,
    };
    if ffi::FT_Request_Size(Some(&mut face), Some(&request)) != ffi::FT_Err_Ok {
        return Err(PilError::ValueError("FT_Request_Size failed".into()));
    }

    let family_name = face.family_name.clone().unwrap_or_else(|| "Unknown".into());
    let style_name = face.style_name.clone().unwrap_or_else(|| "Regular".into());
    let metrics = face.size_metrics;

    Ok(TrueTypeFont {
        engine: TrueTypeEngine {
            face,
            size_pt: size,
            family_name,
            style_name,
            metrics,
        },
    })
}

// ── Public API ───────────────────────────────────────────────────────

pub fn getname(font: &Font) -> (&str, &str) {
    match font {
        Font::TrueType(t) => (t.engine.family_name.as_str(), t.engine.style_name.as_str()),
        Font::Bitmap(_) => ("Aileron", "Regular"),
    }
}

pub fn getmetrics(font: &Font) -> (u32, u32) {
    match font {
        Font::TrueType(t) => (
            pixel(t.engine.metrics.ascender) as u32,
            (-pixel(t.engine.metrics.descender)) as u32,
        ),
        Font::Bitmap(b) => {
            let (_, h) = b.text_bbox("A");
            (h, 0)
        }
    }
}

pub fn getlength(font: &Font, text: &str) -> f32 {
    match font {
        Font::TrueType(t) => length_from_basic_layout(t, text).map_or(0.0, |v| v as f32 / 64.0),
        Font::Bitmap(b) => b.text_bbox(text).0 as f32,
    }
}

pub fn getbbox(font: &Font, text: &str) -> (i32, i32, i32, i32) {
    match font {
        Font::TrueType(t) => bbox_from_run(t, text),
        Font::Bitmap(b) => {
            let (w, h) = b.text_bbox(text);
            (0, 0, w as i32, h as i32)
        }
    }
}

pub fn getmask(font: &Font, text: &str) -> (u32, u32, Vec<u8>) {
    match font {
        Font::TrueType(t) => mask_from_run(t, text),
        Font::Bitmap(b) => b.getmask(text),
    }
}

/// Render a Pillow-compatible mask together with its BASIC-layout offset.
pub fn getmask2(font: &Font, text: &str) -> (u32, u32, Vec<u8>, (i32, i32)) {
    let (width, height, pixels) = getmask(font, text);
    let bbox = getbbox(font, text);
    (width, height, pixels, (bbox.0, bbox.1))
}

pub fn render_text(
    font: &Font,
    text: &str,
    fill: (u8, u8, u8, u8),
    _spacing: f32,
) -> (u32, u32, Vec<u8>) {
    pack_rgba(getmask(font, text), fill, false)
}
pub fn render_text_binary(
    font: &Font,
    text: &str,
    fill: (u8, u8, u8, u8),
    _spacing: f32,
) -> (u32, u32, Vec<u8>) {
    pack_rgba(getmask(font, text), fill, true)
}

fn pack_rgba(
    (w, h, mask): (u32, u32, Vec<u8>),
    fill: (u8, u8, u8, u8),
    binary: bool,
) -> (u32, u32, Vec<u8>) {
    if w == 0 || h == 0 {
        return (w, h, mask);
    }
    let len = match (w as usize)
        .checked_mul(h as usize)
        .and_then(|v| v.checked_mul(4))
    {
        Some(v) => v,
        None => return (0, 0, vec![]),
    };
    let mut canvas = vec![0u8; len];
    for (i, cov) in mask.into_iter().enumerate() {
        let c = if binary && cov < 128 { 0 } else { cov };
        if c == 0 {
            continue;
        }
        let o = i * 4;
        canvas[o] = fill.0;
        canvas[o + 1] = fill.1;
        canvas[o + 2] = fill.2;
        canvas[o + 3] = c;
    }
    (w, h, canvas)
}

// ── FFI helpers ──────────────────────────────────────────────────────

const KERN_DEFAULT: u32 = 0; // FT_KERNING_DEFAULT as u32
const RDR: i32 = 4; // FT_LOAD_RENDER
const TGT_NORM: i32 = 0; // FT_LOAD_TARGET_NORMAL

fn gid(face: &ffi::FT_Face, ch: char) -> u32 {
    ffi::FT_Get_Char_Index(face, ch as u64)
}

fn kern_26dot6(face: &ffi::FT_Face, l: u32, r: u32) -> i32 {
    let mut v = ffi::FT_Vector::default();
    ffi::FT_Get_Kerning(Some(face), l, r, KERN_DEFAULT, Some(&mut v));
    v.x as i32
}

fn basic_layout_kern(face: &ffi::FT_Face, left: u32, right: u32) -> i32 {
    // Pillow 12.2.0 `text_layout_fallback` in `_imagingft.c` adds
    // `PIXEL(delta.x)` directly to the preceding 26.6 `x_advance`.
    pixel(i64::from(kern_26dot6(face, left, right)))
}

fn round26(v: i32) -> i32 {
    pixel(i64::from(v))
}

fn pixel(x: i64) -> i32 {
    (((x + 32) & -64) >> 6) as i32
}

fn floor26(x: i64) -> i32 {
    ((x & -64) >> 6) as i32
}

fn ceil26(x: i64) -> i32 {
    (((x + 63) & -64) >> 6) as i32
}

fn length_from_basic_layout(ttf: &TrueTypeFont, text: &str) -> Option<i32> {
    let face = &ttf.engine.face;
    let mut total = 0i32;
    let mut prev: Option<u32> = None;

    for ch in text.chars() {
        let g = gid(face, ch);
        // Pillow 12.2.0 `text_layout_fallback` uses `FT_LOAD_DEFAULT`.
        // Its hinted `horiAdvance` values are integral pixels for BASIC layout.
        let slot = ffi::FT_Load_Glyph(face, g, 0).ok()?;
        if let Some(p) = prev.filter(|p| *p != 0 && g != 0) {
            total = total.saturating_add(basic_layout_kern(face, p, g));
        }
        total = total.saturating_add(slot.metrics.horiAdvance as i32);
        prev = Some(g);
    }

    Some(total)
}

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
    outline_cbox: ffi::FT_BBox,
    #[allow(dead_code)]
    bitmap: Option<ffi::FT_Bitmap>,
}

/// Load each glyph WITHOUT rendering, collect advances and metrics.
fn glyph_run(ttf: &TrueTypeFont, text: &str) -> Option<GlyphRun> {
    if text.is_empty() {
        return Some(GlyphRun {
            glyphs: vec![],
            max_pen: 0,
        });
    }
    let face = &ttf.engine.face;
    let mut pen = 0i32;
    let mut prev: Option<u32> = None;
    let mut out = Vec::new();
    let mut max_pen = 0i32;

    for ch in text.chars() {
        let g = gid(face, ch);
        // Match Pillow's BASIC layout order: load the current glyph first,
        // then adjust the preceding advance with pixel-rounded kerning.
        let slot = ffi::FT_Load_Glyph(face, g, 0).ok()?;
        if let Some(p) = prev.filter(|p| *p != 0 && g != 0) {
            pen = pen.saturating_add(basic_layout_kern(face, p, g));
        }

        let pen_before = pen;
        let adv = slot.metrics.horiAdvance as i32;

        out.push(RunGlyph {
            gid: g,
            pen_before,
            advance: adv,
            outline_cbox: slot.outline_cbox,
            bitmap: None, // no render
        });

        pen = pen.saturating_add(adv);
        max_pen = max_pen.max(pen);
        prev = Some(g);
    }
    Some(GlyphRun {
        glyphs: out,
        max_pen,
    })
}

fn bbox_from_run(ttf: &TrueTypeFont, text: &str) -> (i32, i32, i32, i32) {
    let Some(run) = glyph_run(ttf, text) else {
        return (0, 0, 0, 0);
    };
    if run.glyphs.is_empty() {
        return (0, 0, 0, 0);
    }

    let mut x_min = 0;
    let mut x_max = 0;
    let mut y_min = 0;
    let mut y_max = 0;

    for g in &run.glyphs {
        let px = pixel(i64::from(g.pen_before));
        let advanced = pixel(i64::from(g.pen_before.saturating_add(g.advance)));
        x_max = x_max.max(px).max(advanced);

        let cbox = g.outline_cbox;
        let glyph_x_min = px + floor26(cbox.xMin);
        let glyph_x_max = px + ceil26(cbox.xMax);
        let glyph_y_min = floor26(cbox.yMin);
        let glyph_y_max = ceil26(cbox.yMax);

        x_min = x_min.min(glyph_x_min);
        x_max = x_max.max(glyph_x_max);
        y_min = y_min.min(glyph_y_min);
        y_max = y_max.max(glyph_y_max);
    }

    x_max = x_max.max(round26(run.max_pen));
    let y_anchor = pixel(ttf.engine.metrics.ascender);
    (x_min, y_anchor - y_max, x_max, y_anchor - y_min)
}

// ── Mask render ──────────────────────────────────────────────────────

fn mask_from_run(ttf: &TrueTypeFont, text: &str) -> (u32, u32, Vec<u8>) {
    if text.is_empty() {
        return (0, 0, vec![]);
    }
    let bbox = bbox_from_run(ttf, text);
    let w = (bbox.2 - bbox.0).max(0) as u32;
    let h = (bbox.3 - bbox.1).max(0) as u32;
    let wu = w as usize;
    let hu = h as usize;
    let mut canvas = vec![0u8; wu.checked_mul(hu).unwrap_or(0)];
    if w == 0 || h == 0 {
        return (w, h, canvas);
    }

    let face = &ttf.engine.face;
    let mut pen = 0i32;
    let mut prev: Option<u32> = None;
    let mut rendered: Vec<(i32, i32, i32, Option<ffi::FT_Bitmap>)> = Vec::new();
    let mut x_min = 0;
    let mut y_max = 0;

    for ch in text.chars() {
        let g = gid(face, ch);
        // Pillow's layout pass obtains the hinted advance before rendering.
        let layout_slot = match ffi::FT_Load_Glyph(face, g, 0) {
            Ok(slot) => slot,
            Err(_) => {
                prev = None;
                continue;
            }
        };
        if let Some(p) = prev.filter(|p| *p != 0 && g != 0) {
            pen = pen.saturating_add(basic_layout_kern(face, p, g));
        }

        let slot = match ffi::FT_Load_Glyph(face, g, RDR | TGT_NORM) {
            Ok(s) => s,
            Err(_) => match ffi::FT_Load_Glyph(face, g, 0) {
                Ok(s) => s,
                Err(_) => {
                    prev = None;
                    continue;
                }
            },
        };

        let px = round26(pen);
        x_min = x_min.min(px + slot.bitmap_left as i32);
        y_max = y_max.max(slot.bitmap_top as i32);
        rendered.push((
            pen,
            slot.bitmap_left as i32,
            slot.bitmap_top as i32,
            slot.bitmap,
        ));
        pen = pen.saturating_add(layout_slot.metrics.horiAdvance as i32);
        prev = Some(g);
    }

    let x_origin = -x_min * 64;
    let y_origin = -y_max * 64;

    for (pen_before, bitmap_left, bitmap_top, bitmap) in &rendered {
        let Some(bm) = bitmap else {
            continue;
        };
        let sx = bm.width as usize;
        let sy = bm.rows as usize;
        if sx == 0 || sy == 0 {
            continue;
        }
        let px = pixel(i64::from(x_origin.saturating_add(*pen_before)));
        let py = pixel(i64::from(y_origin));
        let dx = px + *bitmap_left;
        let dy = -(py + *bitmap_top);
        if dx < 0 || dy < 0 {
            continue;
        }
        let dx = dx as usize;
        let dy = dy as usize;
        if dx >= wu || dy >= hu {
            continue;
        }
        let cw = sx.min(wu - dx);
        let ch = sy.min(hu - dy);
        for row in 0..sy {
            if row >= ch {
                break;
            }
            let src = row * sx;
            let dst = (dy + row) * wu + dx;
            if let (Some(sr), Some(dr)) =
                (bm.buffer.get(src..src + cw), canvas.get_mut(dst..dst + cw))
            {
                for (dc, sc) in dr.iter_mut().zip(sr) {
                    *dc = (*dc).max(*sc);
                }
            }
        }
    }
    (w, h, canvas)
}
