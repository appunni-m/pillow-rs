# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current Baseline (end of 2026-06-27)

| Backend | Pass | Total | Rate |
|---------|------|-------|------|
| PIL | 1546 | 1910 | 80.9% |
| FreeType raw | 1588 | 1910 | 83.1% |

Session: 13 commits. All 22 core functions verified against C.
ft_div_fix sign rounding fix applied (matches C FT_DivFix signed division).

## Remaining: 364 PIL / 322 FT

| Type | PIL | FT | Root cause analysis |
|------|-----|-----|---------------------|
| getmask SHA | 339 | 295 | Subpixel IUP precision + font-specific native hinter diffs |
| getbbox | 25 | 17 | Y-axis ±1px: compute_stem_width snapping with x-height-adjusted widths |
| getlength | 0 | 10 | FT fixture values wrong (0.56px for "hello") |

## Corrected findings from 2026-06-27 investigation

### TASKS.md 194 FU claim — MISATTRIBUTED
Previous TASKS.md claimed: "C detects VERT stem-pair distance of 194 FU" for LiberationSerif 'o'.
**Actual finding:** 194 FU is DejaVuSans HORZ (vertical stem width), which our code computes correctly.
LiberationSerif VERT stdw=79 is geometrically correct (horizontal stem width of 'o').

DejaVuSans widths (dump_metrics):
- HORZ=194 (vertical stem: left-right thickness of 'o')
- VERT=156 (horizontal stem: top-bottom thickness of 'o')

LiberationSerif widths (trace_segments_o):
- HORZ=180, VERT=79

### cw_orientation + major_dir verified correct
- cw_orientation = (area < 0) matches C's FT_Outline_Get_Orientation → FT_ORIENTATION_POSTSCRIPT
- For CW (area<0, PostScript): major_dir = Up(HORZ)/Left(VERT) — no flip, matches C default
- For CCW (area>0, TrueType): major_dir = Down(HORZ)/Right(VERT) — flip, matches C
- LiberationSerif 'o' has CW winding → cw=true → major_dir=Left(VERT), Up(HORZ) ✓

### ft_div_fix fix: sign-stripping → signed division
Fixed `ft_div_fix` to use direct signed division matching C's `FT_DivFix`.
Old: sign-stripped positives, then negated (rounds wrong for negative a).
New: `((a as i64) << 16) + ((b as i64) >> 1)) / (b as i64)` — matches C.
Pass rate unchanged (80.9%/83.1%) — 1-unit diff doesn't affect pixel comparisons.

### getbbox failures: compute_stem_width snapping
The 25 PIL / 17 FT getbbox failures are from compute_stem_width snapping
with x-height-adjusted VERT cur values. For LiberationSerif at 10pt:
- VERT stdw.org=79 → cur=ft_mul_fix(79, v_scale_adjusted) → small cur value
- Small cur makes stem snapping too aggressive (e.g., 72→48 instead of preserving 72)

## What was found today

### Fixed
- **ft_div_fix sign rounding**: now matches C FT_DivFix signed division (commit pending)
- **GlyphMask field restoration**: added back xmin, ymin, advance_width fields
- **BitmapBackend re-export**: added to pillow-rs-font façade
- **Build fixes**: pillow-rs font/mod.rs updated for new Font::truetype API

### Confirmed
- All 22 core autohinter functions verified against C
- Segment detection + linking geometrically correct for 'o' at identity scale
- major_dir logic verified against C's afhints.c:967-974
- cw_orientation matches C's FT_Outline_Get_Orientation

### Not yet fixed
- VERT stdw for LiberationSerif is 79 (geometrically correct), but x-height-adjusted cur
  leads to aggressive stem snapping → getbbox ±1px errors
- Subpixel IUP precision differences (pixel-level, not structural)
- `compute_stem_width` smooth path: |delta|<40 check too narrow with small cur values

## What would close remaining gap

To reach 95%+:
1. **Fix compute_stem_width snapping**: With stdw cur=26, delta=|72-26|=46 > 40 threshold.
   C's cur=61 gives delta=11 < 40 → snaps correctly. Either fix cur computation
   (x-height scaling) or relax the |delta|<40 threshold for small stdw values.
2. **IUP precision**: Byte-level integer division parity in IUP interpolation
3. **Font-specific hints**: Some fonts have native instructions that affect the
   raw FreeType reference — these are not autohinter bugs
