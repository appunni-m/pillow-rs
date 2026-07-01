# 100% SHA-256 Parity — Master Implementation Plan

## Current Status (2026-07-01, session end)

**Start:** 10,231/11,084 passed (853 failures, 92.3%)
**Current:** **10,712/11,084 passed (372 failures, 96.6%)**
**Progress:** **-481 failures (+4.3% pass rate)**
**Latin matrix:** 7,600/7,600 ✅

## Fixes Applied

### Fix 1: top_to_bottom dimension gating (853→569, -284)
`8b9eb67` — `hint_edges` applied `top_to_bottom_hinting` to BOTH dimensions.
C gates to VERT only (aflatin.c:4271-4273).
Scripts fixed: beng, guru, goth, mong.

### Fix 2: blue zone outlier detection (569→483, -86)
`cce672e` — Without HarfBuzz, unshaped standard chars produce wrong Y.
Scripts fixed: knda, gujr, lao, mlym, sinh, sund, taml.

### Fix 3: standard char fallback chain (no immediate change)
`c899649` — C's "o O 0" for latn: try 'o', then 'O', then '0'.

### Fix 4: per-script non-base glyph detection (483→372, -111)
`6dc884f` — C skips `compute_blue_edges` for non-base glyphs.
Our non_base_glyphs missed per-script RANGES_*_NONBASE_UNI.
Fixed: corrected generated data + scan all STYLE_TABLE non_base_ranges.
Scripts fixed: adlm, saur, mymr. deva improved.

## Remaining 372 Failures (16 scripts)

| Script | Fail | Rate | Notes |
|--------|------|------|-------|
| hani | 60/100 | 60% | U+007C pipe in non-CJK fonts. Bbox matches C, pixel diff only |
| nkoo | 32/90 | 36% | NotoSansNKo-Regular, width offset |
| cher | 25/112 | 22% | Mixed fonts |
| hebr | 48/252 | 19% | Mixed fonts |
| deva | 23/144 | 16% | NotoSerifDevanagari, small diffs |
| geok | 49/666 | 7% | DejaVuSerif-Bold, consistent diffs |
| latp | 70/1010 | 7% | Superscript blue zones |
| latb | 43/820 | 5% | Subscript blue zones |
| vaii | 1/20 | 5% | Single glyph |
| cans | 10/370 | 3% | Small diffs |
| telu | 2/84 | 2% | 2 glyphs |
| thai | 3/150 | 2% | 3 glyphs |
| medf | 2/124 | 2% | 2 glyphs |
| ethi | 1/72 | 1% | Single 20pt glyph |
| arab | 1/90 | 1% | 1 glyph |
| geor | 2/528 | <1% | Same codepoint at 10+20pt |

## Prioritized Remaining Phases

### Phase 5: nkoo segment/edge detection (32 failures)
Likely same class as adlm — blue zone or non-base issue.

### Phase 6: hani pixel-level diff (60 failures)
U+007C pipe glyph. Edge positions match C. Purely rasterization diff.

### Phase 7: latb/latp HarfBuzz-aware detection (113 failures)
Needs GSUB feature detection to match C's reshaped output.

### Phase 8: Long tail cleanup (~70 failures)
Individual 1-50 failure scripts. Trace per-failing glyph.

## Debug Commands

```bash
# C binary with trace output
LD_LIBRARY_PATH=pillow-rs-freetype/freetype/build \
  /tmp/gen_refs_v5 <font.ttf> <CP_HEX> <size_pt>

# Our tracer
RUST_LOG=autohint::pipeline=trace \
  cargo run -p pillow-rs-freetype --example debug_glyph -- \
  <font.ttf> <size_pt> <CP_HEX>

# Full test suite
cargo test -p pillow-rs-freetype --test direct_ft_compare
```
