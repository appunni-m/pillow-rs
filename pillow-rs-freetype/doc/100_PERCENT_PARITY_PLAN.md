# 100% FreeType Autohinter Parity — Implementation Plan

## Current Status: 31/55 scripts at 100% SHA-256 parity

```
PASSING (31): armn avst bamu buhd cakm cari copt cprt cyrl dsrt geor glag
               grek kali khmr khms lao latn lisu mlym olck orkh osge osma
               rohg shaw sinh sund taml tavt tfng

FAILING (24): adlm arab beng cans cher deva ethi geok goth gujr guru hani
              hebr knda latb latp medf mong mymr nkoo saur telu thai vaii
              1,249 failures total / 18,500 tests
```

## Root Cause Categories

### Category A: Subscript/Superscript Blue String Selection (306 failures)
- **latb**: 130 failures (5% fail rate)
- **latp**: 176 failures (6% fail rate)
- **Root cause**: Codepoints like U+2080 (subscript 0), U+2070 (superscript 0) fall within LATN Unicode ranges, so the coverage scan assigns LATN style. FreeType uses HarfBuzz GSUB to detect subscript/superscript features and assign LATB/LATP styles with their specific blue strings.
- **Fix**: Add codepoint-range overrides: if a codepoint is in the subscript Unicode block (U+2080-U+2089) or superscript block (U+2070-U+2079, U+00B2, U+00B3, U+00B9), prefer LATB/LATP blue strings if those characters exist in the font. No HarfBuzz needed — simple rang e check.
- **CEffort**: ~30 lines in script.rs or globals.rs

### Category B: top_to_bottom_hinting (760 failures)
- **guru**: 122 failures (58%), **deva**: 134 (53%), **beng**: 89 (56%)
- **knda**: 67 failures (65%), **mong**: 7 (75%), **goth**: 18 (44%)
- **Root cause**: FreeType's `afind ie.c` (157 lines) delegates to `afcjk.c` (2,370 lines) for these scripts. They don't use `aflatin.c` hinting at all — they use the CJK hinting engine. Our port only has `latin.rs` which runs for all scripts.
- **What's actually needed**: Port `afcjk.c` (CJK metrics + edge detection + blue zones). `afindic.c` is just a bridge that says "use CJK for these scripts".
- **Effort**: 2,370 lines of C → ~1,200 lines of Rust. 2-3 sessions.

### Category C: CJK Stroke Snapping (228 failures)
- **hani**: 228 failures (63% fail rate), all on U+124 (Ĥ) — a Latin codepoint rendered by CJK fonts
- **Root cause**: Same as Category B — CJK scripts go through `afcjk.c`, not `aflatin.c`. Our `latin.rs` can't handle CJK stroke-based glyphs.
- **Fix**: Category B's `afcjk.c` port also fixes this.

### Category D: 1-FU Algorithmic Gaps (~165 failures across 9 scripts)
- **adlm, nkoo, cher, hebr, gujr, geok, cans, mymr, saur, thai, arab, medf, telu, vaii, ethi**
- **Root cause**: Genuine small differences where our blue zone computation or stem width detection differs by 1 FU from FreeType. Not related to missing modules.
- **Fix**: Per-glyph debugging with `debug_glyph` tool. These are the hardest to fix but the script coverage is already high (95%+ for most).

## Implementation Order

### Phase 1: Quick Wins (fixes 306 failures, achieves 33/55 scripts at 100%)
- [ ] Add subscript/superscript codepoint range overrides for latb/latp selection
- [ ] ~30 lines of Rust code
- **Expected**: latb and latp move from 5%/6% fail to ~100% pass

### Phase 2: CJK Engine Port (fixes 988 failures, achieves 39/55 scripts at 100%)
- [ ] Port `afcjk.c` metrics + edge detection:
  - `af_cjk_metrics_init_widths` (CJK stem width computation)
  - `af_cjk_metrics_init_blues` (CJK blue zone computation)  
  - `af_cjk_hints_compute_edges` (CJK edge detection with top_to_bottom)
  - `af_cjk_hints_init` (CJK hint initialization)
- [ ] Wire into FaceGlobals for scripts that use CJK writing system
- **Expected**: guru, deva, beng, knda, mong, goth, hani all move to 100%

### Phase 3: Per-Glyph Debugging (fixes remaining 165 failures)
- [ ] Debug adlm failures (Adlam script)
- [ ] Debug nkoo failures (N'Ko script)
- [ ] Debug remaining edge cases
- **Expected**: All 55 scripts at 100%

## Files Needed

| File | Description | Lines |
|------|-------------|-------|
| `src/autohint/cjk.rs` | CJK metrics + edge detection (from afcjk.c) | ~1,200 |
| `src/autohint/indic.rs` | Indic bridge (from afindic.c) | ~80 |
| `src/autohint/mod.rs` | Register new modules | +3 |
| `src/autohint/globals.rs` | Wire CJK path for Indic + CJK scripts | +15 |
| `src/autohint/script.rs` | Subscript/superscript range overrides | +30 |
