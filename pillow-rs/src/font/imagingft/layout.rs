use super::*;

pub(crate) fn getmetrics(font: &FreeTypeFont) -> (u32, u32) {
    (
        pixel(native_long_to_i64(font.engine.metrics.ascender)) as u32,
        (-pixel(native_long_to_i64(font.engine.metrics.descender))) as u32,
    )
}

/// Return whether the loaded face exposes OpenType or Type 1 variation axes.
pub(crate) fn getlength(font: &FreeTypeFont, text: &str) -> Result<f32, PilError> {
    validate_text_length(text)?;
    Ok(length_from_basic_layout_with_flags(font, text, 0)? as f32 / 64.0)
}

/// Return default-layout advance and bounds from one glyph pass.
pub(crate) fn getlength_and_bbox(
    font: &FreeTypeFont,
    text: &str,
) -> Result<(f32, (i32, i32, i32, i32)), PilError> {
    validate_text_length(text)?;
    let run = glyph_run(font, text, TGT_NORM)?;
    let length = run.final_pen as f32 / 64.0;
    let bbox = bbox_from_glyph_run(font, &run)?;
    Ok((length, bbox))
}

pub(crate) fn native_getlength_26dot6(font: &FreeTypeFont, text: &str) -> Result<i32, PilError> {
    validate_text_length(text)?;
    length_from_basic_layout_with_flags(font, text, 0)
}

pub(crate) fn native_getsize(
    font: &FreeTypeFont,
    text: &str,
) -> Result<((i32, i32), (i32, i32)), PilError> {
    let (left, top, right, bottom) = getbbox(font, text)?;
    Ok(((right - left, bottom - top), (left, top)))
}

pub(crate) fn native_face_info(font: &FreeTypeFont) -> ImageFontFaceInfo<'_> {
    let metrics = font.engine.metrics;
    ImageFontFaceInfo {
        family: font.engine.family_name.as_deref(),
        style: font.engine.style_name.as_deref(),
        ascent: pixel(native_long_to_i64(metrics.ascender)) as u32,
        descent: (-pixel(native_long_to_i64(metrics.descender))) as u32,
        height: pixel(native_long_to_i64(metrics.height)) as u32,
        x_ppem: u32::from(metrics.x_ppem),
        y_ppem: u32::from(metrics.y_ppem),
        glyphs: native_long_to_i64(font.engine.face.num_glyphs),
        format: ffi::FT_Get_Font_Format(Some(&font.engine.face)).unwrap_or("Unknown"),
        is_scalable: native_long_to_i64(font.engine.face.face_flags) & ffi::FT_FACE_FLAG_SCALABLE
            != 0,
    }
}

pub(crate) fn getlength_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<f32, PilError> {
    validate_text_length(text)?;
    validate_basic_layout_options(options)?;
    if options.mode.as_deref() != Some("1") {
        return getlength(font, text);
    }
    Ok(length_from_basic_layout_with_flags(font, text, text_load_flags(options))? as f32 / 64.0)
}

pub(crate) fn getbbox(font: &FreeTypeFont, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
    validate_text_length(text)?;
    bbox_from_run(font, text)
}

pub(crate) fn getbbox_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(f32, f32, f32, f32), PilError> {
    validate_text_length(text)?;
    validate_basic_layout_options(options)?;
    let bbox = bbox_from_run_with_flags(font, text, text_load_flags(options))?;
    let (left, top, right, bottom) = anchored_bbox(
        font,
        bbox,
        options.anchor.as_deref(),
        options.anchor_invalid_length_error,
    )?;
    let stroke = options.stroke_width;
    Ok((left - stroke, top - stroke, right + stroke, bottom + stroke))
}

#[cfg(feature = "test-api")]
pub(crate) fn getbbox_binary(
    font: &FreeTypeFont,
    text: &str,
) -> Result<(i32, i32, i32, i32), PilError> {
    bbox_from_run_with_flags(font, text, TGT_MONO)
}

pub(super) fn validate_basic_layout_options(
    options: &ImageFontTextOptions,
) -> Result<(), PilError> {
    if options.direction.is_some()
        || options.features.is_some()
        || options.features_invalid
        || options.language.is_some()
    {
        return Err(PilError::unsupported_libraqm());
    }
    Ok(())
}

pub(super) fn text_load_flags(options: &ImageFontTextOptions) -> i32 {
    let target = if options.mode.as_deref() == Some("1") {
        TGT_MONO
    } else {
        TGT_NORM
    };
    if options.uses_color_mask() {
        target | ffi::FT_LOAD_COLOR
    } else {
        target
    }
}

pub(super) fn validate_text_length(text: &str) -> Result<(), PilError> {
    if text.chars().count() > MAX_STRING_LENGTH {
        return Err(PilError::ValueError("too many characters in string".into()));
    }
    Ok(())
}

pub(super) fn anchored_bbox(
    font: &FreeTypeFont,
    (left, top, right, bottom): (i32, i32, i32, i32),
    anchor: Option<&str>,
    invalid_length_uses_legacy_error: bool,
) -> Result<(f32, f32, f32, f32), PilError> {
    let Some(anchor) = anchor else {
        return Ok((left as f32, top as f32, right as f32, bottom as f32));
    };
    if anchor.len() != 2 {
        if invalid_length_uses_legacy_error {
            return Err(PilError::ValueError(
                "anchor must be a 2 character string".into(),
            ));
        }
        return Err(bad_anchor_error(anchor));
    }
    let width = right - left;
    let ascent = pixel(native_long_to_i64(font.engine.metrics.ascender));
    let descent = -pixel(native_long_to_i64(font.engine.metrics.descender));
    let x_shift = match anchor.as_bytes()[0] {
        b'l' => 0,
        b'm' => -((width + 1) / 2),
        b'r' => -width,
        _ => {
            return Err(bad_anchor_error(anchor));
        }
    };
    let y_shift = match anchor.as_bytes()[1] {
        b'a' => 0,
        b't' => -top,
        b'm' => -((ascent + descent) / 2),
        b's' => -ascent,
        b'b' => -bottom,
        b'd' => -(ascent + descent),
        _ => {
            return Err(bad_anchor_error(anchor));
        }
    };
    Ok((
        (left + x_shift) as f32,
        (top + y_shift) as f32,
        (right + x_shift) as f32,
        (bottom + y_shift) as f32,
    ))
}

fn bad_anchor_error(anchor: &str) -> PilError {
    PilError::ValueError("bad anchor specified: ".to_owned() + anchor)
}

const KERN_DEFAULT: u32 = 0; // FT_KERNING_DEFAULT as u32
pub(super) const RDR: i32 = 4; // FT_LOAD_RENDER
pub(super) const TGT_NORM: i32 = 0; // FT_LOAD_TARGET_NORMAL
pub(super) const TGT_MONO: i32 = 2 << 16; // FT_LOAD_TARGET_MONO

// `_imagingft.c` receives native FreeType `long` records: LLP64 Windows and
// 32-bit targets use 32 bits, while LP64 hosts use 64. Widen signed values at
// this boundary before the existing Pillow rounding and advance arithmetic.
#[allow(clippy::useless_conversion)] // FT_Long is already i64 on LP64 hosts.
pub(super) fn native_long_to_i64(value: ffi::FT_Long) -> i64 {
    i64::from(value)
}

fn gid(face: &ffi::FT_Face, ch: char) -> u32 {
    ffi::FT_Get_Char_Index(face, ch as ffi::FT_ULong)
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

pub(super) fn round26(v: i32) -> i32 {
    pixel(i64::from(v))
}

pub(super) fn pixel(x: i64) -> i32 {
    (((x + 32) & -64) >> 6) as i32
}

fn floor26(x: i64) -> i32 {
    ((x & -64) >> 6) as i32
}

fn ceil26(x: i64) -> i32 {
    (((x + 63) & -64) >> 6) as i32
}

fn length_from_basic_layout_with_flags(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
) -> Result<i32, PilError> {
    if text.is_empty() {
        return Ok(0);
    }

    let mut pen = 0i32;
    let mut prev: Option<u32> = None;
    for ch in text.chars() {
        let glyph_index = basic_layout_glyph_index(ttf, ch);
        let advance = basic_layout_advance_cached(ttf, glyph_index, load_flags)?;
        if let Some(previous) = prev.filter(|previous| *previous != 0 && glyph_index != 0) {
            pen = pen.saturating_add(basic_layout_kern_cached(ttf, previous, glyph_index));
        }
        pen = pen.saturating_add(advance);
        prev = Some(glyph_index);
    }

    Ok(pen)
}

fn basic_layout_glyph_index(ttf: &FreeTypeFont, ch: char) -> u32 {
    if let Some(glyph_index) = ttf
        .engine
        .basic_layout_cache
        .borrow()
        .glyph_indices
        .get(&ch)
        .copied()
    {
        return glyph_index;
    }

    let glyph_index = gid(&ttf.engine.face, ch);
    insert_bounded(
        &mut ttf.engine.basic_layout_cache.borrow_mut().glyph_indices,
        ch,
        glyph_index,
    );
    glyph_index
}

fn basic_layout_advance_cached(
    ttf: &FreeTypeFont,
    glyph_index: u32,
    load_flags: i32,
) -> Result<i32, PilError> {
    let key = (glyph_index, load_flags);
    if let Some(advance) = ttf
        .engine
        .basic_layout_cache
        .borrow()
        .advances
        .get(&key)
        .copied()
    {
        return Ok(advance);
    }

    let advance = basic_layout_advance(&ttf.engine.face, glyph_index, load_flags)?;
    insert_bounded(
        &mut ttf.engine.basic_layout_cache.borrow_mut().advances,
        key,
        advance,
    );
    Ok(advance)
}

fn basic_layout_kern_cached(ttf: &FreeTypeFont, left: u32, right: u32) -> i32 {
    let key = (left, right);
    if let Some(kerning) = ttf
        .engine
        .basic_layout_cache
        .borrow()
        .kernings
        .get(&key)
        .copied()
    {
        return kerning;
    }

    let kerning = basic_layout_kern(&ttf.engine.face, left, right);
    insert_bounded(
        &mut ttf.engine.basic_layout_cache.borrow_mut().kernings,
        key,
        kerning,
    );
    kerning
}

fn basic_layout_advance(
    face: &ffi::FT_Face,
    glyph_index: u32,
    load_flags: i32,
) -> Result<i32, PilError> {
    // Length needs only the hinted advance. FT_Load_Glyph also copies the full
    // outline into an FFI glyph slot, which the BASIC layout path discards.
    // FT_Get_Advance follows the same load route without materializing that
    // slot; its scaled 16.16 result is the glyph loader's 26.6 advance shifted
    // by ten bits.
    let advance = ffi::FT_Get_Advance(face, glyph_index, load_flags).map_err(ft_error_to_pil)?;
    let advance = native_long_to_i64(advance);
    let advance = if load_flags & ffi::FT_LOAD_NO_SCALE != 0 {
        advance
    } else {
        advance >> 10
    };
    let advance =
        i32::try_from(advance).map_err(|_| PilError::OsError("invalid argument".into()))?;
    validate_advance_26_6(i64::from(advance))?;
    Ok(advance)
}

fn validate_advance_26_6(advance: i64) -> Result<(), PilError> {
    if advance.unsigned_abs() >= (0x8000 * 64) as u64 {
        Err(PilError::OsError("invalid argument".into()))
    } else {
        Ok(())
    }
}

// ── Glyph run (no render, for metrics/advance/bbox) ─────────────────

pub(super) struct GlyphRun {
    pub(super) glyphs: Vec<RunGlyph>,
    pub(super) final_pen: i32, // final pen position in 26.6
    pub(super) max_pen: i32,   // maximum pen position in 26.6
}

pub(super) struct RunGlyph {
    pub(super) glyph_index: u32,
    pub(super) pen_before: i32,
    pub(super) advance: i32,
    pub(super) layout_cbox: ffi::FT_BBox,
}

/// Load each glyph WITHOUT rendering, collect advances and metrics.
pub(super) fn glyph_run(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
) -> Result<GlyphRun, PilError> {
    glyph_run_with_slot_visitor(ttf, text, load_flags, |slot, _, _| {
        drop(slot);
        Ok(())
    })
}

pub(super) fn glyph_run_with_slot_visitor(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
    mut visit_slot: impl FnMut(ffi::FT_GlyphSlot, u32, i32) -> Result<(), PilError>,
) -> Result<GlyphRun, PilError> {
    if text.is_empty() {
        return Ok(GlyphRun {
            glyphs: vec![],
            final_pen: 0,
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
        let slot = ffi::FT_Load_Glyph(face, g, load_flags).map_err(ft_error_to_pil)?;
        validate_advance_26_6(native_long_to_i64(slot.advance.x))?;
        if let Some(p) = prev.filter(|p| *p != 0 && g != 0) {
            pen = pen.saturating_add(basic_layout_kern(face, p, g));
        }

        let pen_before = pen;
        let adv = slot.metrics.horiAdvance as i32;
        let layout_cbox = glyph_layout_cbox(&slot);
        visit_slot(slot, g, pen_before)?;

        out.push(RunGlyph {
            glyph_index: g,
            pen_before,
            advance: adv,
            layout_cbox,
        });

        pen = pen.saturating_add(adv);
        max_pen = max_pen.max(pen);
        prev = Some(g);
    }
    Ok(GlyphRun {
        glyphs: out,
        final_pen: pen,
        max_pen,
    })
}

fn bbox_from_run(ttf: &FreeTypeFont, text: &str) -> Result<(i32, i32, i32, i32), PilError> {
    bbox_from_run_with_flags(ttf, text, TGT_NORM)
}

pub(super) fn bbox_from_run_with_flags(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
) -> Result<(i32, i32, i32, i32), PilError> {
    let run = glyph_run(ttf, text, load_flags)?;
    bbox_from_glyph_run(ttf, &run)
}

pub(super) fn bbox_from_glyph_run(
    ttf: &FreeTypeFont,
    run: &GlyphRun,
) -> Result<(i32, i32, i32, i32), PilError> {
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

        let cbox = g.layout_cbox;
        let glyph_x_min = px + floor26(native_long_to_i64(cbox.xMin));
        let glyph_x_max = px + ceil26(native_long_to_i64(cbox.xMax));
        let glyph_y_min = floor26(native_long_to_i64(cbox.yMin));
        let glyph_y_max = ceil26(native_long_to_i64(cbox.yMax));

        x_min = x_min.min(glyph_x_min);
        x_max = x_max.max(glyph_x_max);
        y_min = y_min.min(glyph_y_min);
        y_max = y_max.max(glyph_y_max);
    }

    x_max = x_max.max(round26(run.max_pen));
    let y_anchor = pixel(native_long_to_i64(ttf.engine.metrics.ascender));
    Ok((x_min, y_anchor - y_max, x_max, y_anchor - y_min))
}

fn glyph_layout_cbox(slot: &ffi::FT_GlyphSlot) -> ffi::FT_BBox {
    if let Some(bitmap) = &slot.bitmap {
        // Pillow 12.2.0 `_imagingft.c::bounding_box_and_anchors` sizes the
        // public text mask from the loaded glyph's bitmap extents when a glyph
        // is an embedded bitmap strike. Bitmap-only SBIT glyph slots have no
        // outline cbox, so using `outline_cbox` here collapses the mask even
        // though the render pass has pixels.
        // FreeType `ftglyph.c::ft_bitmap_glyph_bbox` multiplies C int
        // bearings and unsigned dimensions before converting to FT_Pos.
        let x_min = slot.bitmap_left.wrapping_mul(64) as ffi::FT_Pos;
        let x_max = x_min.wrapping_add(bitmap.width.wrapping_mul(64) as ffi::FT_Pos);
        let y_max = slot.bitmap_top.wrapping_mul(64) as ffi::FT_Pos;
        let y_min = y_max.wrapping_sub(bitmap.rows.wrapping_mul(64) as ffi::FT_Pos);
        return ffi::FT_BBox {
            xMin: x_min,
            yMin: y_min,
            xMax: x_max,
            yMax: y_max,
        };
    }

    slot.outline_cbox
}

// ── Mask render ──────────────────────────────────────────────────────
