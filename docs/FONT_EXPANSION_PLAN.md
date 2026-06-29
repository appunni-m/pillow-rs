# Font Expansion Plan: 100-Family Matrix → Minimal Covering Set

## Objective

Expand the autohinter test matrix from 7 to ~100 diverse font families,
fix all discovered FT parity gaps, then use code-coverage-based similarity
analysis to identify the minimal font subset that exercises 100% of
autohinter code paths.

## Current State

- 7 font families in `pillow-rs-freetype/tests/fixtures/input/fonts_autohint/`
- FT parity: 6670/6685 (99.8%) — 15 DejaVuSansCondensed gaps
- Fixtures regenerated from C FreeType 2.14.3 binary
- Test matrix: `<font>_<size>_<codepoint>_<operation>` rows

## Design Decisions Recorded

| Decision | Rationale |
|----------|-----------|
| All fixture references from C FreeType 2.14.3 binary | Self-referential fixtures prove nothing |
| Only TrueType outline fonts (.ttf with glyf/loca tables) | CFF/PostScript outline parser not yet implemented |
| ASCII 33-126 coverage required | Latin autohinter is the scope |
| Code-coverage-based minimization | Keeps test suite fast while maintaining exhaustiveness |
| Every fix adds a C-verification annotation | Prevents re-investigation during future expansion |

## Phase 1: Source 100+ Diverse Fonts

### Font Selection Criteria
- **Outline format**: TrueType (glyf/loca) — CFF fonts excluded until CFF parser exists
- **Coverage**: Complete ASCII 33-126 required
- **Diversity dimensions**:
  - Foundry (DejaVu, Liberation, Noto, Ubuntu, URW, FreeFont, Microsoft, Apple, etc.)
  - UPEM (1000, 1024, 2048, 4096)
  - Style (sans, serif, mono, condensed, expanded, display, handwriting)
  - Weight (thin, light, book, regular, medium, bold, heavy, black)
  - Creation era (1990s, 2000s, 2010s, 2020s)
  - x-height and cap-height variation
  - Stem width and contrast variation

### Source Directories
- /usr/share/fonts/truetype/dejavu/
- /usr/share/fonts/truetype/liberation/
- /usr/share/fonts/truetype/noto/
- /usr/share/fonts/truetype/ubuntu/
- /usr/share/fonts/truetype/freefont/
- /usr/share/fonts/opentype/urw-base35/ (OTF with TrueType outlines only)
- Any additional system fonts

### Steps
1. Find all .ttf and .otf files on the system
2. Copy unique fonts (deduplicated by file hash) to staging directory
3. Compile metadata CSV: name, foundry, UPEM, style, weight, creation year, glyph count, ASCII coverage

## Phase 2: Filter & Validate

### Steps
1. Check each font for ASCII 33-126 coverage using C FreeType `FT_Get_Char_Index`
2. Verify Rust `Font::truetype()` loads each font without panic/error
3. Remove fonts that fail either check
4. Goal: 100 fonts remaining after filtering

## Phase 3: Regenerate Fixtures

### Steps
1. Update `scripts/gen_ft_matrix.py` FONTS dict with all qualifying fonts
2. Rebuild C FreeType 2.14.3 binary (`scripts/build_ft.sh`)
3. Run `python3 scripts/gen_ft_matrix.py` to generate `coverage_matrix_ft.json`
4. Expected: ~100 families × 5 sizes × 94 chars ≈ 47,000 rows

## Phase 4: Identify & Fix All Gaps

### Approach
1. Run `cargo test test_font_coverage_matrix_freetype -- --nocapture`
2. Export failure list to `/tmp/ft_gaps.txt`
3. Categorize failures by root cause pattern:
   - `compute_stem_width` rounding at specific PPEM/UPEM combinations
   - Direction chain NEAR-point artifacts
   - Phase 2 BOUND check edge cases
   - Phase 4 serif alignment overcorrection
   - Vertical separation for dot-above glyphs
4. For each category, pick one representative glyph
5. Add C fprintf instrumentation to trace the exact numeric values
6. Apply targeted fix with C-verification annotation
7. Re-run full test suite to confirm fix and check for regressions
8. Repeat until all gaps closed

### Debugging Protocol (per established pattern)
1. Trace C vs Rust per-phase edge positions for one failing glyph
2. Find first divergent value (Phase N, edge M, C=X, Rust=Y)
3. Identify C source line and Rust equivalent
4. Implement exact C logic in Rust
5. Verify fix on the representative glyph
6. Re-run full suite

## Phase 5: Code-Coverage Similarity Analysis

### Concept
Two fonts are "similar" for our purposes if they exercise the same
autohinter code paths across all glyphs. We want the smallest set of
fonts that collectively exercises every code path.

### Approach: Instrumentation-Based Coverage Matrix

1. **Instrument the Rust autohinter**: Add a global `HashSet<&'static str>`
   that records which code paths are hit during processing of each glyph.

   Key code paths to trace:
   ```
   - compute_segments: direction assignment (Up/Down/Left/Right/None)
   - compute_edges: segment filtering (height check, serif filter, directionless pass)
   - compute_edges: link/serif propagation (link, serif, both)
   - compute_blue_edges: blue assignment (capital top/bottom, small top/bottom, neutral)
   - hint_edges Phase 1: blue-zone alignment (with/without link, neutral dedup)
   - hint_edges Phase 2: stem alignment (anchor stem, relative stem, BOUND)
   - hint_edges Phase 3: lowercase m symmetry (6 edges, 12 edges)
   - hint_edges Phase 4: serif handling, anchor-relative, interpolation
   - compute_stem_width: serif short-circuit, round-edge, thin clamp, standard-width match, fractional quant, bdelta
   - align_strong_points: before-first, after-last, exact-match, interpolation
   - iup_shift / iup_interp: single-reference shift, dual-reference interp
   - vertical_separation_adjustments: adjustment applied, not applied
   ```

2. **Generate per-glyph coverage vectors**: For each font/size/glyph
   combination, record which code paths were exercised.

3. **Build N×N similarity matrix**: For each pair of fonts, compute
   Jaccard similarity of their code path sets across all glyphs.

4. **Greedy set cover**: Start with empty set, iteratively add the font
   that covers the most uncovered code paths until coverage reaches 100%.

5. **Verify**: Run full FT parity on the minimal set to confirm 100% passes.

## Phase 6: Trim & Finalize

### Steps
1. Remove fonts not in the minimal covering set from fixtures
2. Remove unused font files from `fonts_autohint/`
3. Regenerate `coverage_matrix_ft.json` for the minimal set
4. Run final FT parity test — must be 100%
5. Commit with summary of which fonts were kept and why

### Expected Outcome
- 10-15 fonts in the minimal set (down from 100)
- 100% FT parity
- Test suite runs in < 5 seconds
- Every code path exercised by at least one font

## Timeline Estimate

| Phase | Effort |
|-------|--------|
| Phase 1: Source fonts | 30 min |
| Phase 2: Filter & validate | 30 min |
| Phase 3: Regenerate fixtures | 60 min |
| Phase 4: Fix all gaps | 3-6 hours |
| Phase 5: Coverage analysis | 2-3 hours |
| Phase 6: Trim & finalize | 30 min |

## Acceptance Criteria

- [ ] 100+ font families sourced with diverse characteristics
- [ ] All fonts pass ASCII 33-126 coverage + Rust loading
- [ ] FT parity matrix generated from C FreeType 2.14.3 for all fonts
- [ ] All autohinter gaps identified and fixed
- [ ] Code-coverage similarity matrix computed
- [ ] Minimal covering font set identified (exercises 100% of code paths)
- [ ] Final fixture set trimmed to minimal covering set
- [ ] Final FT parity: 100% across the minimal covering set
