//! TrueType bytecode hinter — runs the font's embedded glyph programs.
//!
//! C reference: `src/truetype/ttinterp.c` (7,546 lines), `ttgload.c:770-924`,
//! `ttpload.c` (fpgm/prep/cvt table loading), `ttobjs.c` (TT_Load_Context).
//!
//! This module is called from `scaler.rs:scale_glyph()` when `latin_metrics`
//! is `None` and the font has bytecode tables (fpgm, prep, cvt).
//! It operates on 26.6 fixed-point coordinates and mutates them in-place
//! to match FreeType's native bytecode interpreter via `FT_LOAD_DEFAULT`.
//!
//! ## Architecture
//!
//! ```text
//! scaler::scale_glyph()
//!   └─ if metrics.is_some() → autohint_glyph()   [existing, unchanged]
//!      else if has bytecode  → hinter::hint_glyph() [NEW — this module]
//! ```
//!
//! The hinter runs in three stages:
//!
//! 1. **Setup**: Reuse the face/size execution context prepared by the caller,
//!    then set up the glyph zone with 4 phantom points.
//!
//! 2. **Execution**: Use the function definitions and graphics state saved
//!    by `fpgm`/`prep`, then run the glyph's instruction stream through the
//!    bytecode VM. This modifies point coordinates in `zone.cur`.
//!
//! 3. **Cleanup**: Copy hinted coordinates from `zone.cur` back to the
//!    scaled outline, restore freedom/projection vectors.

pub mod exec;
pub mod gs;
pub(crate) mod iup;
pub mod tables;
pub mod zone;

use crate::error::FontError;
use crate::outline::OutlinePoint;
use crate::outline::{OUTLINE_IGNORE_DROPOUTS, OUTLINE_INCLUDE_STUBS, OUTLINE_SMART_DROPOUTS};
use zone::GlyphZone;

/// TrueType interpreter render mode used by GETINFO and prep re-execution.
///
/// C stores this in `exec->mode` from `FT_LOAD_TARGET_MODE(load_flags)`
/// (`src/truetype/ttgload.c:2203-2228`) and exposes it to bytecode through
/// `GETINFO` (`src/truetype/ttinterp.c:6570-6597`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeHintMode {
    Normal,
    Mono,
    Lcd,
    LcdV,
}

/// Scale factors and ppem for bytecode hinting, replacing individual params.
pub struct HintScale {
    pub x_scale: i32,
    pub y_scale: i32,
    pub tt_scale: i32,
    pub ppem: i32,
    pub point_size: i32,
    pub storage_size: usize,
    pub max_function_defs: usize,
    pub max_instruction_defs: usize,
    pub twilight_points: usize,
    pub is_composite: bool,
    pub reset_vectors_at_glyph_entry: bool,
    pub metrics_legacy_phantoms: bool,
    pub pedantic_hinting: bool,
    pub native_hint_mode: NativeHintMode,
    pub phantom_x_override: Option<(i32, i32)>,
}

/// Metrics side effects produced by glyph bytecode hinting.
#[derive(Debug, Clone)]
pub struct HintOutcome {
    /// Horizontal advance derived from hinted phantom points, in 26.6 pixels.
    pub advance_width: i32,
    pub pp1_x: i32,
    pub pp2_x: i32,
    pub pp3_y: i32,
    pub pp4_y: i32,
    /// Outline dropout flags derived from TrueType scan-control state.
    pub outline_flags: u32,
    /// Per-contour black rasterizer dropout controls.
    pub contour_dropouts: Vec<u8>,
    /// Full public `FT_Outline.tags` bytes for real glyph points.
    pub point_tags: Vec<u8>,
    /// Execution context after running this glyph program.
    pub context: exec::ExecContext,
}

/// Build the reusable TrueType execution state for one face/size.
///
/// FreeType runs `fpgm` and `prep` for the active size, then reuses the saved
/// context for glyph loads. Keeping the prepared state in Rust avoids paying
/// bytecode setup cost for every glyph while preserving pure-Rust execution.
pub(crate) fn prepare_context(
    cvt: &[i32],
    fpgm: &[u8],
    prep: &[u8],
    scale: &HintScale,
) -> Result<exec::ExecContext, FontError> {
    let mut ctx = exec::ExecContext::new(cvt, fpgm, scale);

    if !fpgm.is_empty() {
        ctx.run_fpgm()?;
    }
    let saved_storage = ctx.storage.clone();
    ctx.run_prep(prep, &saved_storage)?;
    ctx.backward_compatibility = if scale.native_hint_mode == NativeHintMode::Mono {
        // C `tt_loader_init` disables v40 backward compatibility for
        // `FT_RENDER_MODE_MONO` so monochrome target loads are fully controlled
        // by the font program (ttgload.c:2284-2294).
        0
    } else {
        (ctx.gs.instruct_control & 4) ^ 4
    };
    Ok(ctx)
}

/// Entry point: run bytecode hinting on scaled 26.6 coordinates.
///
/// This is called once per glyph when the font has `fpgm`, `prep`, and `cvt`
/// tables and the default native TrueType path is selected.
///
/// The `scaled` array is modified in-place with the hinted coordinates.
/// After calling this function, the caller should compute bbox from
/// the updated `scaled` array.
///
/// # Arguments
/// * `scaled` — 26.6 coordinates (modified in-place)
/// * `raw` — font-unit coordinates from the glyf table
/// * `prepared_context` — face/size state after running `fpgm` and `prep`
///
/// # Execution Stages
///
/// The entry point prepares glyph and twilight zones, clones the prepared size
/// state, runs glyph bytecode, and applies untouched-point interpolation before
/// returning the modified coordinates to the caller.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hint_glyph(
    scaled: &mut [OutlinePoint],
    raw: &[OutlinePoint],
    raw_tags: &[u8],
    contours: &[u16],
    advance_width: i32,
    raw_advance_width: i32,
    pp1_x: i32,
    raw_pp1_x: i32,
    raw_pp3_y: i32,
    raw_pp4_y: i32,
    scale: &HintScale,
    glyph_ins: &[u8],
    prepared_context: &exec::ExecContext,
) -> Result<HintOutcome, FontError> {
    // ── Build the glyph zone ──────────────────────────────────────────
    // C: ttgload.c:874-891 — adds 4 phantom points to the zone.
    // Phantom points are at indices [n_points..n_points+3], not included
    // in the glyph's n_contours. They represent:
    //   pp1: left side bearing   (x = 0 at origin scaling)
    //   pp2: advance width       (x = lsb + advance)
    //   pp3: top side bearing    (y = 0 in horizontal mode)
    //   pp4: bottom side bearing (y = 0 in horizontal mode)
    //
    // FreeType seeds horizontal phantom points (pp1, pp2), then lets the
    // bytecode program adjust them.
    // Vertical phantoms (pp3, pp4) are unused in Latin fonts.

    let n_points = scaled.len();
    let n_phantoms = 4;
    let total = n_points + n_phantoms;
    let n_pts: u16 = u16::try_from(total).unwrap_or(u16::MAX);
    let default_pp2_x = crate::fixed::ft_mul_fix(raw_pp1_x + raw_advance_width, scale.x_scale);
    let (seed_pp1_x, seed_pp2_x) = if scale.is_composite && !scale.metrics_legacy_phantoms {
        scale.phantom_x_override.unwrap_or((pp1_x, default_pp2_x))
    } else {
        (pp1_x, default_pp2_x)
    };
    let public_base_tags =
        public_base_tags(raw, raw_tags, scale.is_composite, !glyph_ins.is_empty());

    let mut zone = GlyphZone {
        cur_x: Vec::with_capacity(total),
        cur_y: Vec::with_capacity(total),
        org_x: Vec::with_capacity(total),
        org_y: Vec::with_capacity(total),
        orus_x: Vec::with_capacity(total),
        orus_y: Vec::with_capacity(total),
        tags: public_base_tags
            .iter()
            .map(|&tag| public_tag_to_internal_touch_tag(tag))
            .chain(std::iter::repeat(0))
            .take(total)
            .collect(),
        contours: contours.to_vec(),
        n_points: n_pts,
        n_contours: contours.len() as u16,
        first_point: 0,
    };

    // Copy scaled 26.6 coords
    for p in scaled.iter() {
        zone.cur_x.push(p.x);
        zone.cur_y.push(p.y);
    }
    // Copy unscaled font-unit coords
    for p in raw.iter() {
        zone.orus_x.push(p.x);
        zone.orus_y.push(p.y);
    }

    let seed_pp3_y = crate::fixed::ft_mul_fix(raw_pp3_y, scale.y_scale);
    let seed_pp4_y = crate::fixed::ft_mul_fix(raw_pp4_y, scale.y_scale);
    if scale.metrics_legacy_phantoms {
        // Metrics parity branch contract: seed horizontal phantoms at zero and
        // keep vertical phantoms seeded from the loader.  C computes pp2 in
        // font units first (`pp1_fu + advance_fu`) and then scales that
        // combined value (ttgload.c:1339-1342, 958-962); scaling advance
        // alone can differ by one 26.6 unit at FT_PIX_ROUND thresholds.
        zone.cur_x.push(0);
        zone.cur_y.push(0);
        zone.orus_x.push(0);
        zone.orus_y.push(0);
        zone.cur_x.push(seed_pp2_x - pp1_x);
        zone.cur_y.push(0);
        zone.orus_x.push(raw_advance_width);
        zone.orus_y.push(0);
        zone.cur_x.push(0);
        zone.cur_y.push(seed_pp3_y);
        zone.orus_x.push(0);
        zone.orus_y.push(raw_pp3_y);
        zone.cur_x.push(0);
        zone.cur_y.push(seed_pp4_y);
        zone.orus_x.push(0);
        zone.orus_y.push(raw_pp4_y);
    } else {
        // Add phantom points.
        // FreeType seeds horizontal phantoms as pp1 = xMin - lsb and
        // pp2 = pp1 + advance, then translates the final outline by -pp1.x.
        zone.cur_x.push(seed_pp1_x);
        zone.cur_y.push(0);
        zone.orus_x.push(raw_pp1_x);
        zone.orus_y.push(0);
        // pp2: advance width in the shifted glyph coordinate system.
        zone.cur_x.push(seed_pp2_x);
        zone.cur_y.push(0);
        zone.orus_x.push(raw_pp1_x + raw_advance_width);
        zone.orus_y.push(0);
        // pp3, pp4: vertical phantom points.  FreeType seeds them from vmtx
        // or synthesized vertical metrics before scaling (`ttgload.c:1337-1347`).
        zone.cur_x.push(0);
        zone.cur_y.push(seed_pp3_y);
        zone.orus_x.push(0);
        zone.orus_y.push(raw_pp3_y);
        zone.cur_x.push(0);
        zone.cur_y.push(seed_pp4_y);
        zone.orus_x.push(0);
        zone.orus_y.push(raw_pp4_y);
    }

    // Copy cur → org for the bytecode interpreter's initial state
    zone.org_x = zone.cur_x.clone();
    zone.org_y = zone.cur_y.clone();
    if scale.is_composite {
        // C `TT_Hint_Glyph` treats composite parent instructions as operating
        // on already hinted subglyph positions: it copies `cur` into `orus`
        // and runs the interpreter with identity scales (ttgload.c:797-806).
        zone.orus_x = zone.cur_x.clone();
        zone.orus_y = zone.cur_y.clone();
    }

    // ── Initialize execution context ──────────────────────────────────
    // C `TT_Hint_Glyph` receives `loader->exec` after face/size setup has
    // already run `fpgm` and `prep` (`ttgload.c:770-865`). It never owns a
    // fallback size-context initialization path.
    let mut ctx = prepared_context.clone();
    ctx.is_composite = scale.is_composite;
    if scale.is_composite {
        ctx.x_scale = 1 << 16;
        ctx.y_scale = 1 << 16;
    }
    ctx.pedantic_hinting = scale.pedantic_hinting;

    // C `TT_Hint_Glyph` rounds all phantoms before bytecode execution,
    // regardless of v40 backward compatibility (`ttgload.c:812-815`).
    // Compatibility only controls whether the post-program phantoms are
    // copied back to the loader (`ttgload.c:845-857`).
    let pp1_idx = n_points;
    let pp2_idx = n_points + 1;
    let pp3_idx = n_points + 2;
    let pp4_idx = n_points + 3;
    zone.cur_x[pp1_idx] = crate::scaler::ft_pix_round(zone.cur_x[pp1_idx]);
    zone.cur_x[pp2_idx] = crate::scaler::ft_pix_round(zone.cur_x[pp2_idx]);
    zone.cur_y[pp3_idx] = crate::scaler::ft_pix_round(zone.cur_y[pp3_idx]);
    zone.cur_y[pp4_idx] = crate::scaler::ft_pix_round(zone.cur_y[pp4_idx]);

    // ── Run the glyph's instruction stream ────────────────────────────
    if !glyph_ins.is_empty() {
        // C `TT_Run_Context` resets the IUP tracking bits before every glyph
        // program while preserving the compatibility-mode enable bit
        // (`ttinterp.c:7529-7532`).
        ctx.backward_compatibility &= !0x3;
        if scale.reset_vectors_at_glyph_entry {
            ctx.gs.set_vectors_to_x();
        }
        ctx.set_glyph_program(glyph_ins);
        if let Err(error) = ctx.run_program(&mut zone) {
            // Pinned `TT_Hint_Glyph` preserves the partially interpreted zone
            // and suppresses `TT_Run_Context` errors unless FT_LOAD_PEDANTIC
            // is active (ttgload.c:828-837).
            if scale.pedantic_hinting {
                return Err(error);
            }
        }
    }

    // ── Write hinted coordinates back ──────────────────────────────────
    let use_current_phantoms =
        ctx.backward_compatibility == 0 || (scale.is_composite && glyph_ins.is_empty());
    for (i, pt) in scaled.iter_mut().enumerate().take(n_points) {
        pt.x = if scale.metrics_legacy_phantoms {
            // C `TT_Hint_Glyph` saves the current pp1 when glyph bytecode
            // disables v40 compatibility, and `TT_Load_Glyph` translates
            // simple outlines by that saved `-loader.pp1.x` before computing
            // metrics (ttgload.c:845-857, 2578-2583). Composite metrics use
            // the loader's cached bbox instead (ttgload.c:1965-1966).
            let origin_shift = if scale.is_composite {
                0
            } else if use_current_phantoms {
                zone.cur_x[n_points]
            } else {
                pp1_x
            };
            zone.cur_x[i] - origin_shift
        } else {
            let pp1 = if use_current_phantoms {
                zone.cur_x[n_points]
            } else {
                pp1_x
            };
            zone.cur_x[i] - pp1
        };
        pt.y = zone.cur_y[i];
    }

    let (pp1, pp2) = if use_current_phantoms {
        (
            zone.cur_x.get(n_points).copied().unwrap_or(0),
            zone.cur_x
                .get(n_points + 1)
                .copied()
                .unwrap_or(advance_width),
        )
    } else {
        (seed_pp1_x, seed_pp2_x)
    };
    let (pp3_y, pp4_y) = if use_current_phantoms {
        (
            zone.cur_y.get(n_points + 2).copied().unwrap_or(seed_pp3_y),
            zone.cur_y.get(n_points + 3).copied().unwrap_or(seed_pp4_y),
        )
    } else {
        (seed_pp3_y, seed_pp4_y)
    };
    let outline_flags = outline_flags_from_scan_control(ctx.gs.scan_control, ctx.gs.scan_type);
    let mut contour_dropouts =
        vec![dropout_control_from_outline_flags(outline_flags); contours.len()];
    if !glyph_ins.is_empty() {
        // C `TT_Hint_Glyph` stores `GS.scan_type` in the first outline tag
        // after executing glyph bytecode; `ftraster.c` lets that tag override
        // outline-level dropout flags for the first contour only. The scaler
        // rejects zero-contour or zero-point outlines before bytecode dispatch,
        // matching `TT_Load_Glyph`, so an executing glyph program has both.
        contour_dropouts[0] = ctx.gs.scan_type & 7;
    }
    let mut point_tags = Vec::with_capacity(n_points);
    for (index, point) in raw.iter().enumerate().take(n_points) {
        let mut tag = public_base_tags
            .get(index)
            .copied()
            .unwrap_or(if point.on_curve { 0x01 } else { 0x00 })
            & !0x18;
        if zone.tags.get(index).is_some_and(|value| value & 0x01 != 0) {
            tag |= 0x08;
        }
        if zone.tags.get(index).is_some_and(|value| value & 0x02 != 0) {
            tag |= 0x10;
        }
        point_tags.push(tag);
    }
    if !glyph_ins.is_empty() {
        // C stores `(scan_type << 5) | FT_CURVE_TAG_HAS_SCANMODE` in
        // `outline.tags[0]` after TrueType bytecode execution
        // (`src/truetype/ttgload.c:839-840`).
        point_tags[0] |= (ctx.gs.scan_type << 5) | 0x04;
    }

    Ok(HintOutcome {
        advance_width: pp2 - pp1,
        pp1_x: pp1,
        pp2_x: pp2,
        pp3_y,
        pp4_y,
        outline_flags,
        contour_dropouts,
        point_tags,
        context: ctx,
    })
}

fn public_base_tags(
    raw: &[OutlinePoint],
    raw_tags: &[u8],
    is_composite: bool,
    has_parent_instructions: bool,
) -> Vec<u8> {
    raw.iter()
        .enumerate()
        .map(|(index, point)| {
            let fallback = if point.on_curve { 0x01 } else { 0x00 };
            let mut tag = raw_tags.get(index).copied().unwrap_or(fallback);
            if is_composite && has_parent_instructions {
                // C `TT_Process_Composite_Glyph` preserves curve/scan bits but
                // clears component touch state before executing parent
                // instructions (`src/truetype/ttgload.c:1244-1249`).
                tag &= !0x18;
            }
            tag
        })
        .collect()
}

fn public_tag_to_internal_touch_tag(tag: u8) -> u8 {
    let mut internal = 0;
    if tag & 0x08 != 0 {
        internal |= 0x01;
    }
    if tag & 0x10 != 0 {
        internal |= 0x02;
    }
    internal
}

fn outline_flags_from_scan_control(scan_control: bool, scan_type: u8) -> u32 {
    if !scan_control {
        return OUTLINE_IGNORE_DROPOUTS;
    }

    outline_flags_from_scan_type(scan_type)
}

fn outline_flags_from_scan_type(scan_type: u8) -> u32 {
    // C `ttgload.c` maps TrueType scan conversion modes to
    // `FT_OUTLINE_*` flags before the black rasterizer consumes them.
    match scan_type {
        0 => OUTLINE_INCLUDE_STUBS,
        1 => 0,
        4 => OUTLINE_SMART_DROPOUTS | OUTLINE_INCLUDE_STUBS,
        5 => OUTLINE_SMART_DROPOUTS,
        _ => OUTLINE_IGNORE_DROPOUTS,
    }
}

fn dropout_control_from_outline_flags(flags: u32) -> u8 {
    let mut control = 0;
    if flags & OUTLINE_IGNORE_DROPOUTS != 0 {
        control |= 2;
    }
    if flags & OUTLINE_SMART_DROPOUTS != 0 {
        control |= 4;
    }
    if flags & OUTLINE_INCLUDE_STUBS == 0 {
        control |= 1;
    }
    control
}
