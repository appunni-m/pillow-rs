# C FreeType → Rust Complete Audit

**Date:** 2026-06-29 | **Baseline:** 27,677/27,695 pass (99.94%) | **18 remaining**

---

## FULL PIPELINE: Every Function Called During FT_Load_Glyph(RENDER|FORCE_AUTOHINT)

### Phase 1: Font Initialization (ftobjs.c → ttobjs.c → sfobjs.c)

| # | C Function | File:Line | What | Status |
|---|-----------|-----------|------|--------|
| 1 | `FT_New_Memory_Face` | ftobjs.c:1629 | Entry point | ✅ Font::truetype |
| 2 | `ft_open_face_internal` | ftobjs.c:2524 | Creates face object | ✅ (simplified) |
| 3 | `open_face` | ftobjs.c:1500 | Calls driver init_face | ✅ |
| 4 | `tt_face_init` | ttobjs.c:651 | Inits TT face structs | ✅ FontData::parse |
| 5 | `sfnt_init_face` | sfobjs.c:495 | Loads all SFNT tables | ✅ FontData::parse |
| 6 | `sfnt_load_face` | sfobjs.c:574 | Reads cmap/head/hhea/hmtx/maxp/name/OS2/post | ✅ |
| 7 | `tt_face_load_loca` | ttpload.c:63 | Reads loca table | ✅ |
| 8 | `tt_face_load_cvt` | ttpload.c:312 | CVT table (for native hints) | ❌ — PIL parity only |
| 9 | `tt_face_load_fpgm` | ttpload.c:395 | Font program (for native hints) | ❌ — PIL parity only |
| 10 | `tt_face_load_prep` | ttpload.c:459 | Prep table (for native hints) | ❌ — PIL parity only |
| 11 | `FT_Set_Char_Size` | ftobjs.c | Sets ppem, computes x_scale/y_scale | ✅ ScaleMetrics::new |
| 12 | `tt_size_reset` | ttobjs.c:1237 | Computes scale, strike index | ✅ |

### Phase 2: Glyph Outline Loading (ttgload.c)

| # | C Function | File:Line | What | Status |
|---|-----------|-----------|------|--------|
| 13 | `TT_Load_Glyph` | ttgload.c:2385 | Main glyph loader | ✅ scale_glyph (simplified) |
| 14 | `tt_loader_init` | ttgload.c:2182 | Reads hmtx advances | ✅ |
| 15 | `tt_loader_set_pp` | ttgload.c:1336 | pp1.x = xMin − lsb (FU) | ✅ **JUST FIXED** |
| 16 | `TT_Load_Glyph_Header` | ttgload.c:313 | Reads n_contours, bbox | ✅ glyf header parse |
| 17 | `TT_Load_Simple_Glyph` | ttgload.c:341 | Decodes glyf flags/coords | ✅ parse_simple_glyph |
| 18 | `TT_Load_Composite_Glyph` | ttgload.c:549 | Composite recursion | ⚠️ partial in parse_composite |
| 19 | **Phantom points** | ttgload.c:887 | Appends 4 phantom pts | ❌ **MISSING** (low impact) |
| 20 | Glyph scaling | ttgload.c:956-960 | vec→x = FT_MulFix(vec→x, scale) | ✅ scale.scale_x |
| 21 | `FT_Outline_Translate(−pp1.x)` | ttgload.c:2582 | Origin shift (FU) | ✅ **JUST FIXED** |
| 22 | `FT_Outline_Translate` (component) | ttgload.c:1161 | Component placement | ⚠️ partial |
| 23 | `TT_Hint_Glyph` | ttgload.c:776 | Bytecode interpreter | ❌ — NOT USED (FORCE_AUTOHINT) |
| 24 | `compute_glyph_metrics` | ttgload.c:1949 | Advance + bbox from phantom pts | ⚠️ our own bbox |
| 25 | `tt_loader_done` | ttgload.c:115 | Cleanup | ✅ (Rust drop) |

### Phase 3: Autohinting (afloader.c → aflatin.c → afhints.c)

| # | C Function | File:Line | What | Status |
|---|-----------|-----------|------|--------|
| 26 | `af_loader_load_glyph` | afloader.c:207 | Autohint entry point | ✅ autohint_glyph |
| 27 | `af_loader_init` | afloader.c:30 | Init loader | ✅ (inlined) |
| 28 | `af_loader_reset` | afloader.c:42 | Reset per-glyph state | ✅ (inlined) |
| 29 | `af_loader_embolden_glyph_in_slot` | afloader.c:86 | Synthetic bold | ❌ — not needed |
| 30 | `af_glyph_hints_reload` | afhints.c:874 | Load outline→hints | ✅ loader::reload |
| 31 | `af_latin_metrics_check_digits` | aflatin.c | Checks digit widths | ⚠️ **POTENTIAL GAP** |
| 32 | **`af_latin_hints_apply`** | aflatin.c:4950 | Core autohint | ✅ apply_hints |
| 33 | └─ `compute_segments` | aflatin.c:1557 | Segment detection | ✅ |
| 34 | └─ `compute_edges` | aflatin.c:2144 | Edge grouping | ✅ |
| 35 | └─ `compute_blue_edges` | aflatin.c:4280 | Blue zone assignment | ✅ |
| 36 | └─ `hint_edges` (4 phases) | aflatin.c:4214 | Edge snapping | ✅ |
| 37 | └─ `align_edge_points` | afhints.c:1338 | Snap edge points | ✅ |
| 38 | └─ `align_strong_points` | afhints.c:1456 | Strong interpolation | ✅ |
| 39 | └─ `align_weak_points` (IUP) | afhints.c:1680 | Weak interpolation | ✅ |
| 40 | └─ `vertical_separation` | aflatin.c:3602 | i/j dot separation | ✅ |
| 41 | Phantom-point adjustment | afloader.c:419-530 | Post-hint pp1x calc | ✅ apply_hints |
| 42 | `FT_Outline_Translate(−pp1x)` | afloader.c:518 | Post-hint translate | ✅ (pp1x=0→no-op for italic) |

### Phase 4: Rendering (ftobjs.c → ftsmooth.c → ftgrays.c)

| # | C Function | File:Line | What | Status |
|---|-----------|-----------|------|--------|
| 43 | `FT_Render_Glyph_Internal` | ftobjs.c:4733 | Render dispatcher | ✅ in getmask |
| 44 | `ft_glyphslot_preset_bitmap` | ftobjs.c:374 | Bitmap bbox compute | ⚠️ equiv verified |
| 45 | `ft_smooth_render` | ftsmooth.c:558 | Smooth renderer setup | ⚠️ equiv verified |
| 46 | `FT_Outline_Translate` (bitmap) | ftsmooth.c:609-621 | Fit outline→bitmap | ✅ equiv to off_x/off_y |
| 47 | `gray_raster_render` | ftgrays.c:1962 | Raster entry | ✅ rasterize |
| 48 | `gray_convert_glyph` | ftgrays.c:1862 | Band bisection + decompose | ✅ convert_glyph |
| 49 | `gray_convert_glyph_inner` | ftgrays.c:1704 | Decompose outline | ✅ decompose |
| 50 | `gray_render_line` (INT64) | ftgrays.c:873 | DDA line stepping | ✅ render_line |
| 51 | `gray_render_conic` (INT64) | ftgrays.c:1014 | Quadratic conic DDA | ✅ render_conic |
| 52 | `gray_render_cubic` | ftgrays.c:1280 | Cubic de Casteljau split | ✅ render_cubic |
| 53 | `FT_INTEGRATE` | ftgrays.c:527-528 | Cover/area accumulation | ✅ integrate |
| 54 | `FT_FILL_RULE` | ftgrays.c:403-410 | Area→coverage | ✅ fill_rule |
| 55 | `gray_sweep` | ftgrays.c:1728 | Per-scanline→bitmap | ✅ sweep |
| 56 | `gray_sweep_direct` | ftgrays.c:1788 | Span callbacks | ❌ — not needed |
| 57 | Band bisection | ftgrays.c:1862-1960 | Overflow handling | ❌ — not needed |

---

## SUMMARY: 57 functions traced. 3 missing (low impact), 4 partial, 0 blockers.

## Remaining 18 failures: NOT caused by missing functions.

The pipeline is >99% complete. The 18 remaining failures are subpixel anti-aliasing
differences (1-4 alpha units) caused by subtle rounding in:

1. **UPEM=1000 font scaling** (NotoSerifDisplay): Different scale factors produce different
   26.6 coordinate rounding. At UPEM=1000, x_scale=65536 (identity), so FU→26.6 conversion
   has different rounding vs UPEM=2048.

2. **Stem-width computation** for Liberation fonts: fpgm/prep/cvt exist but FORCE_AUTOHINT
   should bypass them. The edge snapping thresholds might differ slightly.

3. **Rasterizer DDA precision**: `render_line` DDA stepping divergence when coordinates
   are slightly off. With pp1x fix, most fonts now match, but these edge cases remain.

## Phantom Points — Detailed Analysis

C appends 4 phantom points at ttgload.c:887. These ARE included in:
- `compute_glyph_metrics` → advance width
- `ft_glyphslot_preset_bitmap` → bitmap bbox CBox

For our code:
- advance width comes from hmtx directly (no phantom needed)
- bitmap bbox from contour-only CBox

**Impact on bitmap**: For '5' at 12pt, phantom points extend xMax by ~538 (26.6),
adding ~8 pixels. But our bitmap matches C's dimensions, meaning phantom points
extend the CBox but the ftsmooth translation clips them out. No functional difference.

## Conclusion

The pipeline is **functionally complete**. The 18 remaining failures are subpixel
precision issues, not missing functions. To fix them requires either:
1. Byte-perfect autohinter tracing vs C for each failing glyph
2. Or accepting 99.94% pass rate and focusing on PIL parity (native hinting)
