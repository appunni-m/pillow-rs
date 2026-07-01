# 100% SHA-256 Parity — Master Implementation Plan

## Current Status (2026-07-01)

**Baseline:** 10,231/11,084 passed (853 failures, 92.3% pass rate)
**Latin matrix:** 7,600/7,600 ✅
**All unit tests:** pass

Failing scripts by severity:
- Complete failure (80-100%): deva, knda, beng, adlm, guru, hani, goth, mong
- High failure (15-80%): nkoo, cher, hebr
- Moderate failure (5-15%): saur, gujr, geok, latp, latb, vaii
- Low failure (1-4%): mymr, cans, telu, thai, medf, ethi, arab

## Pipeline Verification Status (Updated)

All pipeline functions match C for Latin scripts (verified by 7,600/7,600 passing).
The 853 remaining failures are in non-Latin scripts only.

```
✅ loader::reload              — matches C point coordinates
✅ compute_segments            — matches C segment detection
✅ link_segments_inner         — matches C scoring formula
✅ compute_edges               — matches C edge merging
✅ compute_blue_edges          — matches C blue zone assignment
✅ hint_edges (Phase 1-4)      — matches C for Latin
✅ align_edge_points           — matches C point snapping
✅ align_strong_points         — matches C interpolation
✅ align_weak_points           — matches C IUP
✅ vertical_separation         — matches C tilde/cedilla
```

## Empirical Investigation Results

### Hypothesis 1: Override LATB/LATP → LATN for shared glyphs (Phase 1)
**Result: NO EFFECT.** Noto fonts have dedicated subscript/superscript glyph forms.
No shared glyph indices between LATB/LATP and LATN were detected.
Test result unchanged: 853 failures.

### Hypothesis 2: Always use LATN blue zones for LATB/LATP
**Result: CATASTROPHIC.** 2,515 failures. Dedicated subscript glyphs need their
own blue zones; forcing LATN zones produces wrong edge assignments.

### Hypothesis 3: Use Latin 'o' (U+006F) for all scripts' standard widths
**Result: CATASTROPHIC.** 6,758 failures. Each script MUST use its own standard
character for stem width detection. Script-specific outlines produce
fundamentally different stem widths.

### Hypothesis 4: major_dir = Direction::Up for top_to_bottom VERT
**Result: WORSE.** 954 failures. Overriding VERT major_dir breaks segment
direction matching. The segment linking code already handles top_to_bottom
correctly for edge sorting.

### Hypothesis 5: Invert compute_blue_edges enter condition
**Result: CATASTROPHIC.** 9,590 failures. The current enter condition is correct.

## Root Cause Analysis

### C trace comparison for Bengali U+0995 at 10pt (NotoSansBengali-Regular)

Edge positions match C exactly:
- VERT (horizontal edges): fpos=622, 551, 233, 159, 0 ✅
- HORZ (vertical edges): fpos=761, 683, 492, 416 ✅

Edge links match C: (HE0↔HE1) and (HE2↔HE3) stem pairs ✅

BUT hint_edges output diverges:
- C: HE2.pos=454 (opos=437, snapped to 7.09px → 454 in 26.6)
- Our: HE2.pos=314 (collapses to HE1.pos)

**Root cause:** Phase 1 blue-zone alignment marks edges E0+E1 as DONE for VERT
dimension. Then Phase 2 uses E2 as the first anchor (since E0+E1 are DONE),
producing wrong relative-stem positions for E2+E3.

C does NOT mark these edges as DONE in Phase 1 because C's blue zone
assignment differs for VERT dimension on Bengali scripts. The standard
character for Bengali (U+09E6) produces stem widths that are close to
but NOT identical between C and our code:

```
C:   horizontal widths = 71 fu, vertical widths = 81 fu
Our: horizontal widths = 81 fu, vertical widths = 71 fu  (SWAPPED!)
```

The dimension swap in standard width computation propagates through
compute_stem_width → wrong stem snapping → different edge positions.

### Validation

Scaled standard widths at 10pt (upem=1000, scale≈0.64):
```
          C (expected)    Our (actual)
HORZ:     45 (26.6)       52 (26.6)   ← matches C's VERT value
VERT:     52 (26.6)       44 (26.6)   ← close to C's HORZ value
```

Our HORZ axis has the VERT-axis standard width, and vice versa.
This swap originates in `metrics_init_widths` where the standard
character outline processing assigns widths to the wrong axis.

## Corrected Implementation Plan

### Phase A: Fix standard width dimension swap
1. In `metrics_init_widths` at latin.rs:144, verify the stem width
   collection loop assigns widths to the correct axis dimension.
2. The `for dim in 0..2` loop should store HORZ widths in axis[0] and
   VERT widths in axis[1], matching C's AF_DIMENSION_HORZ=0 and
   AF_DIMENSION_VERT=1.
3. Verify by comparing trace output against C's standard width values.

### Phase B: Trace remaining 1-FU drift failures
For scripts with 1-4% fail rate, the standard width fix should resolve
most edge-position differences. Remaining failures are likely from:
- Sub/superscript blue zone differences (113 failures: latb+latp)
- Rounding/quantization differences in stem snapping
- Edge case outlines where the standard width swap has cascading effects

### Phase C: Per-script verification
After Phase A fix, re-run the full test suite and categorize remaining
failures by script/root cause.

## Verification Strategy

After each phase:
1. `cargo test -p pillow-rs-freetype --test direct_ft_compare`
2. Verify no regression on Latin matrix (7,600/7,600)
3. Verify per-script pass rates improve
4. Commit with detailed before/after analysis
