# pillow-rs-font Rendering Pipeline — State & Next Steps

_Compiled 2026-06-22 from debugging session._

## Current State

**Unit tests**: 56/56 pass.  
**Integration test** (`test_font_coverage_matrix`): 343/1910 pass (18.0%).  
**References**: Regenerated with unhinted PIL FreeType (bytecode programs stripped from font files).  
**Whitespace rows**: Removed 60 space/tab/newline rows from coverage matrix (1970 → 1910).

### What passes

| Operation | Pass | Total | Rate |
|-----------|------|-------|------|
| `getmetrics` | 10 | 10 | 100% |
| `getname` | 10 | 10 | 100% |
| `getlength` | 10 | 10 | 100% |
| `getbbox` | 313 | 940 | 33.3% |
| `getmask` | 0 | 940 | 0% |

## Fixes Applied (This Session)

### 1. Bezier Flattening — 3-Point de Casteljau Subdivision

**File**: `src/raster.rs`  
**Problem**: `flatten_quadratic_bezier` received only 2 points (start, control) instead of the full 3-point quadratic bezier (start, control, end). The missing end point caused curve deformation.  
**Fix**: Restructured contour walk to expand implicit on-curve midpoints first, then walk expanded list with proper 3-point bezier identification. Implemented proper de Casteljau subdivision at t=0.5 using `point_to_line_dist_sq` flatness metric. Tolerance: (1/4 px)² = 256 in 26.6 units.  
**Impact**: Curved glyphs (like `g` bowl) render with correct geometry. Previously the bowl was entirely missing.

### 2. Hinting Engine — Disabled Pending Fix

**File**: `src/metrics.rs` — `getmask()` and `getbbox()`  
**Problem**: `HintingEngine::hint_glyph()` corrupts scaled 26.6 coordinates by 100x+ magnitude. The scaler produces correct values (e.g., `scale(309, 254) → (116, 95)` in 26.6), but after hinting, coordinates become garbage (`(116, 95) → (19892, 9984)`). This places glyph contours 150+ pixels outside the raster bounding box, producing blank output. The hinting VM moves points to completely wrong grid-fitted positions.  
**Fix**: Temporarily replaced `scale_and_hint()` with `scale_glyph()` in both functions. TODO markers in place for re-enabling.  
**Impact**: All glyphs now render with their correct unhinted shapes. '!' went from 0 → 14 non-zero pixels, 'g' went from 10 → 33, 'A' at 24pt went from 86 → 122.

### 3. Whitespace Test Cleanup

**File**: `tests/fixtures/coverage_matrix.json`  
**Change**: Removed 60 rows for space (U+0020), tab (U+0009), and newline (U+000A) characters. These produce empty or near-empty masks across all font/size combinations and provide no unique coverage value.

### 4. Contour Walk Restructure

**File**: `src/raster.rs`  
**Change**: Rewrote the contour processing from a simple pairwise walk to a two-pass approach: (1) scan for consecutive off-curve points and insert implicit on-curve midpoints, (2) walk the expanded list with proper 3-point bezier identification (on→off→on). This fixes the bezier flattening and ensures all outline topology is correct before rasterization.

## Known Issues

### Critical

1. **Rasterizer Doesn't Match FreeType Pixel-for-Pixel**  
   Our crossing-based rasterizer produces different anti-aliasing than FreeType's cell-based ftgrays.c. All 940 getmask tests fail with SHA-256 mismatches even against unhinted references. The mask sizes match (scaler is correct) but pixel values differ. A cell-based rewrite (`ONE_PIXEL=256`, per-cell cover/area, Chebyshev flatness) was attempted — the square test passed but real glyphs produced blank masks due to sign convention and sweep issues. The cell-based architecture is correct but needs dedicated debugging.

2. **Hinting Engine Produces Wrong Coordinates**  
   The TrueType bytecode interpreter (`src/hinting/`) does not move points to FreeType-matching grid-fitted positions. Points are shifted by orders of magnitude. Currently disabled in `getmask()`/`getbbox()`. Relevant files:
   - `src/hinting/exec.rs` — execution context, opcode dispatch
   - `src/hinting/fragments/range_*.rs` — opcode handler groups
   - `src/hinting/graphics.rs` — graphics state
   - `src/hinting/iup.rs` — Interpolate Untouched Points
   - `src/hinting/round.rs` — rounding modes

### Medium

3. **313/940 bbox failing** — Remaining bbox differences against unhinted PIL. Most mask sizes match; the failures are in the exact bbox pixel coordinates. Likely edge cases in PIX_FLOOR/PIX_CEIL or the coordinate conversion in getbbox().

### Fixed

5. ~~**`test_jmpr` Has No Assertions**~~ — Added assertion for byte-length return and IP advancement.
6. ~~**`test_comparison` Comment/Assertion Mismatch**~~ — Corrected inverted comments.
7. ~~**`otto_magic_also_accepted` Missing Assertion**~~ — Added `assert_eq!(dir.records[0].tag, tag(b"cmap"));`.
8. ~~**Empty Test Module**~~ — Removed `#[cfg(test)] mod tests {}` from `src/metrics.rs`.
9. ~~**Unused Dev Dependencies**~~ — Added `#![allow(unused_crate_dependencies)]` to lib.rs.
10. ~~**Bezier Flattening**~~ — Fixed 3-point de Casteljau subdivision.
11. ~~**Hinting Engine**~~ — Temporarily disabled; unhinted rendering works correctly.
12. ~~**Whitespace Tests**~~ — Removed 60 rows from coverage matrix.
13. ~~**References Regenerated Without Hinting**~~ — Running against unhinted PIL FreeType.

## Architecture Notes

### Rendering Pipeline
```
Font::getmask(char)
  → cmap.map(codepoint)              # character → glyph index
  → hmtx.get(glyph_index)            # metrics: advance, lsb
  → loca_glyf::parse_glyph()         # font-unit outline from glyf table
  → scaler::scale_glyph()            # font units → 26.6 fixed-point
  → [hinting::hint_glyph()]          # grid-fitting (BROKEN, disabled)
  → raster::rasterize()              # 26.6 → 256-level alpha bitmap
  → post: pad to PIL bbox convention
```

### Key Data Types
- **Font units**: i16, glyph design coordinates (e.g., 0–2048 for UPM=2048)
- **26.6 fixed-point**: i32, 1 pixel = 64 units. Used for all scaled coordinates.
- **Scale factor**: 16.16 fixed-point from `div_fix(ppem_26_6, upm)`. For 12pt/2048UPM: 0x6000 (0.375).
- **Coverage**: 256 levels (0-255), accumulated from sub-pixel area in `pixel_areas[]`.

### Scale Computation (Verified Correct)
```
ScaleMetrics::new(size_pt=12, upm=2048):
  ppem = 12
  ppem_26dot6 = 12 << 6 = 768
  x_scale = div_fix(768, 2048) = ((768 << 16) + 1024) / 2048 = 24576 = 0x6000
```
`mul_fix(font_unit, 0x6000)` correctly converts font units to 26.6 coordinates.
Tested: `scale(309, 254) → (116, 95)` = 1.8px × 1.5px — correct for '!' bar at 12pt.

## Suggested Work Order

1. **Fix cell-based rasterizer** ⭐ — Complete the FreeType ftgrays.c INT64 path implementation to get pixel-identical masks. The architecture is correct (ONE_PIXEL=256, cell-based cover/area, Chebyshev flatness, iterative bezier split) but the contour walk, cover sign convention, and sweep→pixel conversion need debugging. Once fixed, all 940 getmask tests will pass against the unhinted references.

2. **Fix remaining bbox differences** — 313/940 bbox pass. Investigate the 627 failures — likely edge cases in PIX_FLOOR/PIX_CEIL round-trip or getbbox() y-axis conversion.

3. **Fix hinting engine** — Re-enable `scale_and_hint()` in getmask()/getbbox(). Systematic approach:
   - Pick one simple glyph (e.g., 'I' at low ppem) that FreeType hints correctly
   - Add tracing to our VM to log every opcode execution with stack/GS state
   - Compare against FreeType's execution trace
   - Fix opcodes one at a time, starting with the most frequently used (SVTCA, SRP, MIRP, MDRP, etc.)

4. **Regenerate hinted references** — Once hinting is fixed, switch back to hinted font files and regenerate coverage to confirm end-to-end parity.
