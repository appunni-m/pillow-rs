#[cfg(feature = "test-api")]
use super::layout::TGT_MONO;
use super::layout::{
    GlyphRun, RDR, RunGlyph, TGT_NORM, anchored_bbox, bbox_from_glyph_run,
    bbox_from_run_with_flags, glyph_run, glyph_run_with_slot_visitor, pixel, round26,
    text_load_flags, validate_basic_layout_options, validate_text_length,
};
use super::*;

struct RenderedBitmap {
    pen_before: i32,
    bitmap_left: i32,
    bitmap_top: i32,
    bitmap: ffi::FT_Bitmap,
}

pub(crate) fn native_render(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<FontMaskAndOffset, PilError> {
    getmask2_with_options(font, text, options)
}

pub(crate) fn getmask(font: &FreeTypeFont, text: &str) -> Result<(u32, u32, Vec<u8>), PilError> {
    validate_text_length(text)?;
    mask_from_run_with_start(font, text, TGT_NORM, (0.0, 0.0))
}

pub(crate) fn getmask_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    let (width, height, pixels, _) = getmask2_with_options(font, text, options)?;
    Ok((width, height, pixels))
}

/// Render a Pillow-compatible mask together with its BASIC-layout offset.
pub(crate) fn getmask2(font: &FreeTypeFont, text: &str) -> Result<FontMaskAndOffset, PilError> {
    validate_text_length(text)?;
    getmask2_with_start(font, text, (0.0, 0.0))
}

/// Render a Pillow-compatible mask with a fractional raster start.
///
/// Pillow applies `start` to the mask canvas and glyph origin while leaving
/// the returned BASIC-layout offset unchanged.
pub(crate) fn getmask2_with_start(
    font: &FreeTypeFont,
    text: &str,
    start: (f64, f64),
) -> Result<FontMaskAndOffset, PilError> {
    validate_text_length(text)?;
    let (width, height, pixels, bbox) =
        mask_from_run_with_start_and_bbox(font, text, TGT_NORM, start)?;
    Ok((width, height, pixels, (bbox.0, bbox.1)))
}

pub(crate) fn getmask2_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<FontMaskAndOffset, PilError> {
    validate_text_length(text)?;
    validate_basic_layout_options(options)?;
    if options.start_invalid {
        return Err(PilError::TypeError(
            "render() argument 11 must be 2-item sequence, not float".into(),
        ));
    }
    if options_are_default_mask_options(options) {
        return getmask2(font, text);
    }
    let load_flags = text_load_flags(options);
    let _pillow_ignored_public_args = (options.ink, options.has_args, options.has_kwargs);
    let foreground = rgba_foreground_from_ink(options.ink);
    let start = options.start.unwrap_or((0.0, 0.0));
    let (width, height, pixels) = if options.stroke_width != 0.0 {
        let (width, height, mask) = stroked_mask_from_run_with_start(
            font,
            text,
            load_flags,
            start,
            options.stroke_width,
            options.stroke_filled,
        )?;
        if options.uses_color_mask() {
            (width, height, grayscale_mask_to_rgba(mask, foreground))
        } else {
            (width, height, mask)
        }
    } else if options.uses_color_mask() {
        color_mask_from_run_with_start(font, text, load_flags, start, foreground)?
    } else {
        mask_from_run_with_start(font, text, load_flags, start)?
    };
    let bbox = bbox_from_run_with_flags(
        font,
        text,
        if options.stroke_width != 0.0 {
            load_flags | ffi::FT_LOAD_NO_BITMAP
        } else {
            load_flags
        },
    )?;
    let (left, top, _, _) = anchored_bbox(
        font,
        bbox,
        options.anchor.as_deref(),
        options.anchor_invalid_length_error,
    )?;
    let left = left - options.stroke_width;
    let top = top - options.stroke_width;
    let offset = if options.stroke_width != 0.0 {
        let top = if top < 0.0 { top.floor() } else { top.ceil() };
        (left.floor() as i32, top as i32)
    } else {
        (left as i32, top as i32)
    };
    Ok((width, height, pixels, offset))
}

fn options_are_default_mask_options(options: &ImageFontTextOptions) -> bool {
    options.uses_default_mask()
}

#[cfg(feature = "test-api")]
pub(crate) fn render_text_binary(
    font: &FreeTypeFont,
    text: &str,
    fill: (u8, u8, u8, u8),
    spacing: f32,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    validate_text_length(text)?;
    let _ = spacing;
    pack_rgba(
        mask_from_run_with_start(font, text, TGT_MONO, (0.0, 0.0))?,
        fill,
    )
}

#[cfg(feature = "test-api")]
fn pack_rgba(
    (w, h, mask): (u32, u32, Vec<u8>),
    fill: (u8, u8, u8, u8),
) -> Result<(u32, u32, Vec<u8>), PilError> {
    if w == 0 || h == 0 {
        return Ok((w, h, mask));
    }
    debug_assert_eq!(
        mask.len(),
        (w as usize) * (h as usize),
        "text mask dimensions are produced by mask_from_run_with_start"
    );
    let len = mask.len() * 4;
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

fn mask_from_run_with_start(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
    start: (f64, f64),
) -> Result<(u32, u32, Vec<u8>), PilError> {
    let (width, height, pixels, _) =
        mask_from_run_with_start_and_bbox(ttf, text, load_flags, start)?;
    Ok((width, height, pixels))
}

fn mask_from_run_with_start_and_bbox(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
    start: (f64, f64),
) -> Result<FontMaskAndBounds, PilError> {
    let mut loaded_slots = Vec::new();
    let run = if load_flags == TGT_NORM {
        glyph_run_with_slot_visitor(ttf, text, load_flags, |slot, _, _| {
            loaded_slots.push(slot);
            Ok(())
        })?
    } else {
        glyph_run(ttf, text, load_flags)?
    };
    if run.glyphs.is_empty() {
        return Ok((0, 0, vec![], (0, 0, 0, 0)));
    }
    // Pillow 12.2.0 `_imagingft.c` uses FT_LOAD_TARGET_MONO consistently
    // during BASIC layout, bbox calculation, and both render passes for
    // `fontmode="1"`. Thresholding the normal grayscale mask is not
    // equivalent: monochrome hinting changes advances and glyph geometry.
    let bbox = bbox_from_glyph_run(ttf, &run)?;
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` expands the allocated
    // mask by ceil(start), then rounds the shifted 26.6 pen origin.
    let start_width = start.0.ceil() as i32;
    let start_height = start.1.ceil() as i32;
    let base_w = bbox.2 - bbox.0;
    let base_h = bbox.3 - bbox.1;
    let adjusted_w = base_w.saturating_add(start_width);
    let adjusted_h = base_h.saturating_add(start_height);
    if positive_dimension_collapsed(base_w, adjusted_w)
        || positive_dimension_collapsed(base_h, adjusted_h)
    {
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
    if canvas_len == 0 {
        return Ok((w, h, canvas, bbox));
    }

    let face = &ttf.engine.face;
    let mut rendered = Vec::new();
    let mut x_min = 0;
    let mut y_max = 0;

    let reuse_loaded_slots = load_flags == TGT_NORM
        && loaded_slots
            .iter()
            .all(|slot| slot.format == ffi::FT_GLYPH_FORMAT_OUTLINE);
    if reuse_loaded_slots {
        for (slot, glyph) in loaded_slots.into_iter().zip(&run.glyphs) {
            let slot =
                ffi::FT_Render_Glyph(slot, ffi::FT_RENDER_MODE_NORMAL).map_err(ft_error_to_pil)?;
            x_min = x_min.min(round26(glyph.pen_before) + slot.bitmap_left as i32);
            y_max = y_max.max(slot.bitmap_top as i32);
            if let Some(bitmap) = slot.bitmap {
                rendered.push(RenderedBitmap {
                    pen_before: glyph.pen_before,
                    bitmap_left: slot.bitmap_left as i32,
                    bitmap_top: slot.bitmap_top as i32,
                    bitmap,
                });
            }
        }
    } else {
        for glyph in &run.glyphs {
            let slot = ffi::FT_Load_Glyph(face, glyph.glyph_index, RDR | load_flags)
                .map_err(ft_error_to_pil)?;

            let px = round26(glyph.pen_before);
            x_min = x_min.min(px + slot.bitmap_left as i32);
            y_max = y_max.max(slot.bitmap_top as i32);
            if let Some(bitmap) = slot.bitmap {
                rendered.push(RenderedBitmap {
                    pen_before: glyph.pen_before,
                    bitmap_left: slot.bitmap_left as i32,
                    bitmap_top: slot.bitmap_top as i32,
                    bitmap,
                });
            }
        }
    }

    let x_origin = ((f64::from(-x_min) + start.0) * 64.0).round() as i32;
    let y_origin = ((f64::from(-y_max) - start.1) * 64.0).round() as i32;
    paste_rendered_bitmaps(&rendered, &mut canvas, w, h, x_origin, y_origin);
    Ok((w, h, canvas, bbox))
}

/// Render a mask while preserving embedded BGRA glyph pixels as RGBA.
///
/// `fontdone` follows FreeType's `FT_LOAD_COLOR` contract: color bitmap
/// channels are stored as premultiplied sRGB BGRA and must be unpremultiplied
/// before Pillow receives the RGBA mask. Outline and monochrome bitmap glyphs
/// use the low three bytes of Pillow's `ink` value for RGB, with their
/// rendered coverage in alpha.
fn color_mask_from_run_with_start(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
    start: (f64, f64),
    foreground: (u8, u8, u8),
) -> Result<(u32, u32, Vec<u8>), PilError> {
    // COLR foreground-index paints resolve through the face's foreground
    // color. Set it before the first glyph load so fontdone applies the same
    // state to every glyph in this render run; embedded BGRA strikes remain
    // unchanged because they do not consult the face foreground state.
    check_ft_error(ffi::FT_Palette_Set_Foreground_Color(
        Some(&ttf.engine.face),
        ffi::FT_Color {
            blue: foreground.2,
            green: foreground.1,
            red: foreground.0,
            alpha: 255,
        },
    ))?;
    let run = glyph_run(ttf, text, load_flags)?;
    if run.glyphs.is_empty() {
        return Ok((0, 0, vec![]));
    }
    let bbox = bbox_from_glyph_run(ttf, &run)?;
    let start_width = start.0.ceil() as i32;
    let start_height = start.1.ceil() as i32;
    let base_w = bbox.2 - bbox.0;
    let base_h = bbox.3 - bbox.1;
    let adjusted_w = base_w.saturating_add(start_width);
    let adjusted_h = base_h.saturating_add(start_height);
    if positive_dimension_collapsed(base_w, adjusted_w)
        || positive_dimension_collapsed(base_h, adjusted_h)
    {
        return Err(PilError::ValueError("bad image size".into()));
    }
    let w = adjusted_w.max(0) as u32;
    let h = adjusted_h.max(0) as u32;
    let wu = w as usize;
    let hu = h as usize;
    let pixels = wu
        .checked_mul(hu)
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| PilError::DimensionError("text mask dimensions overflow".into()))?;
    let mut canvas = vec![0u8; pixels];
    if pixels == 0 {
        return Ok((w, h, canvas));
    }

    let face = &ttf.engine.face;
    let mut rendered = Vec::new();
    let mut x_min = 0;
    let mut y_max = 0;
    for glyph in &run.glyphs {
        let slot = ffi::FT_Load_Glyph(face, glyph.glyph_index, RDR | load_flags)
            .map_err(ft_error_to_pil)?;
        let px = round26(glyph.pen_before);
        x_min = x_min.min(px + slot.bitmap_left as i32);
        y_max = y_max.max(slot.bitmap_top as i32);
        if let Some(bitmap) = slot.bitmap {
            rendered.push(RenderedBitmap {
                pen_before: glyph.pen_before,
                bitmap_left: slot.bitmap_left as i32,
                bitmap_top: slot.bitmap_top as i32,
                bitmap,
            });
        }
    }

    let x_origin = ((f64::from(-x_min) + start.0) * 64.0).round() as i32;
    let y_origin = ((f64::from(-y_max) - start.1) * 64.0).round() as i32;
    paste_rendered_bitmaps_rgba(&rendered, &mut canvas, w, h, x_origin, y_origin, foreground);
    Ok((w, h, canvas))
}

fn grayscale_mask_to_rgba(mask: Vec<u8>, foreground: (u8, u8, u8)) -> Vec<u8> {
    let mut pixels = vec![0u8; mask.len() * 4];
    for (index, coverage) in mask.into_iter().enumerate() {
        if coverage > 0 {
            pixels[index * 4] = foreground.0;
            pixels[index * 4 + 1] = foreground.1;
            pixels[index * 4 + 2] = foreground.2;
        }
        pixels[index * 4 + 3] = coverage;
    }
    pixels
}

fn stroked_mask_from_run_with_start(
    ttf: &FreeTypeFont,
    text: &str,
    load_flags: i32,
    start: (f64, f64),
    stroke_width: f32,
    stroke_filled: bool,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    if text.is_empty() {
        let side = (stroke_width * 2.0).ceil() as i32;
        if side < 0 {
            return Err(PilError::ValueError("bad image size".into()));
        }
        let side = side as u32;
        return Ok((side, side, vec![0; side.saturating_mul(side) as usize]));
    }

    let run = glyph_run(ttf, text, load_flags)?;
    let face = &ttf.engine.face;
    let stroker = new_stroker(&ttf.engine.library)?;
    let radius = (stroke_width * 64.0).round() as ffi::FT_Fixed;
    ffi::FT_Stroker_Set(
        stroker.handle,
        radius,
        ffi::FT_STROKER_LINECAP_ROUND as ffi::FT_Int,
        ffi::FT_STROKER_LINEJOIN_ROUND as ffi::FT_Int,
        0,
    );
    let mut rendered = Vec::new();
    let mut render_x_min = 0;
    let mut first_y_max = 0;

    // Pillow 12.2.0 `_imagingft.c::font_render_impl` loads with
    // `FT_LOAD_NO_BITMAP` whenever `stroke_width != 0`, so scalable faces
    // render stroked outlines instead of embedded strikes.  Bitmap-only
    // CBLC/CBDT faces still return their strike (or a synthesized whitespace
    // bitmap) because `TT_Load_Glyph` only honors `FT_LOAD_NO_BITMAP` for
    // scalable faces (`truetype/ttgload.c:2401-2404`).
    let stroked_load_flags = load_flags | ffi::FT_LOAD_NO_BITMAP;
    for glyph in &run.glyphs {
        // Pillow's first loop measures the pen from the unstroked rendered
        // extents (`FT_LOAD_NO_BITMAP | FT_LOAD_RENDER`), then its second
        // loop loads the outline again and strokes it.
        let rendered_slot = ffi::FT_Load_Glyph(face, glyph.glyph_index, stroked_load_flags | RDR)
            .map_err(ft_error_to_pil)?;
        let px = round26(glyph.pen_before);
        if let Some(_bitmap) = rendered_slot.bitmap {
            render_x_min = render_x_min.min(px + rendered_slot.bitmap_left as i32);
            first_y_max = first_y_max.max(rendered_slot.bitmap_top as i32);
        }

        let layout_slot = ffi::FT_Load_Glyph(face, glyph.glyph_index, stroked_load_flags)
            .map_err(ft_error_to_pil)?;
        let bitmap_glyph = stroked_bitmap_glyph(&layout_slot, stroke_filled, stroker.handle)?;
        rendered.push(RenderedBitmap {
            pen_before: glyph.pen_before,
            bitmap_left: bitmap_glyph.left as i32,
            bitmap_top: bitmap_glyph.top as i32,
            bitmap: bitmap_glyph.bitmap,
        });
    }

    // Pillow's `bounding_box_and_anchors` mixes the BASIC-layout advances
    // (loaded with `FT_LOAD_DEFAULT` in `text_layout_fallback`) with glyph
    // cboxes loaded under `FT_LOAD_NO_BITMAP`.  Strikes and outlines can
    // report different advances, so the pen line must come from the
    // default-load run while the cboxes come from the no-bitmap run.
    let bbox_run = glyph_run(ttf, text, stroked_load_flags)?;
    let merged_run = GlyphRun {
        glyphs: run
            .glyphs
            .iter()
            .zip(bbox_run.glyphs.iter())
            .map(|(layout_glyph, cbox_glyph)| RunGlyph {
                glyph_index: layout_glyph.glyph_index,
                pen_before: layout_glyph.pen_before,
                advance: layout_glyph.advance,
                layout_cbox: cbox_glyph.layout_cbox,
            })
            .collect(),
        final_pen: run.final_pen,
        max_pen: run.max_pen,
    };
    let bbox = bbox_from_glyph_run(ttf, &merged_run)?;
    let expected_w = ((bbox.2 - bbox.0) as f32 + stroke_width * 2.0)
        .ceil()
        .max(0.0) as i32;
    let expected_h = ((bbox.3 - bbox.1) as f32 + stroke_width * 2.0)
        .ceil()
        .max(0.0) as i32;
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` allocates the mask from
    // `bounding_box_and_anchors` plus `ceil(stroke_width * 2 + start)`, not
    // from the stroked glyph bitmap extents.  Descender-heavy strings such as
    // DejaVuSans "jQ" can therefore include a leading blank column when
    // `floor(left - stroke_width)` is left of the rendered stroked bitmap, and
    // bitmap-only glyphs keep their bbox-derived height even when the stroked
    // bitmap is empty.
    let x_min = ((bbox.0 as f32) - stroke_width).floor() as i32;
    let x_max = x_min + expected_w;

    let start_width = start.0.ceil() as i32;
    let start_height = start.1.ceil() as i32;
    let base_w = x_max - x_min;
    let base_h = expected_h;
    let adjusted_w = base_w.saturating_add(start_width);
    let adjusted_h = base_h.saturating_add(start_height);
    if positive_dimension_collapsed(base_w, adjusted_w)
        || positive_dimension_collapsed(base_h, adjusted_h)
    {
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

    let x_origin =
        ((f64::from(-render_x_min) + f64::from(stroke_width) + start.0) * 64.0).round() as i32;
    let y_origin =
        ((f64::from(-first_y_max) - f64::from(stroke_width) - start.1) * 64.0).round() as i32;
    paste_rendered_bitmaps(&rendered, &mut canvas, w, h, x_origin, y_origin);
    Ok((w, h, canvas))
}

fn paste_rendered_bitmaps(
    rendered: &[RenderedBitmap],
    canvas: &mut [u8],
    w: u32,
    h: u32,
    x_origin: i32,
    y_origin: i32,
) {
    let wu = w as usize;
    let hu = h as usize;
    for rendered_bitmap in rendered {
        let bitmap = &rendered_bitmap.bitmap;
        let sx = bitmap.width as usize;
        let sy = bitmap.rows as usize;
        let px = pixel(i64::from(
            x_origin.saturating_add(rendered_bitmap.pen_before),
        ));
        let py = pixel(i64::from(y_origin));
        let dx = px + rendered_bitmap.bitmap_left;
        let dy = -(py + rendered_bitmap.bitmap_top);
        let source_x = if dx < 0 { (-dx) as usize } else { 0 };
        let target_x = (dx.max(0) as usize).min(wu);
        if source_x >= sx {
            continue;
        }
        let cw = (sx - source_x).min(wu.saturating_sub(target_x));
        let source_y = if dy < 0 { (-dy) as usize } else { 0 };
        if source_y >= sy {
            continue;
        }
        let end_y = if dy >= 0 {
            sy.min(hu.saturating_sub(dy as usize))
        } else {
            sy
        };
        for row in source_y..end_y {
            let target_y = dy + row as i32;
            let dst = target_y as usize * wu + target_x;
            let dr = &mut canvas[dst..dst + cw];
            for (column, dc) in dr.iter_mut().enumerate() {
                let sc = bitmap_coverage(bitmap, row, source_x + column);
                if sc > 0 {
                    let under = crate::color::muldiv255(u32::from(*dc), u32::from(255 - sc));
                    *dc = sc.saturating_add(under as u8);
                }
            }
        }
    }
}

fn paste_rendered_bitmaps_rgba(
    rendered: &[RenderedBitmap],
    canvas: &mut [u8],
    w: u32,
    h: u32,
    x_origin: i32,
    y_origin: i32,
    foreground: (u8, u8, u8),
) {
    let wu = w as usize;
    let hu = h as usize;
    for rendered_bitmap in rendered {
        let bitmap = &rendered_bitmap.bitmap;
        let sx = bitmap.width as usize;
        let sy = bitmap.rows as usize;
        let px = pixel(i64::from(
            x_origin.saturating_add(rendered_bitmap.pen_before),
        ));
        let py = pixel(i64::from(y_origin));
        let dx = px + rendered_bitmap.bitmap_left;
        let dy = -(py + rendered_bitmap.bitmap_top);
        let source_x = if dx < 0 { (-dx) as usize } else { 0 };
        let target_x = (dx.max(0) as usize).min(wu);
        if source_x >= sx {
            continue;
        }
        let cw = (sx - source_x).min(wu.saturating_sub(target_x));
        let source_y = if dy < 0 { (-dy) as usize } else { 0 };
        if source_y >= sy {
            continue;
        }
        let end_y = if dy >= 0 {
            sy.min(hu.saturating_sub(dy as usize))
        } else {
            sy
        };
        for row in source_y..end_y {
            let target_y = dy + row as i32;
            let dst = (target_y as usize * wu + target_x) * 4;
            for column in 0..cw {
                let source = bitmap_rgba_pixel(bitmap, row, source_x + column, foreground);
                if source[3] == 0 {
                    continue;
                }
                let offset = dst + column * 4;
                blend_color_mask_pixel(&mut canvas[offset..offset + 4], source);
            }
        }
    }
}

fn blend_color_mask_pixel(destination: &mut [u8], source: [u8; 4]) {
    let source_alpha = u32::from(source[3]);
    let destination_alpha = u32::from(destination[3]);
    let inverse_source_alpha = 255 - source_alpha;
    let output_alpha = source_alpha + ((destination_alpha * inverse_source_alpha + 127) / 255);
    if output_alpha == 0 {
        destination.fill(0);
        return;
    }
    for channel in 0..3 {
        let source_premultiplied = u32::from(source[channel]) * source_alpha;
        let destination_premultiplied = u32::from(destination[channel]) * destination_alpha;
        let output_premultiplied =
            source_premultiplied + ((destination_premultiplied * inverse_source_alpha + 127) / 255);
        destination[channel] =
            ((output_premultiplied + output_alpha / 2) / output_alpha).min(255) as u8;
    }
    destination[3] = output_alpha as u8;
}

fn stroked_bitmap_glyph(
    slot: &ffi::FT_GlyphSlot,
    stroke_filled: bool,
    stroker: ffi::FT_Stroker,
) -> Result<ffi::FT_BitmapGlyphOwned, PilError> {
    if slot.bitmap.is_some() {
        // Pillow's `FT_Glyph_Stroke` fails with `Invalid_Argument` for bitmap
        // glyphs (the bitmap glyph class has no stroke method in FreeType's
        // `ftglyph.c`), surfacing as `OSError: invalid argument`.  This is
        // reachable for bitmap-only CBLC/CBDT faces whose strikes win even
        // under `FT_LOAD_NO_BITMAP`.
        return Err(PilError::OsError("invalid argument".into()));
    }
    let outline = ffi::FT_Get_Outline_Glyph(Some(slot)).map_err(ft_error_to_pil)?;
    let stroked = if stroke_filled {
        ffi::FT_Outline_Glyph_StrokeBorder(Some(&outline), stroker, 0).map_err(ft_error_to_pil)
    } else {
        ffi::FT_Outline_Glyph_Stroke(Some(&outline), stroker).map_err(ft_error_to_pil)
    };
    let stroked = stroked?;
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` always renders stroked
    // outline glyphs with `FT_RENDER_MODE_NORMAL`, even when the public
    // `mode="1"` path set `FT_LOAD_TARGET_MONO` for glyph loading.
    ffi::FT_Outline_Glyph_To_Bitmap(&stroked, ffi::FT_RENDER_MODE_NORMAL).map_err(ft_error_to_pil)
}

struct StrokerGuard {
    handle: ffi::FT_Stroker,
}

impl Drop for StrokerGuard {
    fn drop(&mut self) {
        ffi::FT_Stroker_Done(self.handle);
    }
}

fn new_stroker(library: &ffi::FT_Library) -> Result<StrokerGuard, PilError> {
    let mut stroker = std::ptr::null_mut();
    // Keep the status visible at the Pillow boundary. `fontdone::ffi` models
    // FreeType's output-handle contract, so a future allocator/library failure
    // must become Pillow's normal OSError instead of flowing into a null
    // stroker. The guard also mirrors the C render operation's cleanup when a
    // later glyph load or stroke step returns an error.
    check_ft_error(ffi::FT_Stroker_New(Some(library), Some(&mut stroker)))?;
    if stroker.is_null() {
        return Err(PilError::OsError("invalid argument".into()));
    }
    Ok(StrokerGuard { handle: stroker })
}

fn positive_dimension_collapsed(base: i32, adjusted: i32) -> bool {
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` accepts an exactly
    // zero-sized mask dimension when `start` consumes the base dimension;
    // only a negative result raises "bad image size".  `Image::from_luma_mask`
    // likewise preserves a legal `(width, 0)` mask with no pixel bytes.
    base > 0 && adjusted < 0
}

fn bitmap_coverage(bitmap: &ffi::FT_Bitmap, row: usize, column: usize) -> u8 {
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    // `fontdone` preserves FreeType's signed-pitch convention: with a
    // negative pitch the buffer starts at the last logical row and rows are
    // addressed backwards.  `_imagingft.c` consumes that same descriptor
    // directly, so do not normalize the bitmap by reversing its owned bytes
    // here.  This matters for embedded bitmap strikes and any future
    // fontdone route that returns a bottom-up glyph bitmap.
    let physical_row = if bitmap.pitch < 0 {
        bitmap.rows.saturating_sub(1).saturating_sub(row as u32) as usize
    } else {
        row
    };
    let row_start = physical_row.saturating_mul(pitch);
    match bitmap.pixel_mode {
        ffi::FT_PIXEL_MODE_MONO => {
            let byte = bitmap
                .buffer
                .get(row_start + column / 8)
                .copied()
                .unwrap_or(0);
            if byte & (0x80 >> (column & 7)) != 0 {
                255
            } else {
                0
            }
        }
        ffi::FT_PIXEL_MODE_GRAY2 => {
            let byte = bitmap
                .buffer
                .get(row_start + column / 4)
                .copied()
                .unwrap_or(0);
            let value = (byte >> (6 - 2 * (column & 3))) & 0x03;
            value.saturating_mul(85)
        }
        ffi::FT_PIXEL_MODE_GRAY4 => {
            let byte = bitmap
                .buffer
                .get(row_start + column / 2)
                .copied()
                .unwrap_or(0);
            let value = if column & 1 == 0 {
                byte >> 4
            } else {
                byte & 0x0F
            };
            value.saturating_mul(17)
        }
        ffi::FT_PIXEL_MODE_BGRA => {
            let offset = row_start + column * 4;
            // `fontdone` SBIT decoding mirrors FreeType's BGRA bitmap shape:
            // pitch is `width * 4`, buffer length is `pitch * rows`, and this
            // helper is called only for pixels inside `bitmap.width/rows`.
            // Malformed embedded bitmap tables must be rejected in the lower
            // FreeType-compatible decoder, not hidden by Pillow `_imagingft`.
            debug_assert!(offset + 4 <= bitmap.buffer.len());
            let bgra = &bitmap.buffer[offset..offset + 4];
            gray_for_premultiplied_srgb_bgra(bgra)
        }
        _ => {
            debug_assert_eq!(
                bitmap.pixel_mode,
                ffi::FT_PIXEL_MODE_GRAY,
                "Pillow ImageFont BASIC rendering reaches imagingft as MONO or GRAY"
            );
            bitmap.buffer.get(row_start + column).copied().unwrap_or(0)
        }
    }
}

fn bitmap_rgba_pixel(
    bitmap: &ffi::FT_Bitmap,
    row: usize,
    column: usize,
    foreground: (u8, u8, u8),
) -> [u8; 4] {
    if bitmap.pixel_mode == ffi::FT_PIXEL_MODE_BGRA {
        let pitch = bitmap.pitch.unsigned_abs() as usize;
        let physical_row = if bitmap.pitch < 0 {
            bitmap.rows.saturating_sub(1).saturating_sub(row as u32) as usize
        } else {
            row
        };
        let offset = physical_row
            .saturating_mul(pitch)
            .saturating_add(column.saturating_mul(4));
        debug_assert!(offset + 4 <= bitmap.buffer.len());
        if let Some(bgra) = bitmap.buffer.get(offset..offset.saturating_add(4)) {
            return rgba_for_premultiplied_srgb_bgra(bgra);
        }
    }
    let coverage = bitmap_coverage(bitmap, row, column);
    if coverage == 0 {
        [0, 0, 0, 0]
    } else {
        [foreground.0, foreground.1, foreground.2, coverage]
    }
}

fn rgba_foreground_from_ink(ink: Option<i64>) -> (u8, u8, u8) {
    let bytes = ink.unwrap_or(0).to_le_bytes();
    (bytes[0], bytes[1], bytes[2])
}

fn rgba_for_premultiplied_srgb_bgra(bgra: &[u8]) -> [u8; 4] {
    let alpha = u32::from(bgra[3]);
    if alpha == 0 {
        return [0, 0, 0, 0];
    }
    [
        (u32::from(bgra[2]) * 255 / alpha).min(255) as u8,
        (u32::from(bgra[1]) * 255 / alpha).min(255) as u8,
        (u32::from(bgra[0]) * 255 / alpha).min(255) as u8,
        bgra[3],
    ]
}

fn gray_for_premultiplied_srgb_bgra(bgra: &[u8]) -> u8 {
    let alpha = u32::from(bgra[3]);
    if alpha == 0 {
        return 0;
    }
    let luminance = (4731u32 * u32::from(bgra[0]) * u32::from(bgra[0])
        + 46868u32 * u32::from(bgra[1]) * u32::from(bgra[1])
        + 13937u32 * u32::from(bgra[2]) * u32::from(bgra[2]))
        >> 16;
    alpha.wrapping_sub(luminance / alpha) as u8
}
