//! TrueType bytecode hinter — runs the font's embedded glyph programs.
//!
//! C reference: `src/truetype/ttinterp.c` (7,546 lines), `ttgload.c:770-924`,
//! `ttpload.c` (fpgm/prep/cvt table loading), `ttobjs.c` (TT_Load_Context).
//!
//! This module is called from `scaler.rs:scale_glyph()` when `latin_metrics`
//! is `None` and the font has bytecode tables (fpgm, prep, cvt).
//! It operates on 26.6 fixed-point coordinates and mutates them in-place
//! to match Python Pillow's pixel output (which uses FreeType's native
//! bytecode interpreter via `FT_LOAD_DEFAULT`).
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
//! 1. **Setup**: Load fpgm/prep bytecode, initialize execution context
//!    with CVT values scaled to the current ppem, set up the glyph zone
//!    with 4 phantom points.
//!
//! 2. **Execution**: Run the font program (fpgm) once to define functions,
//!    then run the glyph's instruction stream through the bytecode VM.
//!    This modifies point coordinates in the `zone.cur` array.
//!
//! 3. **Cleanup**: Copy hinted coordinates from `zone.cur` back to the
//!    scaled outline, restore freedom/projection vectors.

pub mod zone;
pub mod tables;
pub mod gs;
pub mod exec;

use crate::error::FontError;
use crate::outline::OutlinePoint;
use zone::GlyphZone;

/// Scale factors and ppem for bytecode hinting, replacing individual params.
pub struct HintScale {
    pub x_scale: i32,
    pub y_scale: i32,
    pub ppem: i32,
}

/// Entry point: run bytecode hinting on scaled 26.6 coordinates.
///
/// This is called once per glyph when:
/// - The backend is `BitmapBackend::PIL` (no autohinting available)
/// - The font has `fpgm`, `prep`, and `cvt` tables
///
/// The `scaled` array is modified in-place with the hinted coordinates.
/// After calling this function, the caller should compute bbox from
/// the updated `scaled` array.
///
/// # Arguments
/// * `scaled` — 26.6 coordinates (modified in-place)
/// * `raw` — font-unit coordinates from the glyf table
/// * `cvt` — control value table (scaled to 26.6, one entry per CVT index)
/// * `fpgm` — font program bytecode (function definitions)
/// * `prep` — CVT program bytecode (executed when ppem changes)
/// * `x_scale` — horizontal scale factor (16.16)
/// * `y_scale` — vertical scale factor (16.16)
/// * `ppem` — pixels per em
///
/// # Current status
///
/// Phase 1 (infrastructure): ✅ complete — parses tables, sets up zones,
/// initializes execution context, runs fpgm for function definitions.
///
/// Phase 2 (VM opcodes): ✅ glyph opcodes implemented — 30+ opcodes operational.
/// Phase 3 (prep + IUP): 🚧 in progress.
pub fn hint_glyph(
    scaled: &mut [OutlinePoint],
    raw: &[OutlinePoint],
    cvt: &[i32],
    fpgm: &[u8],
    prep: &[u8],
    scale: &HintScale,
    glyph_ins: &[u8],
) -> Result<(), FontError> {
    // ── Build the glyph zone ──────────────────────────────────────────
    // C: ttgload.c:874-891 — adds 4 phantom points to the zone.
    // Phantom points are at indices [n_points..n_points+3], not included
    // in the glyph's n_contours. They represent:
    //   pp1: left side bearing   (x = 0 at origin scaling)
    //   pp2: advance width       (x = lsb + advance)
    //   pp3: top side bearing    (y = 0 in horizontal mode)
    //   pp4: bottom side bearing (y = 0 in horizontal mode)
    //
    // For PIL emulation, we set horizontal phantom points (pp1, pp2)
    // to zero and rely on the bytecode program to adjust them.
    // Vertical phantoms (pp3, pp4) are unused in Latin fonts.

    let n_points = scaled.len();
    let n_phantoms = 4;
    let total = n_points + n_phantoms;
    let n_pts: u16 = u16::try_from(total).unwrap_or(u16::MAX);

    let mut zone = GlyphZone {
        cur_x: Vec::with_capacity(total),
        cur_y: Vec::with_capacity(total),
        org_x: Vec::with_capacity(total),
        org_y: Vec::with_capacity(total),
        orus_x: Vec::with_capacity(total),
        orus_y: Vec::with_capacity(total),
        tags: vec![0u8; total],
        contours: vec![],
        n_points: n_pts,
        n_contours: 0,
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

    // Add phantom points
    // pp1: left side bearing (unhinted = 0)
    zone.cur_x.push(0);
    zone.cur_y.push(0);
    zone.orus_x.push(0);
    zone.orus_y.push(0);
    // pp2: advance width (scaled, will be rounded by run_program)
    zone.cur_x.push(0);
    zone.cur_y.push(0);
    zone.orus_x.push(0);
    zone.orus_y.push(0);
    // pp3, pp4: vertical phantom points (unused)
    zone.cur_x.push(0);
    zone.cur_y.push(0);
    zone.orus_x.push(0);
    zone.orus_y.push(0);
    zone.cur_x.push(0);
    zone.cur_y.push(0);
    zone.orus_x.push(0);
    zone.orus_y.push(0);

    // Copy cur → org for the bytecode interpreter's initial state
    zone.org_x = zone.cur_x.clone();
    zone.org_y = zone.cur_y.clone();

    // ── Initialize execution context ──────────────────────────────────
    let mut ctx = exec::ExecContext::new(
        scale.x_scale,
        scale.y_scale,
        scale.ppem,
        cvt,
        fpgm,
    );

    // Run the font program to set up function definitions
    if !fpgm.is_empty() {
        ctx.run_fpgm()?;
    }

    // Prep program disabled: needs twilight zone initialization first.
    // Running against uninitialized twilight zone zeroes out CVT values
    // via WCVTP, which breaks MIRP in glyph programs.
    let _prep = prep;

    // CVT scaling: without prep execution, CVT values are in font_units * 64.
    // Scale to 26.6 pixel units so MIRP/MIAP compute correct distances.
    // Each CVT entry is a FWORD (i16) from the font file, multiplied by 64
    // in our parser (matching C's FT_GET_SHORT() * 64).
    // To get 26.6 pixel units: ft_mul_fix(cvt_i16, y_scale)
    // = ft_mul_fix(cvt_26dot6 / 64, y_scale) = (cvt_i16 * y_scale) >> 16
    // where cvt_i16 = cvt_raw / 64 = cvt[i] / 64
    let y_scale = scale.y_scale;
    for cv in &mut ctx.cvt {
        // cvt[i] is in font_units * 64. Extract the font-unit value
        // by dividing by 64, then scale to 26.6 pixel units.
        let fu = *cv / 64;
        *cv = crate::fixed::ft_mul_fix(fu, y_scale);
    }

    // ── Run the glyph's instruction stream ────────────────────────────
    if !glyph_ins.is_empty() {
        ctx.set_glyph_program(glyph_ins);
        ctx.gs.set_vectors_to_y();
        ctx.run_program(&mut zone)?;
    }

    // ── Write hinted coordinates back ──────────────────────────────────
    for (i, pt) in scaled.iter_mut().enumerate().take(n_points) {
        pt.x = zone.cur_x[i];
        pt.y = zone.cur_y[i];
    }

    Ok(())
}
