//! Adapter for PIL's `_imagingft.c` connector surface.
//!
//! All glyph loading, rendering, advance, and kerning go through
//! `fontdone::ffi` — proven pixel-identical with C FreeType 2.14.3
//! (4,097/4,097 unified parity).

use super::Font;
use crate::error::PilError;
use crate::image::Image;
use fontdone::ffi;

pub(super) struct TrueTypeEngine {
    face: ffi::FT_Face,
    pub(super) size_pt: f32,
    family_name: Option<String>,
    style_name: Option<String>,
    metrics: ffi::FT_Size_Metrics,
}

pub(super) fn load_truetype(data: Vec<u8>, size: f32) -> Result<Font, PilError> {
    if !(size > 0.0) {
        return Err(PilError::ValueError(format!(
            "font size must be greater than 0, not {}",
            if size.fract() == 0.0 {
                format!("{:.0}", size)
            } else {
                size.to_string()
            }
        )));
    }

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
    let size_error = ffi::FT_Request_Size(Some(&mut face), Some(&request));
    if size_error != ffi::FT_Err_Ok {
        let message = match size_error {
            x if x == ffi::FT_Err_Invalid_Argument as i32 => "invalid argument",
            x if x == ffi::FT_Err_Invalid_Pixel_Size as i32 => "invalid pixel size",
            x if x == ffi::FT_Err_Invalid_PPem as i32 => "invalid ppem value",
            _ => "FT_Request_Size failed",
        };
        return Err(PilError::OsError(message.into()));
    }

    let family_name = face.family_name.clone();
    let style_name = face.style_name.clone();
    let metrics = face.size_metrics;

    Ok(Font {
        engine: TrueTypeEngine {
            face,
            size_pt: size,
            family_name,
            style_name,
            metrics,
        },
    })
}

fn ft_error_to_pil(error: i32) -> PilError {
    match error {
        x if x == ffi::FT_Err_Invalid_Argument as i32 => {
            PilError::OsError("invalid argument".into())
        }
        x if x == ffi::FT_Err_Invalid_Pixel_Size as i32 => {
            PilError::OsError("invalid pixel size".into())
        }
        x if x == ffi::FT_Err_Invalid_PPem as i32 => PilError::OsError("invalid ppem".into()),
        x if x == ffi::FT_Err_Too_Many_Instruction_Defs as i32 => {
            PilError::OsError("too many instruction definitions".into())
        }
        x if x == ffi::FT_Err_Too_Many_Function_Defs as i32 => {
            PilError::OsError("too many function definitions".into())
        }
        x if x == ffi::FT_Err_Code_Overflow as i32 => PilError::OsError("code overflow".into()),
        x if x == ffi::FT_Err_Nested_DEFS as i32 => PilError::OsError("nested DEFS".into()),
        other => PilError::ValueError(format!("FreeType error {other}")),
    }
}

// ── Public API ───────────────────────────────────────────────────────

pub(crate) fn getname_optional(font: &Font) -> (Option<&str>, Option<&str>) {
    (
        font.engine.family_name.as_deref(),
        font.engine.style_name.as_deref(),
    )
}

/// Normalize a wrapped font bounding box using Pillow's `TransposedFont` rules.
///
/// Pillow moves the top-left to the origin and swaps the extents only for
/// `ROTATE_90` and `ROTATE_270`. Other transpose methods retain the extents.
#[must_use]
pub(crate) fn transposed_bbox(
    (left, top, right, bottom): (i32, i32, i32, i32),
    orientation: Option<&str>,
) -> (i32, i32, i32, i32) {
    let width = right - left;
    let height = bottom - top;
    if transposed_swaps_axes(orientation) {
        (0, 0, height, width)
    } else {
        (0, 0, width, height)
    }
}

/// Validate whether Pillow defines text length for a transposed font.
///
/// # Errors
///
/// Returns Pillow's exact [`PilError::ValueError`] for 90° and 270° rotation.
pub(crate) fn validate_transposed_length(orientation: Option<&str>) -> Result<(), PilError> {
    if transposed_swaps_axes(orientation) {
        return Err(PilError::ValueError(
            "text length is undefined for text rotated by 90 or 270 degrees".into(),
        ));
    }
    Ok(())
}

fn transposed_swaps_axes(orientation: Option<&str>) -> bool {
    matches!(orientation, Some("ROTATE_90" | "ROTATE_270"))
}

/// Render a font mask and apply Pillow's optional transpose operation.
///
/// # Errors
///
/// Returns [`PilError`] when the requested transpose is invalid or the mask
/// pipeline cannot be materialized.
pub(crate) fn get_transposed_mask(
    font: &Font,
    text: &str,
    orientation: Option<&str>,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    let (width, height, pixels) = getmask(font, text)?;
    let image = Image::from_luma_mask(width, height, pixels)?;
    let transformed = if let Some(method) = orientation {
        image.transpose(method)?
    } else {
        image
    };
    let (width, height) = transformed.size()?;
    Ok((width, height, transformed.tobytes_unpacked()?))
}

pub(crate) fn getmetrics(font: &Font) -> (u32, u32) {
    (
        pixel(font.engine.metrics.ascender) as u32,
        (-pixel(font.engine.metrics.descender)) as u32,
    )
}

/// Return whether the loaded face exposes OpenType or Type 1 variation axes.
pub(crate) fn has_variations(font: &Font) -> bool {
    font.engine.face.face_flags & ffi::FT_FACE_FLAG_MULTIPLE_MASTERS != 0
}

pub(crate) fn getlength(font: &Font, text: &str) -> Result<f32, PilError> {
    Ok(length_from_basic_layout(font, text)? as f32 / 64.0)
}

pub(crate) fn getbbox(font: &Font, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
    bbox_from_run(font, text)
}

pub(crate) fn getbbox_binary(font: &Font, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
    bbox_from_run_with_flags(font, text, TGT_MONO)
}

pub(crate) fn getmask(font: &Font, text: &str) -> Result<(u32, u32, Vec<u8>), PilError> {
    mask_from_run_with_start(font, text, TGT_NORM, (0.0, 0.0))
}

/// Render a Pillow-compatible mask together with its BASIC-layout offset.
pub(crate) fn getmask2(
    font: &Font,
    text: &str,
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    getmask2_with_start(font, text, (0.0, 0.0))
}

/// Render a Pillow-compatible mask with a fractional raster start.
///
/// Pillow applies `start` to the mask canvas and glyph origin while leaving
/// the returned BASIC-layout offset unchanged.
pub(crate) fn getmask2_with_start(
    font: &Font,
    text: &str,
    start: (f64, f64),
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    let (width, height, pixels) = mask_from_run_with_start(font, text, TGT_NORM, start)?;
    let bbox = getbbox(font, text)?;
    Ok((width, height, pixels, (bbox.0, bbox.1)))
}

pub(crate) fn render_text(
    font: &Font,
    text: &str,
    fill: (u8, u8, u8, u8),
    _spacing: f32,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    pack_rgba(getmask(font, text)?, fill)
}

pub(crate) fn render_text_binary(
    font: &Font,
    text: &str,
    fill: (u8, u8, u8, u8),
    spacing: f32,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    let _ = spacing;
    pack_rgba(
        mask_from_run_with_start(font, text, TGT_MONO, (0.0, 0.0))?,
        fill,
    )
}

fn pack_rgba(
    (w, h, mask): (u32, u32, Vec<u8>),
    fill: (u8, u8, u8, u8),
) -> Result<(u32, u32, Vec<u8>), PilError> {
    if w == 0 || h == 0 {
        return Ok((w, h, mask));
    }
    let len = match (w as usize)
        .checked_mul(h as usize)
        .and_then(|v| v.checked_mul(4))
    {
        Some(v) => v,
        None => {
            return Err(PilError::DimensionError(
                "text RGBA mask dimensions overflow".into(),
            ));
        }
    };
    let mut canvas = vec![0u8; len];
    for (i, cov) in mask.into_iter().enumerate() {
        if cov == 0 {
            continue;
        }
        let o = i * 4;
        canvas[o] = fill.0;
        canvas[o + 1] = fill.1;
        canvas[o + 2] = fill.2;
        canvas[o + 3] = cov;
    }
    Ok((w, h, canvas))
}

// ── FFI helpers ──────────────────────────────────────────────────────

const KERN_DEFAULT: u32 = 0; // FT_KERNING_DEFAULT as u32
const RDR: i32 = 4; // FT_LOAD_RENDER
const TGT_NORM: i32 = 0; // FT_LOAD_TARGET_NORMAL
const TGT_MONO: i32 = 2 << 16; // FT_LOAD_TARGET_MONO

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

fn length_from_basic_layout(ttf: &Font, text: &str) -> Result<i32, PilError> {
    let face = &ttf.engine.face;
    let mut total = 0i32;
    let mut prev: Option<u32> = None;

    for ch in text.chars() {
        let g = gid(face, ch);
        // Pillow 12.2.0 `text_layout_fallback` uses `FT_LOAD_DEFAULT`.
        // Its hinted `horiAdvance` values are integral pixels for BASIC layout.
        let slot = ffi::FT_Load_Glyph(face, g, 0).map_err(ft_error_to_pil)?;
        validate_advance_26_6(slot.advance.x)?;
        if let Some(p) = prev.filter(|p| *p != 0 && g != 0) {
            total = total.saturating_add(basic_layout_kern(face, p, g));
        }
        total = total.saturating_add(slot.metrics.horiAdvance as i32);
        prev = Some(g);
    }

    Ok(total)
}

const MAX_ADVANCE_26_6_EXCLUSIVE: i64 = 0x8000 * 64;

fn validate_advance_26_6(advance: i64) -> Result<(), PilError> {
    if advance >= MAX_ADVANCE_26_6_EXCLUSIVE || advance <= -MAX_ADVANCE_26_6_EXCLUSIVE {
        Err(PilError::OsError("invalid argument".into()))
    } else {
        Ok(())
    }
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
fn glyph_run(ttf: &Font, text: &str, load_flags: i32) -> Result<Option<GlyphRun>, PilError> {
    if text.is_empty() {
        return Ok(Some(GlyphRun {
            glyphs: vec![],
            max_pen: 0,
        }));
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
        let slot = ffi::FT_Load_Glyph(face, g, load_flags).map_err(ft_error_to_pil)?;
        validate_advance_26_6(slot.advance.x)?;
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
    Ok(Some(GlyphRun {
        glyphs: out,
        max_pen,
    }))
}

fn bbox_from_run(ttf: &Font, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
    bbox_from_run_with_flags(ttf, text, TGT_NORM)
}

fn bbox_from_run_with_flags(
    ttf: &Font,
    text: &str,
    load_flags: i32,
) -> Result<(i32, i32, i32, i32), PilError> {
    let Some(run) = glyph_run(ttf, text, load_flags)? else {
        return Ok((0, 0, 0, 0));
    };
    if run.glyphs.is_empty() {
        return Ok((0, 0, 0, 0));
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
    Ok((x_min, y_anchor - y_max, x_max, y_anchor - y_min))
}

// ── Mask render ──────────────────────────────────────────────────────

fn mask_from_run_with_start(
    ttf: &Font,
    text: &str,
    load_flags: i32,
    start: (f64, f64),
) -> Result<(u32, u32, Vec<u8>), PilError> {
    if text.is_empty() {
        return Ok((0, 0, vec![]));
    }
    // Pillow 12.2.0 `_imagingft.c` uses FT_LOAD_TARGET_MONO consistently
    // during BASIC layout, bbox calculation, and both render passes for
    // `fontmode="1"`. Thresholding the normal grayscale mask is not
    // equivalent: monochrome hinting changes advances and glyph geometry.
    let bbox = bbox_from_run_with_flags(ttf, text, load_flags)?;
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` expands the allocated
    // mask by ceil(start), then rounds the shifted 26.6 pen origin.
    let start_width = start.0.ceil() as i32;
    let start_height = start.1.ceil() as i32;
    let base_w = bbox.2 - bbox.0;
    let base_h = bbox.3 - bbox.1;
    let adjusted_w = base_w.saturating_add(start_width);
    let adjusted_h = base_h.saturating_add(start_height);
    if (base_w > 0 && adjusted_w <= 0) || (base_h > 0 && adjusted_h <= 0) {
        return Err(PilError::ValueError("bad image size".into()));
    }
    let w = adjusted_w.max(0) as u32;
    let h = adjusted_h.max(0) as u32;
    let wu = w as usize;
    let hu = h as usize;
    let canvas_len = wu
        .checked_mul(hu)
        .ok_or_else(|| PilError::DimensionError("text mask dimensions overflow".into()))?;
    let mut canvas = vec![0u8; canvas_len];
    if w == 0 || h == 0 {
        return Ok((w, h, canvas));
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
        let layout_slot = ffi::FT_Load_Glyph(face, g, load_flags).map_err(ft_error_to_pil)?;
        if let Some(p) = prev.filter(|p| *p != 0 && g != 0) {
            pen = pen.saturating_add(basic_layout_kern(face, p, g));
        }

        let slot = match ffi::FT_Load_Glyph(face, g, RDR | load_flags) {
            Ok(s) => s,
            Err(_) => match ffi::FT_Load_Glyph(face, g, load_flags) {
                Ok(s) => s,
                Err(error) => return Err(ft_error_to_pil(error)),
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

    let x_origin = ((f64::from(-x_min) + start.0) * 64.0).round() as i32;
    let y_origin = ((f64::from(-y_max) - start.1) * 64.0).round() as i32;

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
        if dx >= wu as i32 || dy >= hu as i32 {
            continue;
        }
        let source_x = if dx < 0 { (-dx) as usize } else { 0 };
        let target_x = dx.max(0) as usize;
        if source_x >= sx {
            continue;
        }
        let cw = (sx - source_x).min(wu - target_x);
        if cw == 0 {
            continue;
        }
        let source_y = if dy < 0 { (-dy) as usize } else { 0 };
        if source_y >= sy {
            continue;
        }
        for row in source_y..sy {
            let target_y = dy + row as i32;
            if target_y < 0 {
                continue;
            }
            if target_y >= hu as i32 {
                break;
            }
            let dst = target_y as usize * wu + target_x;
            if let Some(dr) = canvas.get_mut(dst..dst + cw) {
                for (column, dc) in dr.iter_mut().enumerate() {
                    let Some(sc) = bitmap_coverage(bm, row, source_x + column) else {
                        continue;
                    };
                    if sc > 0 {
                        let under = crate::color::muldiv255(u32::from(*dc), u32::from(255 - sc));
                        *dc = sc.saturating_add(under as u8);
                    }
                }
            }
        }
    }
    Ok((w, h, canvas))
}

fn bitmap_coverage(bitmap: &ffi::FT_Bitmap, row: usize, column: usize) -> Option<u8> {
    let rows = bitmap.rows as usize;
    let pitch = usize::try_from(bitmap.pitch.unsigned_abs()).ok()?;
    let storage_row = if bitmap.pitch < 0 {
        rows.checked_sub(row + 1)?
    } else {
        row
    };
    let row_start = storage_row.checked_mul(pitch)?;
    match bitmap.pixel_mode {
        ffi::FT_PIXEL_MODE_MONO => {
            let byte = *bitmap.buffer.get(row_start + column / 8)?;
            Some(if byte & (0x80 >> (column & 7)) != 0 {
                255
            } else {
                0
            })
        }
        ffi::FT_PIXEL_MODE_GRAY => bitmap.buffer.get(row_start + column).copied(),
        _ => None,
    }
}
