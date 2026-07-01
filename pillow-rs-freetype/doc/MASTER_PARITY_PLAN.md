# 100% SHA-256 Parity — Master Implementation Plan

## Current Status (2026-07-01, EOD)

**Start:** 10,231/11,084 passed (853 failures, 92.3%)
**Current:** **10,802/11,084 passed (282 failures, 97.5%)**
**Progress:** **-571 failures (+5.2% pass rate)**
**Latin matrix:** 7,600/7,600 ✅

## Fixes Applied (5 commits)

| Commit | Fix | Delta | Scripts fixed to 100% |
|--------|-----|-------|----------------------|
| `8b9eb67` | top_to_bottom gating to VERT only | -284 | beng, guru, goth, mong |
| `cce672e` | Blue zone outlier detection | -86 | knda, gujr, lao, mlym, sinh, sund, taml |
| `c899649` | Standard char fallback `['o','O','0']` | 0 | Preventive only |
| `6dc884f` | Per-script non-base glyph detection | -111 | adlm, saur, mymr |
| `c94f379` / `409c7c7` | Skip hinting when blue_count==0 | -90 | hani, nkoo (partial) |
| **Total** | | **-571** | **16 scripts** |

## Remaining 282 Failures (15 scripts)

### Category A: 1-FU pixel drift (219 tests)
Scripts: cher (25), hebr (48), deva (23), geok (49), latp (70), latb (43), vaii (1)
All have matching bbox, small diff count, low avg_diff (<25px).
Root cause: edge positions match C exactly (verified for multiple glyphs),
but pixel values differ. Likely in the rasterization path or stem width
snapping produces slightly different edge.pos values.

**Fix approach:** Per-glyph per-stage C trace comparison using
`FT2_DEBUG="aflatin:7" /tmp/gen_refs_v7`. Compare edge positions at
each phase (INITIAL, PHASE1, PHASE2, FINAL).

### Category B: Size mismatches (63 tests)
Scripts: cher (some), cans, telu, thai, medf, ethi, arab, geor, nkoo (2)
Width/height differs by 1-2px. Likely bbox computation or scaler
rounding differences.

**Fix approach:** Trace `scale_glyph` bbox computation vs C's
`FT_Glyph_Get_CBox` + `FT_PIX_FLOOR`/`FT_PIX_CEIL`.

## Key Architectural Gaps

1. **HarfBuzz GSUB**: C uses HarfBuzz to reshape subscript/superscript
   glyphs via 'subs'/'sups' GSUB features. Without this, latb/latp glyphs
   get wrong outlines → wrong blue zones.
2. **Rasterizer**: Our `grays::rasterize` may produce slightly different
   antialiasing than FreeType's renderer for certain edge configurations.
3. **Stem width quantization**: `sort_and_quantize_widths` might cluster
   widths differently than C's `af_sort_and_quantize_widths` for
   multi-width scripts like cher.

## Debug Commands

```bash
# Rebuild C reference binary with trace-enabled lib
cd pillow-rs-freetype/freetype/build
cmake .. -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_FLAGS="-DFT_DEBUG_LEVEL_TRACE"
cmake --build . -j$(nproc)
cd /home/appunni/work/pil-wasm
gcc -o /tmp/gen_refs_v7 /tmp/gen_refs_v2.c \
  -Ipillow-rs-freetype/freetype/include \
  -Lpillow-rs-freetype/freetype/build \
  -Wl,-rpath,$(pwd)/pillow-rs-freetype/freetype/build \
  -lfreetyped -lm -lz

# C trace with per-stage edge dump
FT2_DEBUG="aflatin:7" /tmp/gen_refs_v7 <font.ttf> <CP_HEX> <size_pt>

# Our trace
RUST_LOG=autohint::pipeline=trace \
  cargo run -p pillow-rs-freetype --example debug_glyph -- \
  <font.ttf> <size_pt> <CP_HEX>

# Full test suite
cargo test -p pillow-rs-freetype --test direct_ft_compare
```
