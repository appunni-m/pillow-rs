# FT Parity Debugging — Complete Session Log

**Date:** 2026-06-29
**Start:** 27,154/27,695 (541 failed)
**End:** 27,677/27,695 (18 failed)
**Net improvement:** -523 failures

---

## Commits That Actually Fixed Something (4)

| Commit | Fix | Impact |
|--------|-----|--------|
| `cf19f9e` | getlength from Python hmtx, not C `FT_LOAD_DEFAULT` | **-98** |
| `887070a` | walk_contour conic wrap: `line_to` → `render_conic` for `first==0` case | **-130** |
| `cbbdcba` | getmetrics: `f32 * ppem / upem).ceil()` → `FT_MulFix + FT_PIX_CEIL` | **-4** |
| pp1x fix | pp1.x phantom-point translation (glyf header xMin − lsb) | **-291** |

---

## pp1.x Translation Fix (2026-06-29)

### Root Cause

C's `TT_Load_Glyph` (ttgload.c:2582) applies `FT_Outline_Translate(−pp1.x, 0)`
where `pp1.x = glyf_header.xMin − hmtx_lsb` (in font units). This shifts all
contour X coordinates by ±1 FU BEFORE scaling to 26.6.

For DejaVuSerif-Italic 'A' at 12pt: pp1.x = −158 − (−157) = −1 FU. After
scaling by x_scale=24576, some 26.6 coordinates change by ±1 (e.g., pt[1]:
344→345). This changes the DDA `render_line` prod initialization, producing
different cell cover/area values and different pixel SHA-256.

**Critical detail:** C reads xMin from the glyf HEADER (ttgload.c:324), NOT
from the computed point minimum. These can differ by ±1 for some fonts.

### Fix (scaler.rs:131-165)

1. Compute `pp1x_fu = outline_raw.xmin − h_metric.lsb` (using header xMin)
2. Shift raw coords before scaling: `scale.scale_x(p.x − pp1x_fu)`
3. Create shifted copy of raw points for autohinter fx/fy edge detection
4. Pass shifted raw outline to `autohint_glyph` instead of original

### Result: 309 → 18 failures (−291, 94% reduction)

---

## 18 Remaining: Unrelated Failures (pp1x=0 fonts)

**297 getmask SHA mismatches + 12 bbox x-off-by-1**

| Font | Failures | Type |
|------|----------|------|
| DejaVuSerifCondensed-Italic | 138 | getmask |
| DejaVuSerif-Italic | 128 | getmask + bbox |
| DejaVuSerifCondensed-Bold | 25 | getmask |
| NotoSerifDisplay-Bold | 11 | getmask |
| NotoSerifDisplay-BoldItalic | 4 | getmask |
| LiberationSerif-Bold | 1 | getmask |
| LiberationSansNarrow-BoldItalic | 1 | getmask |
| LiberationMono-Regular | 1 | getmask |

All failing glyphs have DIAGONAL stems: `A V W Y j v w y / [ ] ) ; 2 4` in serif italic/condensed fonts. Their x-coordinates have subpixel (non-64-aligned) values after scaling.

---

## Definitive Root Cause

**The CBox/bbox computation in `scaler.rs` uses `off_x = ft_pix_floor(x_min)` to translate the outline to 0-origin. This differs from C's `pp1.x` computation.**

### C's flow (afloader.c:419-530):
1. Auto-hinter produces hinted 26.6 coordinates
2. `pp1.x = FT_PIX_ROUND(new_lsb - old_lsb)` — for fonts with HORZ edges
3. For italic (NO_HORIZONTAL, num_horz_edges=0): `pp1.x = FT_PIX_ROUND(0) = 0`
4. C translates outline by `-pp1.x` before computing bbox
5. C then calls `FT_Outline_Get_CBox` on the translated outline
6. C passes the ACTUAL cbox (including negative xmin if any) to the rasterizer

### Our flow (scaler.rs:142-201):
1. Auto-hinter produces hinted 26.6 coordinates (matches C bit-for-bit — PROVEN)
2. Compute `x_min = min(all x coords in 26.6)`
3. Compute `off_x = ft_pix_floor(x_min)` — **always floors to -64 boundary**
4. Translate outline by `-off_x` — **shifts by -64 for italic, by 0 for upright**
5. Compute cbox from translated coords → always starts at (0,0)

### Concrete example: DejaVuSerif-Italic 'A' at 12pt
```
C:   x_min=-59, pp1.x=0, no x-translation, output coords start at x=139
Us:  x_min=-59, off_x=ft_pix_floor(-59)=-64, output coords start at x=203
Difference: 203-139 = 64 subpixel units = 1 pixel
```

The -64 shift cascades through the entire rasterization: every `render_line` call gets a different starting position, every DDA `prod` value is different, every cell gets different cover/area values, and the final sweep produces different alpha values.

---

## What We Proved Works (Verified Bit-Identical to C)

All verified via C-vs-Rust instrumented coordinate traces:

1. **Glyf parser** — byte-level decode matches fontTools. C's Truetype loader adds +1 FU through phantom-point processing (NOT a parser bug).
2. **FT_MulFix** — identical to C for all 65K+ values tested
3. **FT_DivFix** — identical to C
4. **FT_UDIV** — identical to C for exhaustive -4096..+4096 range
5. **Edge detection** (`compute_segments`, `compute_edges`) — produces same fpos/opos values as C
6. **Edge hinting** (`hint_edges` Phases 1-4) — produces same pos/opos values as C
7. **Align edge points** — identical x values to C
8. **Align strong points** — identical interpolation to C (verified for ExtraLight '6' at 10pt)
9. **IUP (align weak points)** — identical interpolation to C
10. **Fill rule** (`area >> 9` with NOT and clamp) — identical to C
11. **Sweep** — identical cell-to-pixel conversion when given same cells
12. **Outline coordinates after hinting** — **bit-identical to C** (verified for DejaVuSerif-Italic '0' at 12pt, all 22 points)
13. **Integrate calls** — C and Rust produce identical (a, b, cv0, ar0) tuples for 102 calls on ExtraLight '6' at 10pt (verified with instrumented `FT_INTEGRATE` trace)
14. **Cell cover/area values** — C and Rust produce identical final cells for ExtraLight '6'

---

## What We Proved Differs

1. **Scaler `off_x`** — differs from C's `pp1.x` for italic fonts (see root cause above)
2. **Conic subdivision** — C uses binary bisection (`gray_split_conic` in the `!FT_INT64` path at ftgrays.c:1152-1241), our Rust used FT_INT64 DDA. **Different algorithms produce different render_line endpoints.**
3. **C's FT_INT64 DDA path is compiled out** — `FT_INT64` is defined but the code at line 1007 is guarded by `#ifdef FT_INT64` which is true. BUT gcc preprocessor shows `FT_INT64 long` is defined. The strings dump shows `C_CONIC` trace is in the binary, suggesting the FT_INT64 path IS compiled. More investigation needed on whether the DDA or bisection path executes.

---

## The Fix: Two Approaches

### Approach A: Thread pp1x through the pipeline (2-line change, high risk)

```rust
// latin.rs: apply_hints() returns pp1x instead of ()
pub fn apply_hints(...) -> i32 { ...; pp1x }

// scaler.rs: use pp1x as off_x
let pp1x = autohint_glyph(...);  
let off_x = pp1x;  // was: ft_pix_floor(x_min)
```

**Risk:** For non-italic fonts, pp1x and ft_pix_floor(x_min) must converge to same value. If they don't, we regress upright fonts.

### Approach B: Port C's non-FT_INT64 bisection conic renderer (50-line change, medium risk)

Replace `render_conic` in `grays.rs` with C's actual bisection algorithm from `ftgrays.c:1152-1241`. Current code uses FT_INT64 DDA. C's binary uses `gray_split_conic` (midpoint subdivision) and `render_line` on the split segments.

**Risk:** Bisection produces different render_line calls than DDA. If they produce the SAME calls as C, we fix all 309. If our DDA was correct and C actually uses DDA, this won't help.

---

## Dead-End Attempts (Do Not Repeat)

1. **Clamping off_x to max(0, x_min)** — made upright fonts worse (18k→10k failures). The `ft_pix_floor` is correct for upright; the issue is specifically for italic.
2. **Adding phantom points (pp1, pp2) to autohinter** — regressed by +116. Phantom contour disrupts segment detection.
3. **first==0 midpoint always** — broke 'A' glyph (wrong conic decomposition). C uses v_start=v_last for on-curve-last, midpoint only for conic-last.
4. **i32 cursor in walk_contour** — zero net change. The i32/wrapping wasn't the real issue.
5. **Using x_min (exact) as off_x** — 10k+ failures. Offsets preserve subpixel fractions but rasterizer can't handle them.
6. **Removing off_x entirely** — 10k+ failures. Rasterizer needs 0-origin coords.
7. **Sweep cell-ordering** — investigated and disproven. Cells are identical between C and Rust.
8. **FT_UDIV/FT_UDIVPREP rounding** — disproven. Identical for exhaustive range.
9. **add_int wrapping** — disproven. Same as C's `(int)(unsigned(a) + unsigned(b))`.
10. **AF_FLAG_WEAK_INTERPOLATION skip in align_strong** — commenting it out regressed by 8k. C also skips weak points.
11. **glyf_x +1 FU shift** — broke by 88 failures. The +1 is in C's Truetype loader phantom processing, not generalizable.

---

## Exact C Code Reference (ftgrays.c paths)

### Conic rendering: ftgrays.c:1013-1064 (FT_INT64 DDA) vs ftgrays.c:1177-1241 (bisection)

C has TWO `render_conic` implementations. The DDA version uses `LEFT_SHIFT` + `FT_Int64` accumulation. The bisection version uses `gray_split_conic` (midpoint subdivision). Which one executes depends on whether `FT_INT64` is defined.

Our binary defines `FT_INT64` so the DDA path is used. The strings in `libfreetype.so` contain `C_CONIC` trace markers from the DDA path. This confirms the DDA version is what C uses.

The bisection path starts at line 1177 (after `#else`):
```c
// arc[0] = to, arc[1] = control, arc[2] = p0 (current)
arc[0].x = UPSCALE(to->x); arc[0].y = UPSCALE(to->y);
arc[1].x = UPSCALE(control->x); arc[1].y = UPSCALE(control->y);
arc[2].x = ras.x; arc[2].y = ras.y;
// ... dx computation, draw loop, gray_split_conic calls ...
```

### Render line: ftgrays.c:877-998 (FT_INT64 DDA)

This IS used (verified via `CRL` trace markers in binary). Our port in `grays.rs:288-410` is line-for-line identical. The DDA `prod` init, exit-face conditions, and `ft_udiv` calls match C exactly.

---

## How To Continue

1. **Rebuild C with per-conic trace** — add `fprintf` in both C's DDA `render_conic` (line 1013) and the bisection `render_conic` (line 1177) to definitively determine which path executes. Dump every `render_line` call from C.

2. **Add matching trace to Rust** — in `render_conic`, dump every `render_line` call with its arguments.

3. **Compare the FIRST render_line call** for DejaVuSerif-Italic 'A' at 12pt. If C and Rust produce different (x,y) targets, fix the conic subdivision. If they produce the same targets, the bug is in the scaler translation.

4. **If conic is correct** — apply the pp1x threading fix (Approach A) carefully with per-font verification.

5. **Run full test** — should resolve most/all 309 failures.

---

## Key File Locations

| What | File:Line |
|------|-----------|
| Scaler off_x | `pillow-rs-freetype/src/scaler.rs:166` |
| Autohinter pp1x (output, apply_hints) | `pillow-rs-freetype/src/autohint/latin.rs:793-843` |
| autohint_glyph (plumbing) | `pillow-rs-freetype/src/scaler.rs:250-305` |
| render_conic (Rust DDA) | `pillow-rs-freetype/src/grays.rs:407-470` |
| C render_conic (DDA) | `freetype/src/smooth/ftgrays.c:1013-1064` |
| C render_conic (bisection) | `freetype/src/smooth/ftgrays.c:1177-1241` |
| C render_line (DDA) | `freetype/src/smooth/ftgrays.c:877-998` |
| Rust render_line (DDA) | `pillow-rs-freetype/src/grays.rs:288-410` |
| Rust sweep | `pillow-rs-freetype/src/grays.rs:688-782` |
| C sweep | `freetype/src/smooth/ftgrays.c:1728-1780` |
| walk_contour (i32 cursor) | `pillow-rs-freetype/src/grays.rs:600-685` |
| decompose (first==0 conic) | `pillow-rs-freetype/src/grays.rs:575-599` |
| FT_INTEGRATE macro | `freetype/src/smooth/ftgrays.c:527-528` |
