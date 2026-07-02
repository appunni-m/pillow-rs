# Dead Code Index — Autohinter Pipeline

**Generated:** 2026-07-02  
**Test suite:** direct_ft_compare (11,084 glyphs across 24 fonts)  
**Coverage tool:** `grcov` + `RUSTFLAGS="-C instrument-coverage"`

## Quick Summary

| File | Covered Lines | Total Lines | Coverage | Uncovered | Notes |
|------|--------------|-------------|----------|-----------|-------|
| latin.rs | 1,607 | 2,188 | 73.4% | 581 | Main hinting pipeline |
| loader.rs | 201 | 203 | 99.0% | 2 | Almost fully covered |
| globals.rs | 114 | 153 | 74.5% | 39 | Standard char + coverage setup |
| globals_data.rs | 63 | 121 | 52.1% | 58 | Type tables, dead helpers |
| grays.rs | 482 | 693 | 69.6% | 211 | Rasterizer — cubic dead for TTF |
| scaler.rs | 164 | 219 | 74.9% | 55 | Scale + hint dispatch |
| cjk.rs | 0 | 138 | 0% | 138 | CJK — entirely unported |
| script.rs | 0 | 58 | 0% | 58 | Script detection — called once at init |
| coverage.rs | 5 | 25 | 20% | 20 | Coverage tracking debug infra |
| types.rs | 87 | 121 | 71.9% | 34 | Small helpers, mostly inlined |

## Real Dead Functions (Genuinely Uncalled)

| Function | File:Line | Lines | Why Dead | Action |
|----------|-----------|-------|----------|--------|
| `snap_width()` | latin.rs:1918 | 15 | Only called from strong-hinting path (`compute_stem_width` strong branch). Our config uses smooth rendering (`AF_LATIN_HINTS_STEM_ADJUST` only). | Keep for future strong-hint support or remove |
| `render_cubic()` | grays.rs:518 | 53 | TrueType fonts use quadratic curves only. Cubic Bézier only needed for CFF/PostScript/OTF-CFF fonts. Current test suite has only TTF fonts. | Keep — needed for CFF font support |
| `cjk_compute_edges()` | cjk.rs:163 | 63 | Full CJK autohinter not ported from afcjk.c. | **Port afcjk.c** for CJK font parity |
| `cjk_metrics_init_widths()` | cjk.rs:42 | 65 | Full CJK autohinter not ported. | **Port afcjk.c** |
| `blue_chars_for_script()` | globals_data.rs:1100 | 55 | Blue zone detection uses `metrics_init_blues_impl` internally, which has its own char iteration. This public helper is dead. | Remove or wire into blue detection |
| `metrics_init_blues_greek()` | latin.rs:354 | 5 | Greek-specific blue init, not wired. `metrics_init_blues_impl` handles everything. | Remove or wire into script dispatch |
| `detect_script()` (globals) | globals.rs:253 | 5 | Redundant — script detection happens in `FaceGlobals::new` via `compute_style_coverage`. | Remove |
| `detect_font_scripts()` | script.rs:126 | 7 | Pre-computes font scripts from cmap coverage. Called once at startup — coverage tool misses it. | Coverage artifact |
| `build_glyph_script_map()` | script.rs:89 | 19 | Builds glyph→script mapping table. Called once at startup — coverage artifact. | Coverage artifact |
| `script_for_codepoint()` | script.rs:70 | 8 | Script lookup helper, used by `build_glyph_script_map`. | Coverage artifact |
| `detect_script()` (script.rs) | script.rs:139 | 5 | Returns blue strings for script; called from `metrics_init_blues`. | Coverage artifact |

## Functions Marked FNDA:0 but Actually Called (Compiler-Inlined)

These show 0 coverage due to Rust compiler inlining at `opt-level=3`. They ARE exercised during tests but grcov can't detect inlined calls.

| Function | File:Line | Why Inlined |
|----------|-----------|-------------|
| `render_scanline()` | grays.rs:260 | Called from `render_line` (single call site) |
| `ft_div_mod()` | grays.rs:49 | Called from `render_scanline` (2 call sites) |
| `metrics_init_blues()` | latin.rs:344 | Called from `globals.rs:FaceGlobals::get_metrics` |
| `link_segments()` | latin.rs:1778 | Thin wrapper around `link_segments_inner` |
| `align_serif_edge()` | latin.rs:1982 | Single-expression function |
| `pixel_ceil()` / `pixel_floor()` / `to_pixel()` / `scale_y()` | scaler.rs | All small helpers, called from `scale_glyph` |
| `is_horizontal()` / `is_vertical()` / `as_i8()` / `num_contours()` | types.rs | Small inline helpers |
| `default()` implementations | types.rs | Called during struct construction |

## Key Uncovered Code Paths in Live Functions

These are branches within frequently-called functions that are never exercised:

| Function | Uncovered Path | Why |
|----------|---------------|-----|
| `metrics_init_widths` | Fallback `latin_constant(50, upem)` path (L237-242) | Standard char always found for tested fonts |
| `compute_stem_width` | Strong hinting path (L2098+) | Only smooth rendering tested |
| `compute_stem_width` | `extra_light` return (L2009) | `extra_light=true` only for very thin fonts |
| `compute_stem_width` | Smooth path standard-width match (L2045-2066) | Standard width rarely matches dist exactly |
| `vertical_separation_adjustments` | i/j dot accent separation | Only applies to specific codepoints (0x69, 0x6A) |
| `hint_edges` Phase 4 | Anchor propagation fallback (L2619-2656) | First non-stem edge always becomes anchor |
| `hint_edges` Phase 4 | BOUND check (L2653-2675) | Stem ordering never violated in tested fonts |

## Recommendations

1. **Remove** `blue_chars_for_script()` and `metrics_init_blues_greek()` — dead code, no callers
2. **Remove** `globals::detect_script()` — redundant with `script::detect_script()`
3. **Keep** `render_cubic()` — needed for CFF/PostScript font support (not in test suite)
4. **Keep** `snap_width()` — needed if strong hinting mode is ever enabled
5. **Port** `cjk.rs` from `afcjk.c` — required for CJK font parity
6. **Ignore** inlined-function FNDA:0 — compiler artifact, not real dead code
