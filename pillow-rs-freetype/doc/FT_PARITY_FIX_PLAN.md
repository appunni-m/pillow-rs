# FT Parity Fix Plan — 443 remaining failures

**Date:** 2026-06-29  
**Baseline:** 27,252/27,695 passed (98.4%), 443 failed, 0 skipped  
**Previous:** walk_contour conic-start fix committed (5267e79), getlength fixed (cf19f9e)

---

## Failure Breakdown

| Category | Count | Root Cause |
|----------|-------|------------|
| getmask SHA mismatch | 427 | DDA line-stepper subpixel rounding |
| getbbox x-off-by-1 | 12 | Phantom-point adjustment skipped for NO_HORIZONTAL fonts |
| getmetrics ascender/descender | 4 | f32 precision in `pick_metrics` ceil |
| **Total** | **443** | |

---

## Fix 1: getmetrics (4 failures) — 15 minutes

### Problem

LiberationMono-Regular and LiberationMono-Italic descender = 4, expected 3.

```
FAIL [LiberationMono-Regular_10_getmetrics]: metrics (9,4) != expected [9, 3]
FAIL [LiberationMono-Italic_10_getmetrics]: metrics (9,4) != expected [9, 3]
FAIL [LiberationMono-Regular_20_getmetrics]: metrics (17,7) != expected [17, 6]
FAIL [LiberationMono-Italic_20_getmetrics]: metrics (17,7) != expected [17, 6]
```

### Root Cause

In `font.rs` line 259:
```rust
let asc = (asc_fu as f32 * ppem / upem).ceil() as u32;
let desc = (desc_fu as f32 * ppem / upem).ceil() as u32;
```

For LiberationMono at 10pt, UPEM=2048:
- `desc_fu = 615` (os2.sTypoDescender absolute value)
- `615 * 10 / 2048 = 3.0029296875`
- f32 rounds this to `3.00293`
- `ceil(3.00293) = 4` → off by 1

### Fix

Use integer arithmetic to match C's `FT_PIX_CEIL`:
```rust
let asc = ((asc_fu as i32 * ppem + upem as i32 - 1) / upem as i32) as u32;
let desc = ((desc_fu as i32 * ppem + upem as i32 - 1) / upem as i32) as u32;
```

Or equivalently, use `div_ceil` since Rust 1.73:
```rust
let asc = (asc_fu as u32 * ppem as u32).div_ceil(upem as u32);
let desc = (desc_fu as u32 * ppem as u32).div_ceil(upem as u32);
```

**File:** `pillow-rs-freetype/src/font.rs` (~line 259)  
**C reference:** `FT_PIX_CEIL` macro in `include/freetype/internal/ftobjs.h`

---

## Fix 2: getbbox x-off-by-1 (12 failures) — 1 hour

### Problem

Bbox right edge differs by 1px for italic fonts. All 12 failures are in:
- DejaVuSerif-Italic (4)
- DejaVuSerifCondensed-Italic (5)
- DejaVuSerifCondensed-Bold (3)

```
FAIL [DejaVuSerif-Italic_16_91_getbbox]: bbox (-1, -2, 7, 13) != expected [0, -2, 7, 13]
FAIL [DejaVuSerifCondensed-Italic_16_47_getbbox]: bbox (-2, -2, 6, 12) != expected [-2, -2, 7, 12]
FAIL [DejaVuSerifCondensed-Bold_10_121_getbbox]: bbox (-1, -2, 5, 5) != expected [-1, -2, 6, 5]
```

### Root Cause

C's `af_loader_load_glyph` (afloader.c:419–530) adjusts phantom points
**even when horizontal edges are absent** (the `else` branch at lines 448–460):

```c
if ( axis->num_edges > 1 && AF_HINTS_DO_ADVANCE( hints ) )
{
    // ... compute pp1.x from edge positions ...
}
else
{
    FT_Pos  pp1x = loader->pp1.x;   // = hints->x_delta (= 0)
    FT_Pos  pp2x = loader->pp2.x;   // = advance_width
    loader->pp1.x = FT_PIX_ROUND( pp1x );   // = 0
    loader->pp2.x = FT_PIX_ROUND( pp2x );   // rounds advance to pixel
    slot->lsb_delta = loader->pp1.x - pp1x; // = 0
    slot->rsb_delta = loader->pp2.x - pp2x; // rounding delta
}
```

Our code (`latin.rs:792-831`) only adjusts phantom points when `num_horz_edges > 1`:

```rust
if num_horz_edges > 1 {
    // ... adjust pp1.x ...
}
// For italic fonts: num_horz_edges = 0 → SKIPPED
```

When horizontal hinting is skipped (italic fonts), our phantom-point adjustment
is not done, so the LSB x-coordinate is not pixel-rounded. This causes the
bbox.xMin to be off by 1 pixel.

### Fix

Add an `else` branch for the case `num_horz_edges == 0` (italic fonts):

```rust
if num_horz_edges > 1 {
    // existing phantom adjustment
} else if num_horz_edges == 0 {
    // NO_HORIZONTAL path: pixel-round advance width, adjust LSB x delta
    // C's afloader.c:448-460 — always adjust even without edges
    let old_lsb = 0i32;                       // x_delta = 0
    let pp1x_uh = 0i32;                      // no hinting delta
    let pp1x = (pp1x_uh + 32) & !63;         // FT_PIX_ROUND(0) = 0
    // pp2.x = FT_PIX_ROUND(advance_width) — but we skip advance adjustment
    // The LSB delta (pp1x - 0) = 0 → no translation
    // But C still applies slot->lsb_delta which affects bbox computation
}
```

Actually, the issue is different. C's `af_loader_load_glyph` always computes
phantom points even when there are no edges. The phantom points ARE computed
in C in the `else` branch at lines 455-460. But they're based on the unhinted
advance width, not hinting deltas.

The real fix: phantom points need to be computed for ALL fonts (not just when
num_horz_edges > 1), because the bbox computation in the scaler relies on them.

**File:** `pillow-rs-freetype/src/autohint/latin.rs` (~line 792)  
**C reference:** `afloader.c:440-460`

---

## Fix 3: getmask DDA precision (427 failures) — 4–8 hours

### Problem

SHA mismatches on rendered alpha bitmaps for:
- DejaVuSerif-Italic / Condensed-Italic: 326 failures
- DejaVuMathTeXGyre: 40 failures  
- DejaVuSans-ExtraLight: 30 failures
- DejaVuSerifCondensed-Bold: 25 failures
- NotoSerifDisplay-Bold/BoldItalic: 15 failures
- LiberationMono-Regular/Italic: 14 failures (includes metrics overlap)

All failing glyphs are characters with diagonal stems: A V W Y j v w y (bold/condensed),
plus specific chars in math/light fonts: . ; 0 D R b p O Q o q 5 B g l

### What matches C bit-for-bit

Verified by exhaustive testing and coordinate tracing:

1. **Outline coordinates after autohinting** — all 22 points of '0' at 12pt match C
2. **Bbox dimensions** — width and height always match expected
3. **FT_UDIV / FT_UDIVPREP** — exhaustive -4096..+4096 test passes
4. **ADD_INT wrapping arithmetic** — matches C's unsigned casting
5. **fill_rule** — shift 9, NOT, clamp 255 — identical
6. **LEFT_SHIFT for conic subdivision** — bit-reinterpretation, verified
7. **set_cell** — binary search insertion creates identical cell chains
8. **sweep** — per-cell area→coverage conversion matches

### What diverges

For non-pixel-snapped outlines (italic/extralight/math fonts), the
**integrated alpha values differ by 1–25 units** (out of 255).

The divergence is in `render_line` DDA stepping — specifically when
`fx1` (subpixel x fraction) is non-zero. The DDA uses four exit-face
conditions to step from cell to cell (left/up/right/down). Each step calls
`integrate` which accumulates `a * b` product into cell area. Small
differences in the `a` and `b` parameters compound across multiple DDA steps.

### Investigation approach

1. **Dump DDA trace for one glyph**  
   Pick `DejaVuSans-ExtraLight` 10pt 'O' (U+004F) — a single round glyph.
   Compare every `render_line` call's intermediate values between C and Rust.

2. **Instrument C's DDA**  
   Add `fprintf` instrumentation to C's `gray_render_line` (ftgrays.c:888–998)
   to dump every `prod`, `fx2`, `fy2`, and `INTEGRATE` call for the test glyph.
   Rebuild FreeType and capture trace.

3. **Instrument Rust's DDA**  
   Add matching `eprintln!` instrumentation to Rust's `render_line` in `grays.rs`.
   Capture trace for the same glyph.

4. **Binary search the first divergence**  
   Compare traces line by line. Find the first INTEGRATE call where
   `a`, `b`, `fx1`, `fy1`, or `prod` differ. That's the root cause.

5. **Fix the specific divergence point**  
   Apply targeted fix for the single value that diverges first.
   Re-run comparison to confirm all downstream values realign.

### Suspect areas

| Suspect | File:Line | What to check |
|---------|-----------|---------------|
| `prod` initialization | grays.rs:353 | `dx * fy1 - dy * fx1` — i64 vs FT_Int64 overflow |
| `prod` update formulas | grays.rs:355-401 | Left/up/right/down arithmetic: `prod -= dx * ONE_PIXEL` etc. |
| `ft_udiv` call chain | grays.rs:360 | `ft_udiv(-prod, -dx_r)` — sign handling |
| ONE_PIXEL constant | grays.rs:14 | `256` vs C's `(1 << 8)` — same value, verify |
| UPSCALE constant | grays.rs:17 | `4` vs C's `ONE_PIXEL >> 6` — same, verify |
| fract() | grays.rs:25 | `x & (ONE_PIXEL - 1)` = `x & 255` — same as C |
| trunc() | grays.rs:19 | `x >> PIXEL_BITS` — same as C |

**Files:** `pillow-rs-freetype/src/grays.rs` (render_line, line 288-410)  
**C reference:** `ftgrays.c:888–998` (FT_INT64 DDA path)

### Fallback strategy

If line-by-line tracing doesn't reveal the issue, compare the full cell
dump (cover, area per cell) between C and Rust for the test glyph.
If cells match, the sweep/fill-rule is the issue. If cells differ,
the DDA stepping is the issue.

---

## Execution Order

| Step | Fix | Effort | Cumulative Pass Rate |
|------|-----|--------|---------------------|
| 1 | getmetrics (Fix 1) | 15 min | 27,256/27,695 (98.42%) |
| 2 | getbbox phantom points (Fix 2) | 1 hr | 27,268/27,695 (98.46%) |
| 3 | DDA precision (Fix 3) | 4–8 hr | 27,695/27,695 (100%) |

**Total estimated time to 100%: 6–9 hours.**
