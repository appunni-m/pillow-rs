# 100% SHA-256 Parity — Master Implementation Plan

## Current Status (2026-07-01, end of session)

**Baseline at start:** 10,231/11,084 passed (853 failures, 92.3%)
**Current:** **10,601/11,084 passed (483 failures, 95.6%)**
**Progress:** **-370 failures (+3.3% pass rate)**
**Latin matrix:** 7,600/7,600 ✅
**All unit tests:** pass

## Fixes Applied

### Fix 1: top_to_bottom dimension gating (853→569, -284)
**Commit:** `8b9eb67`
**Bug:** `hint_edges` applied `top_to_bottom_hinting` to BOTH dimensions.
C gates it to VERT only (aflatin.c:4271-4273). HORZ edge BOUND checks used
wrong ordering for Indic scripts, collapsing stem edge positions.
**Fix:** `dim == Dimension::Vert &&` guard at latin.rs:1937.
**Scripts fixed to 100%:** beng, guru, goth, mong
**Scripts improved:** deva (9%→76% pass)

### Fix 2: blue zone outlier detection (569→483, -86)
**Commit:** `cce672e`
**Bug:** Without HarfBuzz GSUB, some script-specific standard characters
produce unshaped forms with wrong Y coordinates (e.g., knda saknda y=790
instead of shaped headline y=563). Blue zone reference picked the flat
median over the correct round median.
**Fix:** When flat/round medians differ >20% upem, trust rounds for top
zones and flats for bottom zones. Matches what HarfBuzz-shaped forms produce.
**Scripts fixed to 100%:** knda, gujr, lao, mlym, sinh, sund, taml

## Remaining 483 Failures (19 scripts)

### Heavy (15-72% fail rate)
| Script | FP | Fail% | Notes |
|--------|------|-------|-------|
| adlm | 91/126 | 72% | All in NotoSansAdlamUnjoined-Bold. Consistent 2px height offset. Edges match C's fpos/opos but pos differs (E2: 231 vs C 241, E4: 448 vs C 468). Root cause: edge positions differ post-hinting, likely Phase 4 non-stem edge fitting. |
| hani | 60/100 | 60% | All codepoint U+007C (|) in non-CJK fonts. Width always 1px narrower at 10pt. Standard character U+7530 missing → fallback widths match C (24 FU). Issue in stem width usage or phase 2 stem snapping. |
| nkoo | 32/90 | 36% | All in NotoSansNKo-Regular. Consistent width offset. Standard char exists. Widths match C. Issue in segment→edge chain. |
| deva | 35/144 | 24% | All in NotoSerifDevanagari-Regular. Small diff patterns (4-30px). Sizes match. 1-FU drift. |
| cher | 25/112 | 22% | Mixed fonts. Edge/link structure differences. |
| hebr | 48/252 | 19% | Mixed fonts. Moderate diffs. |

### Light (1-8% fail rate)
| Script | FP | Fail% | Notes |
|--------|------|-------|-------|
| geok | 49/666 | 7% | Small diff patterns |
| latp | 70/1010 | 7% | Subscript-specific blue zones |
| latb | 43/820 | 5% | Subscript-specific blue zones |
| saur | 2/24 | 8% | 2 failures |
| vaii | 1/20 | 5% | 1 failure |
| mymr | 6/150 | 4% | Small diff |
| cans | 10/370 | 3% | Small diff |
| telu | 2/84 | 2% | 2 failures |
| thai | 3/150 | 2% | 3 failures |
| medf | 2/124 | 2% | 2 failures |
| ethi | 1/72 | 1% | Single 20pt glyph, small diff |
| arab | 1/90 | 1% | 1 failure |
| geor | 2/528 | <1% | Same codepoint at 10+20pt |

## Prioritized Plan

### Phase 3: adlm edge position drift (91 failures → ~30)
The 2px height offset is consistent and likely has a single root cause.
C's hint_edges Phase 4 (non-stem edges) produces different pos values
than ours for adlm. Key edges to compare:
- E2: our pos=231 vs C pos=241 (diff=10 in 26.6)
- E3: our pos=367 vs C pos=387 (diff=20)
- E4: our pos=448 vs C pos=468 (diff=20)

### Phase 4: hani/codepoint-124 width issue (60 failures → ~10)
Root cause likely in stem width detection for vertical-bar glyph.
Standard char fallback yields same std=24 but Phase 2 snapping differs.

### Phase 5: nkoo/deva/cher/hebr edge alignment (140 failures → ~50)
Systematic edge/link differences for these scripts.

### Phase 6: latb/latp subscript (113 failures → ~30)
Needs HarfBuzz-free subscript detection or blue zone override.

### Phase 7: Long-tail cleanup (~60 failures across 8 scripts)
Individual 1-2-failure scripts plus geok (49).

## Debug Commands Reference

```bash
# C binary with trace output
FT2_DEBUG="aflatin:7" LD_LIBRARY_PATH=pillow-rs-freetype/freetype/build \
  /tmp/gen_refs_v4 <font.ttf> <CP_HEX> <size_pt>

# Our tracer
RUST_LOG=autohint::pipeline=trace \
  cargo run -p pillow-rs-freetype --example debug_glyph -- \
  <font.ttf> <size_pt> <CP_HEX>

# Full test suite
cargo test -p pillow-rs-freetype --test direct_ft_compare

# Build debug lib
cd pillow-rs-freetype/freetype/build
cmake .. -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_FLAGS="-DFT_DEBUG_LEVEL_TRACE"
cmake --build . -j$(nproc)
ln -sf libfreetyped.so.6 libfreetype.so.6
```
