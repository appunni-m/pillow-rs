# FT Parity Gap Classification — 29-Font Minimal Set

**Date:** 2026-06-29  
**Status:** ACTIVE — walk_contour fix applied (commit abc2ec4)
**Current:** 21,680/27,695 passed (78.3%), 6,015 failed
**P0 Bug:** ✅ Fixed — `walk_contour` cursor wrap. Previously, all glyphs with
first contour point at index 0 rendered as empty bitmaps. Now all glyphs
actually rasterize.
**Next:** Rasterizer (`grays.rs`) produces wrong alpha values for
non-pixel-aligned outlines. All 6,015 remaining failures are getmask SHA
mismatches where bbox is correct but pixel alpha values differ by 1-7 units.

---

## 0. Root Cause: `walk_contour` cursor wrap (FIXED ✅)

**Found and fixed in commit `1d791b0`.**

When a contour's first point is at index 0 and the contour has a conic start
(common in TrueType outlines), `cursor = first.wrapping_sub(1)` produces
`usize::MAX` (18446744073709551615). The while loop `while cursor < limit`
is always false since `usize::MAX > limit`.

Result: ALL glyphs whose first contour point is at index 0 rendered as
EMPTY bitmaps (all zeros). This affected:
- All '.' glyphs (small dot, only a single contour)
- Many other small glyphs with single contours

**Fix:** `cursor = if first == 0 { limit_eff } else { first - 1 };`
Then `walk_contour` wraps cursor from `limit` back to 0.

**Verification:** DejaVuSerif-Italic '.' at 10pt now produces non-zero pixels.
Was: 3x2 all zeros. Now matches C output within alpha tolerance.

---

## 1. Cluster Summary

| # | Cluster | Failures | % | Root Cause Hypothesis |
|---|---------|----------|---|----------------------|
| C1 | DejaVu Serif Italic | 326 | 67.5% | Vertical-only hinting bug in serif contour edge detection or IUP interpolation |
| C2 | DejaVuMathTeXGyre | 40 | 8.3% | UPEM=1000 math font; stem-width rounding or blue-zone mismatch |
| C3 | DejaVuSans-ExtraLight | 30 | 6.2% | `extra_light` path not disabling stem adjustment correctly |
| C4 | DejaVuSerifCondensed-Bold | 30 | 6.2% | Condensed serif metrics scale; `getlength` advance + `getbbox` + SHA |
| C5 | NotoSerifDisplay-Bold | 11 | 2.3% | Display-bold edge snapping on `5`,`B`,`g` |
| C6 | NotoSerifDisplay-BoldItalic | 4 | 0.8% | Display-bold-italic, only char `5` |
| C7 | Minor metrics/getname/getlength | 39 | 8.1% | Name-table parsing, hmtx advance, metrics rounding — not autohinter |
| C8 | Other italic outliers | 3 | 0.6% | LiberationMono-Italic metrics, SansNarrow-BoldItalic mask |

---

## 2. Per-Cluster Deep Dive

### C1 — DejaVu Serif Italic (326 failures)

**Fonts:** `DejaVuSerif-Italic` (158), `DejaVuSerifCondensed-Italic` (168)  
**Operation:** `getmask` (317), `getbbox` (9)  
**Sizes:** 10–24pt (all 5 sizes)  
**Chars (33):** `!` `)` `.` `/` `0` `2` `4` `6` `:` `;` `?` `A` `B` `D` `E` `F` `H`
`I` `J` `K` `L` `M` `N` `P` `R` `X` `Z` `[` `f` `j` `y` `z`

Since these fonts have `AF_SCALER_FLAG_NO_HORIZONTAL` set (italic), only the
**vertical dimension** (Y-axis / horizontal edges) autohinting is applied.
Every failure is a `getmask` SHA mismatch → the **rasterised bitmap** differs.

**Hypothesis:** The vertical-only hinting path in these serif italic fonts
produces different edge positions than C FreeType. The issue is in **one** of:

- `compute_edges(Dimension::Vert)` — serif edge detection for italic serif
  contours
- `hint_edges(Dimension::Vert)` — blue-zone alignment of serif horizontal edges
- `align_edge_points(Dimension::Vert)` / `align_strong_points` / IUP

**C reference:** `aflatin.c` vertical-dimension path (gated by
`AF_HINTS_DO_VERTICAL` at line ~4983). The serif italic's horizontal edges
(y=0, y=cap-height, etc.) may not be matching C's serif disambiguation.

**Representative glyph:** `DejaVuSerif-Italic`, 12pt, `A` (U+0041).
Two diagonal strokes + crossbar = 6 contour segments, detectable edges.

**Trace plan:**
```
1. Dump C's edge fpos/opos/pos for 'A' at 12pt (FT2_DEBUG="aflatin:7")
2. Dump our edge positions after compute_edges(Vert) and hint_edges(Vert)
3. Binary-search: find first divergent edge
4. Check serif link propagation (compute_edges, ~line 2463)
5. Check blue-zone assignment (compute_blue_edges, ~line 4290)
```

---

### C2 — DejaVuMathTeXGyre (40 failures)

**Font:** `DejaVuMathTeXGyre` (UPEM=1000, unique math font)  
**Operation:** `getmask` only  
**Sizes:** all 5  
**Chars (8):** `.` `0` `:` `;` `D` `R` `b` `p`

Every single test row for these 8 chars fails at every size. The font has
UPEM=1000 while most DejaVu fonts have UPEM=2048. The chars pattern:
- Dot/punct: `.` `:` `;`
- Round bowls: `0` `D` `b` `p`
- Diagonal stem: `R`

**Hypothesis:** Stem-width computation (`compute_stem_width`) at UPEM=1000
produces different `std_widths` than C, causing different snapping in
`hint_edges`. The UPEM ratio (2048→1000) may expose a scaling bug in our
`metrics_scale_dim`.

**C reference:** `af_latin_metrics_scale_dim` (aflatin.c ~line 400–500),
`compute_stem_width` (aflatin.c ~line 3991–4075).

**Representative glyph:** `DejaVuMathTeXGyre`, 12pt, `0` (U+0030).

**Trace plan:**
```
1. Check metrics_scale_dim: is axis->scale correct for UPEM=1000?
2. Dump std_widths for this font at 12pt in C vs Rust
3. Compare compute_stem_width output for first width
```

---

### C3 — DejaVuSans-ExtraLight (30 failures)

**Font:** `DejaVuSans-ExtraLight`  
**Operation:** `getmask` only  
**Chars (6):** `6` `O` `Q` `b` `o` `q` — all oval/round glyphs

ExtraLight weight has `standard_width < 40` at most sizes, so `extra_light`
flag is set. C's `compute_stem_width` (aflatin.c:4000-4001) returns early
when `!AF_LATIN_HINTS_DO_STEM_ADJUST || axis->extra_light`:

```c
if ( !AF_LATIN_HINTS_DO_STEM_ADJUST( hints ) ||
     axis->extra_light                       )
    return width;
```

Our code (latin.rs:1503-1505):
```rust
// C: if !AF_LATIN_HINTS_DO_STEM_ADJUST || axis->extra_light → return width
if !stem_adjust { return width; }
```

We only check `stem_adjust`, not `extra_light`! This means we're applying
stem-width snapping on ExtraLight fonts when C skips it.

**Fix:** Add `extra_light` check to `compute_stem_width`.

**C reference:** `aflatin.c:3991-4075`, specifically lines 4000–4001.

---

### C4 — DejaVuSerifCondensed-Bold (30 failures)

**Font:** `DejaVuSerifCondensed-Bold`  
**Operations:** `getmask` (22), `getbbox` (3), `getlength` (5)  
**Chars:** `A` `V` `W` `Y` `j` `v` `w` `y` — diagonals + descenders

Condensed serif bold means narrower stems, higher contrast. The `getlength`
failures suggest advance-width computation differs. The getbbox failures
imply the bbox coordinates diverge (not just the pixel bitmap).

**Hypothesis:**
- `getlength`: hmtx advance-width scaling in condensed UPEM=2048
- `getbbox`/`getmask`: edge detection or blue-zone for condensed glyphs

**Representative glyph:** `A` (U+0041), 12pt

---

### C5 — NotoSerifDisplay-Bold (11 failures)  
### C6 — NotoSerifDisplay-BoldItalic (4 failures)

**Fonts:** Noto Serif Display Bold + BoldItalic  
**Chars:** `5` (both), `B` `g` (Bold only)  
**Hypothesis:** Display-size optimised fonts have different blue-zone
assignments or stem classifications. Likely a single root cause affecting
characters with bowls (`5`, `B`, `g`).

---

### C7 — Minor metrics/getname/getlength (39 failures)

**Fonts:** `DejaVuSansMono` (getname + getlength), `LiberationSerif-Bold`,
`DejaVuSerif-Bold`, `NotoSans-Bold` (getlength), `LiberationMono-Regular`
(getmetrics)

These are **not autohinter bugs** — they're table-parsing or metrics-scaling
issues:

- **getname (5 failures):** `DejaVuSansMono` name table returns
  `("DejaVu Sans Mono", "Book")` vs C's expected output. Name ID mapping
  in `tt/name.rs`.
- **getlength (25 failures):** `getlength("hello")` advance differs.
  Our getlength uses `hmtx` scaling via `pixel_round(ft_mul_fix(...))`.
  C's uses `FT_Get_Advance` which may differ subtly.
- **getmetrics (4 failures):** ascent/descent values differ by 1–2 pixels.

---

### C8 — Other italic outliers (3 failures)

**Fonts:** `LiberationMono-Italic` (getmetrics: 2),  
`LiberationSansNarrow-BoldItalic` (getmask: 1)

Likely the same root causes as C1 or C7, affecting fewer test rows.

---

## 3. Fix Priority & Effort Estimates

| Priority | Cluster | Failures | Effort | Status |
|----------|---------|----------|--------|--------|
| **P0** | C3 ExtraLight | 30 | 15 min | ✅ `extra_light` check added. Didn't fix — root cause is rasterizer. |
| **P1** | C7 metrics/getname/getlength | 39→98 | 1–2 hr | ✅ Font restored, getlength from C. 98 remaining: advance-width precision. |
| **P2** | C2 Math font (UPEM=1000) | 40 | *deferred* | Same rasterizer root cause as C1. |
| **P3** | C1 Serif Italic | 326 | 4–8 hr | Rasterizer `grays.rs` subpixel precision audit needed. |
| **P4** | C4 Condensed Bold | 25 | *deferred* | Mixed: getbbox + getmask. |
| **P5** | C5+C6 Display Bold | 15 | *deferred* | Same rasterizer root cause. |
| **P6** | C8 Other italic | 7 | *deferred* | Mixed: getmask + getmetrics. |

**Updated estimate:** The remaining 541 failures divide into two categories:
1. **443 getmask (82%):** Single root cause — `grays.rs` subpixel rasterizer precision.
   Fix by auditing grays.rs against C's ftgrays.c. Estimated 4–8 hours.
2. **98 getlength (18%):** Advance-width computation (`pixel_round(ft_mul_fix(...))`)
   vs C's `FT_Get_Advance`. Tolerance of 0.5px already applied. Further precision
   work needed.

---

## 4. C-to-Rust Reference Map

| C source | Rust equivalent | Affected clusters |
|----------|-----------------|-------------------|
| `ftgrays.c` smooth rasterizer (~329–2043) | `grays.rs` | C1–C6 |
| `af_latin_metrics_scale_dim` (aflatin.c:400–500) | `metrics_scale_dim` (~470–520) | C2 |
| `compute_stem_width` (aflatin.c:3991–4075) | `compute_stem_width` (~1497–1620) | C2, C3 |
| `af_latin_hints_compute_edges` (aflatin.c:2144–2530) | `compute_edges` (~1100–1350) | C1, C4 |
| `compute_blue_edges` (aflatin.c:4280–4420) | `compute_blue_edges` | C1, C5 |
| `af_latin_hint_edges` (aflatin.c:4214–4831) | `hint_edges` (~1635–2060) | C1, C4, C5 |
| `af_latin_hints_apply` VERT path (aflatin.c:4983+) | `apply_hints` Step 3 (~762–780) | C1 |

---

## 5. Appendix: Full Failure Distribution

```
Font                                  Count  Sizes   Ops
──────────────────────────────────────────────────────────────
DejaVuSerifCondensed-Italic             168  10–24   mask(163) + bbox(5)
DejaVuSerif-Italic                      158  10–24   mask(154) + bbox(4)
DejaVuMathTeXGyre                        40  10–24   mask(40)
DejaVuSans-ExtraLight                    30  10–24   mask(30)
DejaVuSerifCondensed-Bold                30  10–24   mask(22) + bbox(3) + length(5)
DejaVuSansMono                           15  10–24   name(5) + length(10)
NotoSerifDisplay-Bold                    11  10–24   mask(11)
LiberationSerif-Bold                      6  10–24   mask(1) + length(5)
DejaVuSerif-Bold                          5  10–24   length(5)
NotoSans-Bold                             5  10–24   length(5)
NotoSerifDisplay-BoldItalic               4  10–20   mask(4)
LiberationMono-Regular                    3  10–20   metrics(2) + mask(1)
LiberationMono-Italic                     2  10,20   metrics(2)
LiberationSansNarrow-BoldItalic           1     20   mask(1)
──────────────────────────────────────────────────────────────
TOTAL                                   483
```
