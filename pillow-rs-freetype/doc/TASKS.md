# Task List — pillow-rs-freetype PIL / FreeType Parity

## Current State
- PIL backend: 1170/1910 (61.3%) — 740 failures
- FreeType backend: 235/1910 (12.3%) — 1675 failures
- Vendored FreeType: 2.14.3 (matches PIL 12.2.0)
- References: externally generated (PIL getmask/getbbox + system FT_LOAD_RENDER)

## Root Cause Analysis

**All 740 PIL failures and 1665 FreeType failures come from ONE source:**
our Rust autohinter port produces different edge positions than real FreeType.

Breakdown:
- 663 SHA-only failures: different edge positions → different pixel coverage
- 77 bbox failures: different edge positions → different bitmap bounds

The bbox assembly formulas are structurally correct. The mask assembly is correct.
The `BitmapBackend` dispatch is correct. The reference matrices are from real PIL/FreeType.

**The entire gap is the autohinter.** Our port targets FreeType 2.14.1 algorithms.
PIL bundles FreeType 2.14.3. Every file in `src/autofit/` changed between versions.

## Autohinter Sub-Component Status

| Component | Source file | Status |
|-----------|------------|--------|
| Glyph outline loading | loader.rs (afhints.c) | Ported (2.14.1) |
| Blue zone detection | latin.rs (aflatin.c:311-1039) | Ported (2.14.1) |
| Segment computation | latin.rs (aflatin.c:1557-2008) | Ported (2.14.1) |
| Edge detection + linking | latin.rs | Ported (2.14.1) |
| Edge hinting (Phase 1-4) | latin.rs (aflatin.c:4214-4831) | Ported (2.14.1) |
| Edge-point alignment | latin.rs | Ported (2.14.1) |
| Strong-point interpolation | latin.rs | Ported (2.14.1) |
| Weak-point interpolation | latin.rs | Ported (2.14.1) |
| Phantom-point advance | NOT PORTED (afloader.c:395-490) | Missing |
| Width computation | latin.rs (aflatin.c:55-265) | Ported (2.14.1) |

## Required Work

### 1. Diff 2.14.1 → 2.14.3 autofit (Analysis)
The algorithmic changes are overflow-safety macros:
- `a - b` → `SUB_LONG(a, b)` 
- `a + b` → `ADD_LONG(a, b)`
- `a * b / d` → `MUL_LONG(a, b) / d`
- `FT_PIX_ROUND(x)` → `FT_PIX_ROUND_LONG(x)` for tilde handling

These don't change the algorithm — they're for 16-bit compatibility.
**Net: no algorithm changes needed.** The port is algorithm-complete.

### 2. Trace Edge Positions (Debugging)
The actual bugs must be in implementation details:
- Different edge sort order
- Edge collapse/dedup logic
- Blue zone assignment priority
- Stem width computation
- Segment angle filtering

**Approach:** Build FreeType 2.14.3 with debug tracing, extract per-glyph edge positions,
compare with our Rust output for specific failing glyphs.

### 3. Fix Specific Mismatches
For each failing glyph category:
1. Extract edge list from FreeType C (left/right edges with positions)
2. Extract edge list from our Rust port
3. Identify first point of divergence
4. Fix the corresponding code path
5. Re-run tests to measure improvement

### 4. Phantom-Point Advance
FreeType adjusts advance width based on hinted edge positions (afloader.c:395-490).
Without this, our advance values differ from FreeType's autohinted advance.
This causes ~25 "right" bbox failures.

## Architecture (Correct)

```
Font::truetype(data, size_pt, backend)
  ├─ BitmapBackend::PIL
  │   ├─ getmask() → autohint + rasterize + pad to ascender/descender
  │   ├─ getbbox() → autohint + PIL screen coords
  │   └─ tests: coverage_matrix.json (PIL 12.2.0 refs)
  │
  └─ BitmapBackend::FreeType
      ├─ getmask() → autohint + rasterize (raw)
      ├─ getbbox() → autohint + FreeType coords
      └─ tests: coverage_matrix_ft.json (system FT refs)
```
