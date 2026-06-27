# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State

| Backend | Pass | Total | Rate | Reference source |
|---------|------|-------|------|-----------------|
| PIL | 1264 | 1910 | 66.2% | PIL 12.2.0 getmask/getbbox (FreeType 2.14.3) |
| FreeType raw | 1238 | 1910 | 64.8% | `/tmp/gen_ft_refs` (FT_LOAD_RENDER from vendored 2.14.3) |

**2026-06-27:** Fixed `compute_stem_width` smooth path (was using `snap_width`
instead of C's inline logic; serif branch was missing `return`).
PIL +94, FreeType +151.

## Root Cause (confirmed via eprintln tracing, then fixed)

The core issue was **edge positioning** (not point interpolation / IUP).
Edge[1] and edge[2] formed a linked STEM pair in both Rust and C, but
`compute_stem_width` returned wrong widths because the smooth-hinting branch
called `snap_width` (a strong-hinting-only function).

### 'A' at DejaVuSans 10pt — before vs after fix

| Edge | C pos | Rust (before) | Rust (after fix) |
|------|-------|---------------|-------------------|
| edge[0] | 0 | 0 ✅ | 0 ✅ |
| edge[1] | 132 | 134 ❌ | 132 ✅ |
| edge[2] | 188 | 186 ❌ | 189 (off by 1) |
| edge[3] | 512 | 512 ✅ | 512 ✅ |

The edge[2] 1-unit gap comes from simplified bdelta=0 — see "Remaining Work" below.

## Fixed Bugs

### ✅ BUG 1: `compute_stem_width` smooth path used wrong function
- **Was:** Called `snap_width` (strong-hinting only, `af_latin_snap_width`)
- **Now:** Uses C's exact inline smooth logic (aflatin.c:4016-4075):
  - `|dist - standard_width| < 40` → snap to standard width, clamp ≥48
  - `dist < 3*64` → fractional-pixel quantization
  - else → bdelta adjustment + round (simplified with bdelta=0 for now)
- **Commit:** `pillow-rs-freetype/src/autohint/latin.rs` — `compute_stem_width`

### ✅ BUG 2: Serif path missing `return` in `compute_stem_width`
- **Was:** Serif check fell through to width quantization
- **Now:** `return dist` immediately, matching C's `goto Done_Width`

### ✅ NOT-A-BUG: Edge links
- Confirmed `link_segments_inner` creates correct segment links
- Confirmed `compute_edges` propagates them to edge links
- Confirmed `major_dir` is correct (non-absoluted value, matching C)
- Phase 2 STEM path in `hint_edges` executes correctly for edge[1]↔edge[2]

## Verified Functions (algorithmically correct)

| Function | C reference | Status |
|----------|------------|--------|
| `ft_mul_div` / `ft_mul_fix` | ftcalc.c:161,211 | ✅ Byte-identical |
| `apply_hints` (structure + flags) | aflatin.c:4843+ | ✅ Matches C |
| `metrics_scale_dim` | aflatin.c:1178-1437 | ✅ Matches C |
| `metrics_init_widths` | aflatin.c:950-1066 | ✅ Matches C |
| `metrics_init_blues` | aflatin.c:311-1039 | ✅ Matches C |
| `loader::reload` | afhints.c:873-1298 | ✅ Matches C |
| `compute_segments` | aflatin.c:1557-2008 | ✅ Matches C |
| `compute_edges` | aflatin.c:2182-2495 | ✅ Matches C |
| `link_segments_inner` | aflatin.c:2015-2148 | ✅ Matches C |
| `compute_blue_edges` | aflatin.c:2529-2640 | ✅ Matches C |
| `hint_edges` Phase 1 (blue-zone) | aflatin.c:4247-4336 | ✅ Matches C |
| `hint_edges` Phase 2 (STEM) | aflatin.c:4340-4564 | ✅ Matches C |
| `hint_edges` Phase 3 ('m' symmetry) | aflatin.c:4582-4627 | ✅ Matches C |
| `hint_edges` Phase 4 (non-stem) | aflatin.c:4629-4824 | ✅ Structure correct |
| `align_edge_points` | afhints.c:1338-1400 | ✅ Matches C |
| `align_strong_points` | afhints.c:1413-1578 | ✅ Matches C |
| `align_weak_points` (IUP) | afhints.c:1687-1808 | ✅ Matches C |
| `iup_shift` / `iup_interp` | afhints.c:1592,1619 | ✅ Matches C |
| `snap_width` | aflatin.c:2725-2767 | ✅ Matches C (strong path only) |

## Remaining Work (646 PIL / 672 FT failures)

### [ ] COMPUTE_STEM_WIDTH: bdelta adjustment
- **File:** `latin.rs`, `compute_stem_width`, around line 1395
- **Issue:** bdelta is hardcoded to 0. C adjusts stem widths when base_delta
  and width have the same sign (aflatin.c:4050-4075), using ppem:
  ```c
  if (ppem < 10) bdelta = base_delta;
  else if (ppem < 30) bdelta = (base_delta * (30 - ppem)) / 20;
  ```
- **Impact:** Fixes edge[2].pos from 189→188 for 'A' at 10pt. May fix more.

### [ ] HINT_EDGES Phase 4: serif cross-axis overlap check
- **File:** `latin.rs`, `hint_edges`, Phase 4 serif section
- **Issue:** C checks cross-axis overlap of serif segments using `v` coords.
  We skip this and treat all serifs as valid. May cause false serif pairing.

### [ ] HORZ edges: 0 edges for 'A' (both Rust and C)
- C also produces 0 HORZ edges for 'A' (diagonal strokes only).
  Not a bug — expected behavior. Investigate if other glyphs have HORZ issues.

### [ ] Segment filtering thresholds in `compute_edges`
- **File:** `latin.rs`, `compute_edges`, seg filtering
- **Issue:** Threshold computation uses scaled 26.6 values.
  Needs verification against C's `af_hint_edges_compute_*Thresh`.

### [ ] `compute_stem_width` round-path and strong-path
- Round path and strong path (snap_width branch) not yet verified against
  glyphs that exercise them. Smooth path is verified for 'A'.

### [ ] Re-validate after each fix
- `cargo test -p pillow-rs-freetype test_font_coverage_matrix -- --nocapture`

## Debugging Tools

| Tool | Location | Purpose |
|---|---|---|
| FreeType 2.14.3 debug .so | `~/.local/lib/libfreetyped.so` | C trace with `FT2_DEBUG` |
| C tracer binary | `/tmp/trace_ft_debug` | Dumps hinted 26.6 points + bitmap |
| FT reference generator | `/tmp/gen_ft_refs` + `scripts/gen_ft_matrix.py` | Regenerates `coverage_matrix_ft.json` |
| PIL reference generator | `scripts/generate_font_refs.py` | Regenerates `coverage_matrix.json` |
| Rust dump tool | `examples/dump_all_masks` | Dumps per-glyph output for both backends |
| cmp_glyph example | `examples/cmp_glyph.rs` | Quick single-glyph comparison |
| References metadata | Both JSON files | Self-documenting (generator, version, mode) |

## NOT the Problem

- ❌ **Point interpolation (IUP)**: iup_interp/iup_shift algorithmically correct
- ❌ **align_strong_points edge search**: <= / >= comparisons correct
- ❌ **edge links**: link_segments_inner + compute_edges work correctly
- ❌ **major_dir**: Non-absoluted value matches C
- ❌ **Version mismatch**: All sources use FreeType 2.14.3
- ❌ **Scaler/ft_mul_fix**: Pre-hinting coords match C
- ❌ **Standard widths**: VERT=194, HORZ=156 confirmed
- ❌ **HORZ edges missing**: C also produces 0 for 'A'
- ❌ **rasterizer (grays.rs)**: Byte-accurate to ftgrays.c
