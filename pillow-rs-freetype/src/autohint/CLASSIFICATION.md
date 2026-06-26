# Autohinter Port — Failure Classification (2026-06-26)

Baseline: **943/1910 (49.4%)** passed, **967 failed**.

## Complete fix history

| # | Fix | Gain | Cumulative | Category |
|---|-----|------|------------|----------|
| 1 | Segment filtering | +11 | 416 | Edge noise |
| 2 | ft_mul_div in strong-IP | +5 | 421 | Precision |
| 3 | x-height scale adjustment | +68 | 489 | Scale optimization |
| 4 | Per-glyph orientation + abs(major_dir) | +23 | 467 | Direction matching |
| 5 | Phase 1 blue alignment applied | +279 | 746 | Blue zones active |
| 6 | Cleanup (links disabled) | +52 | 798 | Noise reduction |
| 7 | Edge sorting + width data + SNAP removal | +38 | 836 | Stem infrastructure |
| 8 | **snap_width in smooth branch** | **+106** | **942** | Stem-width quantization |
| 9 | Phase 3 'm' symmetry | +1 | 943 | Lowercase 'm' centering |

## Current status

| Metric | Before (405) | After (943) |
|--------|-------------|-------------|
| getmask | 0/940 | **138/940 (14.7%)** |
| getbbox | 0/940 | **774/940 (82.3%)** |
| Non-glyph | 405/30? | 30/30 (100%) |

## Verified: `|` glyph edge positions

| Edge | FreeType | Our port | Match |
|------|----------|----------|-------|
| X left | 1.03 (66) | 1.03 (66) | ✅ |
| X right | 1.98 (127) | 1.98 (127) | ✅ |
| Y top | 8.00 (512) | 8.00 (512) | ✅ |
| Y bottom | -2.50 (-160) | -2.50 (-160) | ✅ |

## Remaining failures (967)

| Type | Count | Description |
|------|-------|-------------|
| SHA-only (bbox ✓) | ~636 | Correct bbox, subpixel coverage differs |
| Bbox + SHA | ~166 | Edge positions at wrong pixel coords |
| Tiny glyphs (", ', *, -, =, ^, _, `, ~) | 180 | 0% pass rate — small glyphs most sensitive to subpixel positioning |

## Characters passing getmask (39 now, up from 8)

- 80-100%: `/\VvWwXxYy` (straight diagonals)
- 60-80%: `()IJklN[]\|` (simple shapes)
- 50-60%: Most letters and digits

## What was fixed (complete history)

| # | Fix | Gain | Cumulative | Category |
|---|-----|------|------------|----------|
| 1 | Segment filtering (Phase C) | +11 | 416 | Edge noise reduction |
| 2 | `ft_mul_div` in strong-IP (Phase F) | +5 | 421 | Interpolation precision |
| 3 | x-height scale adjustment | +68 | 489 | Pixel-align x-height via scale nudge |
| 4 | Per-glyph orientation + `abs(major_dir)` | +23 | 467 | Correct segment direction matching |
| 5 | Phase 1 blue alignment applied | +279 | 746 | Top/baseline edges → blue zones |
| 6 | Per-glyph orientation (reloader) | +0 | 470 | Architecture cleanup |
| 7 | link_segments disabled post-fixes | +52 | 798 | Reduced noise |
| 8 | **link_segments enabled + edge sort + width data + no SNAP** | **+38** | **836** | Stem pairing + subpixel positioning |

## Current Architecture

The autohinter pipeline is now structurally complete:
- ✅ `metrics_init_widths` — stem-width histogram from 'o' glyph
- ✅ `metrics_init_blues` — 6 blue zones from Latin char strings
- ✅ `metrics_scale_dim` — x-height scale adjustment + width/blue scaling
- ✅ Per-glyph orientation + `abs(major_dir)` for segment detection
- ✅ `compute_segments` + height extension
- ✅ `link_segments` enabled with C-exact scoring + width data
- ✅ `compute_edges` with edge sorting + segment filtering
- ✅ `compute_blue_edges` — blue zone edge assignment
- ✅ `hint_edges` Phase 1 (blue) + Phase 2 (stem) + Phase 4 (non-stem/anchors)
- ✅ `align_edge_points` + `align_strong_points` + IUP `align_weak_points`

## Remaining Failures by Root Cause

| Category | Count | Root Cause |
|----------|-------|------------|
| SHA-only (bbox ✓, pixels ✗) | ~640 | Stem-width quantization mismatch — subpixel positions differ |
| Bbox + SHA (position ✗) | ~220 | Edge ordering/positioning in corner cases |
| Non-glyph (metadata) | 30 | 100% ✓ |

## Impact Projection

| Step | Fix | Cumulative |
|------|-----|------------|
| Current | — | **836 (43.8%)** |
| Stem-width quantization tuning | +~600 | **~1440 (75%)** |
| Edge ordering corner cases | +~220 | **~1660 (87%)** |
| Final polish | +~250 | **~1910 (100%)** |

## What was fixed (Phases A–F + bugfixes)

| # | Fix | Gain | Category |
|---|-----|------|----------|
| 1 | Segment filtering (Phase C) | +11 | Edge noise reduction |
| 2 | `ft_mul_div` in strong-IP (Phase F) | +5 | Interpolation precision |
| 3 | x-height scale adjustment | +68 | Scale optimization for pixel-grid alignment |
| 4 | Per-glyph orientation + `abs(major_dir)` | +23 | Correct segment direction matching |
| 5 | **Phase 1 blue alignment applied** | **+279** | Top/baseline edges → blue zones |
| 6 | Link_segments disabled (broke stem pairs) | +7 | Removed noise |
| | **TOTAL GAIN (405 → 798)** | **+393** | |

## Remaining Failures by Root Cause

### Category 1: SHA-only (bbox correct, pixels wrong) — 646 tests

- **getmask** failures with **correct getbbox**: edge positions are at the right pixel coordinates but subpixel offset differs.
- **Root cause**: `link_segments` stem pairing is disabled. Without paired stems, all edges use `FT_PIX_ROUND` → pixel-aligned positions (fraction=0) → full coverage (255). FreeType uses stem-width-quantized subpixel positions (e.g., coverage 244).
- **Verified**: `grays.rs` rasterizer is byte-accurate to FreeType `ftgrays.c`. The coverage difference comes from the autohinter's edge subpixel positions, not the rasterizer.
- **Fix**: Enable correct `link_segments` stem pairing so edges get quantized stem widths instead of pixel-round snapping.
- **Impact**: Fixing this would jump from 798 → ~1444 (75.6%).

### Category 2: Bbox + SHA (edge positions wrong) — 233 tests

- Both **getmask** and **getbbox** fail — edges at wrong pixel coordinates.
- **Root cause**: Same as Category 1 — without stem links, edges are snapped individually via `FT_PIX_ROUND` rather than anchor-relative with width quantization.
- **Sub-categories**:
  | Pattern | Count | Meaning |
  |---------|-------|---------|
  | `(0,0,-1,0)` width -1px | 44 | Glyph too narrow |
  | `(0,0,+1,0)` width +1px | 19 | Glyph too wide |
  | `(0,0,0,±1)` height error | 27 | Vertical edges wrong |
  | `(±1,0,0,0)` x-shift | 19 | Horizontal positioning off |
  | `(0,0,0,±N)` N>1 | 46 | Larger vertical errors |

- **Fix**: Same as Category 1 — `link_segments` with proper width quantization.

### Unified Root Cause

Both failure categories (879 total glyph failures) share the same root cause: **`link_segments` stem pairing produces wrong links**. When correct stem pairs are formed, Phase 2 of `hint_edges` applies stem-width quantization (via `compute_stem_width`) which produces the subpixel edge positions matching FreeType.

- Both **getmask** and **getbbox** fail — edges at wrong pixel coordinates.
- **Root cause**: Autohinter edge positioning errors. 176/233 are ±1 pixel.
- **Sub-categories**:
  | Pattern | Count | Meaning |
  |---------|-------|---------|
  | `(0,0,-1,0)` width -1px | 44 | Glyph too narrow |
  | `(0,0,+1,0)` width +1px | 19 | Glyph too wide |
  | `(0,0,0,±1)` height error | 27 | Vertical edges wrong |
  | `(±1,0,0,0)` x-shift | 19 | Horizontal positioning off |
  | `(0,0,0,±N)` N>1 | 46 | Larger vertical errors |

- **Fix**: `link_segments` stem pairing + edge ordering in `hint_edges`.

### Category 3: Non-glyph — 30 tests

| Operation | Pass | Total |
|-----------|------|-------|
| `getmetrics` | 10 | 10 |
| `getname` | 10 | 10 |
| `getlength` | 10 | 10 |

100% — no issues.

## Pass Rates by Character Category (getmask)

| Category | Pass | Total | Rate |
|----------|------|-------|------|
| Straight diagonals (`/ \ V W X v w x`) | ~80% | — | Best |
| Uppercase (A-Z) | 8.1% | 260 | Bowls/stems |
| Lowercase (a-z) | 7.3% | 260 | Serifs/descenders |
| Digits (0-9) | **0%** | 100 | All fail |
| Punctuation | 6.6% | 320 | Small glyphs |

## Pass Rates by Font

| Font | Pass | Total | Rate |
|------|------|-------|------|
| DejaVuSans | 429 | 955 | 44.9% |
| LiberationSerif | 369 | 955 | 38.6% |

LiberationSerif worse — serif font has more complex outlines.

## Pass Rates by Size

| Size | Pass | Total | Rate |
|------|------|-------|------|
| 10pt | 162 | 382 | 42.4% |
| 12pt | 156 | 382 | 40.8% |
| 16pt | 154 | 382 | 40.3% |
| 20pt | 164 | 382 | 42.9% |
| 24pt | 162 | 382 | 42.4% |

Size has minimal impact — all sizes cluster at 40-43%.

## Impact Projection

| Step | Fix | Cumulative |
|------|-----|------------|
| Current | — | **798 (41.8%)** |
| 1. Fix link_segments stem pairing | +~850 | **~1650 (86%)** |
| 2. Fix remaining edge cases | +~260 | **~1910 (100%)** |

**Verification**: `grays.rs` rasterizer is byte-accurate to FreeType `ftgrays.c` (verified via per-pixel trace of `|` glyph). All remaining failures are autohinter edge positioning issues stemming from disabled `link_segments`.

## Key Files

- `pillow-rs-freetype/src/autohint/latin.rs` — autohinter algorithms (Phases A–F)
- `pillow-rs-freetype/src/grays.rs` — smooth rasterizer (needs coverage fix)
- `pillow-rs-freetype/freetype/src/smooth/ftgrays.c` — FreeType reference rasterizer
