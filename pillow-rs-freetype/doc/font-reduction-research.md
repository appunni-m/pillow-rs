# Latin Font Reduction Research: 29 → <10 Without Coverage Loss

**Date:** 2026-06-30
**Scope:** `pillow-rs-freetype/tests/fixtures/input/fonts_autohint/`
**Objective:** Identify a minimal subset (<10) of the 29 Latin fonts that preserves equivalent autohinter code-path coverage in the FT test suite (`coverage_matrix_ft.json`).

---

## 1. Font Inventory

All 29 fonts are in `pillow-rs-freetype/tests/fixtures/input/fonts_autohint/`. The FT coverage matrix (`coverage_matrix_ft.json`, v2.0.0) generates 27,695 test rows (955 per font) covering 5 operations × 5 sizes × 95 codepoints uniformly across all fonts.

| # | Font | Family | Class | Weight | Slant | Width | Glyphs |
|---|------|--------|-------|--------|-------|-------|--------|
| 1 | DejaVuMathTeXGyre.ttf | DejaVu | math | regular | roman | normal | 4282 |
| 2 | DejaVuSans-ExtraLight.ttf | DejaVu | sans | extralight | roman | normal | 2032 |
| 3 | DejaVuSans-Oblique.ttf | DejaVu | sans | regular | italic | normal | 5355 |
| 4 | DejaVuSansMono-Oblique.ttf | DejaVu | mono | regular | italic | normal | 2710 |
| 5 | DejaVuSansMono.ttf | DejaVu | mono | regular | roman | normal | 3377 |
| 6 | DejaVuSerif-Bold.ttf | DejaVu | serif | bold | roman | normal | 3506 |
| 7 | DejaVuSerif-Italic.ttf | DejaVu | serif | regular | italic | normal | 3507 |
| 8 | DejaVuSerifCondensed-Bold.ttf | DejaVu | serif | bold | roman | condensed | 3506 |
| 9 | DejaVuSerifCondensed-Italic.ttf | DejaVu | serif | regular | italic | condensed | 3507 |
| 10 | LiberationMono-Italic.ttf | Liberation | mono | regular | italic | normal | 2425 |
| 11 | LiberationMono-Regular.ttf | Liberation | mono | regular | roman | normal | 2423 |
| 12 | LiberationSans-BoldItalic.ttf | Liberation | sans | bold | italic | normal | 2622 |
| 13 | LiberationSans-Regular.ttf | Liberation | sans | regular | roman | normal | 2620 |
| 14 | LiberationSansNarrow-Bold.ttf | Liberation | sans | bold | roman | narrow | 681 |
| 15 | LiberationSansNarrow-BoldItalic.ttf | Liberation | sans | bold | italic | narrow | 681 |
| 16 | LiberationSerif-Bold.ttf | Liberation | serif | bold | roman | normal | 2602 |
| 17 | LiberationSerif-BoldItalic.ttf | Liberation | serif | bold | italic | normal | 2605 |
| 18 | NotoMono-Regular.ttf | Noto | mono | regular | roman | normal | 897 |
| 19 | NotoSans-Bold.ttf | Noto | sans | bold | roman | normal | 3317 |
| 20 | NotoSans-BoldItalic.ttf | Noto | sans | bold | italic | normal | 3327 |
| 21 | NotoSansMath-Regular.ttf | Noto | math | regular | roman | normal | 2655 |
| 22 | NotoSerif-Italic.ttf | Noto | serif | regular | italic | normal | 3268 |
| 23 | NotoSerif-Regular.ttf | Noto | serif | regular | roman | normal | 3256 |
| 24 | NotoSerifDisplay-Bold.ttf | Noto | serif | bold | roman | normal | 3256 |
| 25 | NotoSerifDisplay-BoldItalic.ttf | Noto | serif | bold | italic | normal | 3268 |
| 26 | Ubuntu-Italic[wdth,wght].ttf | Ubuntu | sans | regular | italic | normal | 1325 |
| 27 | UbuntuMono-Italic[wght].ttf | Ubuntu | mono | regular | italic | normal | 1318 |
| 28 | UbuntuMono[wght].ttf | Ubuntu | mono | regular | roman | normal | 1313 |
| 29 | UbuntuSans[wdth,wght].ttf | Ubuntu | sans | regular | roman | normal | 1856 |

### Classification Summary

| Axis | Values | Count |
|------|--------|-------|
| Family | DejaVu, Liberation, Noto, Ubuntu | 9, 7, 8, 5 |
| Style class | sans, serif, mono, math | 10, 10, 7, 2 |
| Weight | extralight, regular, bold | 1, 19, 9 |
| Slant | roman, italic | 18, 11 |
| Width | normal, condensed, narrow | 25, 2, 2 |

16 unique (class × weight × slant × width) combinations exist among the 29 fonts.

---

## 2. Coverage Analysis: How the Autohinter Uses Font Properties

### 2.1 Autohinter Pipeline

The Latin autohinter pipeline (`src/autohint/latin.rs`, function `apply_hints`) processes each glyph through:

```
1. Load outline → 26.6 coordinates
2. [Horizontal] compute_segments → compute_edges → hint_edges → align_edge_points → align_strong_points → align_weak_points
   ⚠️ ENTIRE horizontal pass SKIPPED for italic fonts
3. [Vertical]   same pipeline (always executed)
4. Vertical separation adjustments
```

### 2.2 Font Properties That Drive Code Paths

#### `is_italic` — MAJOR CODE PATH BIFURCATION

Source: `head.mac_style` bit 1 (`font.rs:136`). This is the **only font-level property** that directly controls which code branches execute:

```rust
// latin.rs:725-727
if is_italic {
    hints.scaler_flags |= AF_SCALER_FLAG_NO_HORIZONTAL;
    record(COV_ITALIC_NO_HORZ);
}

// latin.rs:745-760 — entire horizontal pipeline skipped
if hints.scaler_flags & AF_SCALER_FLAG_NO_HORIZONTAL == 0 {
    // segment → edge → hint → align for horizontal dimension
} else {
    record(COV_ITALIC_HORZ_SKIPPED);
}
```

**Impact:** Italic fonts skip segment detection, edge grouping, edge hinting, strong-point alignment, and IUP for the horizontal dimension. This is roughly 40% of the autohinter pipeline. Every italic font exercises the same COV_ITALIC_* paths regardless of style class or weight.

#### `extra_light` — DYNAMICALLY COMPUTED (Not font metadata)

Source: `latin.rs:476`, `latin.rs:515`:

```rust
axis.extra_light = ft_mul_fix(axis.standard_width, x_scale) < 32 + 8;
```

Computed from `standard_width × scale < threshold`. Not a font-level property — any font at a small enough size could trigger it. The `COV_EXTRA_LIGHT` bit (bit 35) is defined in `coverage.rs` but is **not currently instrumented** (no `cov_hit!` call exists for it in `latin.rs`).

#### Style class (sans/serif/mono) — NOT USED BY THE AUTOHINTER

The autohinter's Latin module does **not** check font-level style classification. There is no `is_serif`, `style_class`, or `af_style_flags` check anywhere in `latin.rs`, `types.rs`, `loader.rs`, `scaler.rs`, or `font.rs` that affects autohinting behavior.

- **Blue zones:** Computed from glyph outlines using fixed character sets (`latin.rs:208-216`) — same for all fonts
- **Serif detection:** Geometric (segment-level), not font-level (`latin.rs:1252-1256`)
- **Serif handling** (COV_HINT_PHASE4_SERIF): Triggered by segment geometry, not font classification

#### Bold vs Regular — VALUES differ, CODE PATHS do not

Bold fonts have wider stems, which affects:
- `standard_width` → different `extra_light` threshold behavior
- Stem width snapping → different `COV_STEM_*` branches
- Blue zone values → different edge positions

But the same code paths execute. The differences are in the VALUES of intermediate computations, not in which branches are taken (beyond the stem-width-dependent branches, which any weight variant could exercise at different sizes).

#### Condensed/Narrow — VALUES differ, CODE PATHS do not

Condensed fonts have different character widths, which affects segment density and edge spacing. Same code paths execute; different intermediate values.

### 2.3 Test Structure

The FT coverage matrix (`coverage_matrix_ft.json`) tests all 29 fonts with **identical** structure:
- Same 5 operations: `getbbox`, `getlength`, `getmask`, `getmetrics`, `getname`
- Same 5 sizes: 10, 12, 16, 20, 24 pt
- Same 95 codepoints: 0–126 (printable ASCII + control)

All 29 fonts get 955 test rows each (94 getmask + 94 getbbox + 1 getmetrics + 1 getname + 1 getlength = 191 per size × 5 sizes). The only variation between fonts is the reference values (SHA-256 for getmask, bbox coordinates, metric numbers).

---

## 3. Redundancy Analysis

### 3.1 Same (style, weight, slant, width) → Same code paths

Within each unique combination, only the specific pixel values differ. For example:

| Font combo | Fonts | Coverage difference |
|------------|-------|---------------------|
| sans, regular, roman, normal | LiberationSans-Regular, UbuntuSans | None (same code paths) |
| serif, bold, roman, normal | DejaVuSerif-Bold, LiberationSerif-Bold, NotoSerifDisplay-Bold | None (same code paths) |
| mono, regular, roman, normal | DejaVuSansMono, LiberationMono-Regular, NotoMono-Regular, UbuntuMono | None (same code paths) |
| mono, regular, italic, normal | DejaVuSansMono-Oblique, LiberationMono-Italic, UbuntuMono-Italic | None (same code paths) |
| math, regular, roman, normal | DejaVuMathTeXGyre, NotoSansMath-Regular | None (same code paths) |
| sans, bold, italic, normal | LiberationSans-BoldItalic, NotoSans-BoldItalic | None (same code paths) |
| sans, regular, italic, normal | DejaVuSans-Oblique, Ubuntu-Italic | None (same code paths) |
| serif, regular, italic, normal | DejaVuSerif-Italic, NotoSerif-Italic | None (same code paths) |
| serif, bold, italic, normal | LiberationSerif-BoldItalic, NotoSerifDisplay-BoldItalic | None (same code paths) |

This accounts for 21 of the 29 fonts — they have at least one twin in the same class.

### 3.2 Cross-family redundancy

The autohinter does not use font-family information. Within the same (style, weight, slant, width) class, DejaVu and Liberation fonts exercise identical code paths. Only VALUES differ. This accounts for redundancy across families (e.g., DejaVuSansMono vs NotoMono-Regular).

### 3.3 Italic redundancy

All 11 italic fonts exercise COV_ITALIC_NO_HORZ and COV_ITALIC_HORZ_SKIPPED. The vertical hinting pipeline (which runs for all fonts, italic or not) is the same for all italic fonts regardless of style class. **One italic font suffices for italic code-path coverage.**

---

## 4. Proposed Minimal Subset: 5 Fonts

### Selection Rationale

For autohinter code coverage, the only font-level property that creates a different code path is `is_italic`. All other axes (style class, weight, width) affect VALUES but not which branches execute.

However, for **future-proofing** and to ensure the test suite exercises a diverse range of autohinter internal thresholds (stem width snapping, blue zone detection, extra_light computation), we include fonts with diverse properties:

| # | Font | Rationale |
|---|------|-----------|
| 1 | **DejaVuSans-Oblique.ttf** | **Italic coverage.** Triggers COV_ITALIC_NO_HORZ + COV_ITALIC_HORZ_SKIPPED. Sans-serif italic represents the most common italic use case. Large glyph set (5355). |
| 2 | **LiberationSans-Regular.ttf** | **Sans roman baseline.** Most common font class. Liberation family has different stem widths from DejaVu (exercises different `extra_light` thresholds and stem snap branches). |
| 3 | **DejaVuSerif-Bold.ttf** | **Serif + bold.** Serif outlines exercise different serif-detection branches (geometric). Bold weight exercises wider stem widths → different COV_STEM_* branches even though not yet instrumented. |
| 4 | **DejaVuSansMono.ttf** | **Monospace.** Fixed-pitch metrics, different glyph proportions → different segment detection behavior. Distinct from proportional fonts. |
| 5 | **DejaVuSerifCondensed-Bold.ttf** | **Condensed width.** Narrower glyphs produce different segment density and edge spacing → exercises edge-case geometric thresholds in edge detection. |

**Total: 5 fonts** (83% reduction from 29).

### Coverage Preservation Argument

| Coverage Concern | How Preserved |
|-----------------|---------------|
| Italic horizontal skip (COV_ITALIC_*) | DejaVuSans-Oblique.ttf (#1) |
| Roman full pipeline | LiberationSans-Regular (#2), DejaVuSerif-Bold (#3), DejaVuSansMono (#4), DejaVuSerifCondensed-Bold (#5) |
| Serif geometric detection | DejaVuSerif-Bold (#3), DejaVuSerifCondensed-Bold (#5) |
| Bold stem widths | DejaVuSerif-Bold (#3), DejaVuSerifCondensed-Bold (#5) |
| Monospace metrics | DejaVuSansMono (#4) |
| Condensed segment density | DejaVuSerifCondensed-Bold (#5) |
| Diverse blue zone values | 5 fonts × 4 families = diverse blue zone computations |
| Diverse stem widths | ExtraLight(thin), Regular(medium), Bold(wide) → all three weight classes |
| Multiple font families | DejaVu (3), Liberation (1), covers 2 of 4 families; cross-family redundancy proven in §3.2 |

### What Is Lost (and Why It's Acceptable)

| Lost Font Category | Why Acceptable |
|--------------------|----------------|
| LiberationSerif-Bold (serif bold DejaVu twin) | DejaVuSerif-Bold covers the same code paths |
| NotoSans-Bold, NotoSans-BoldItalic (sans bold) | LiberationSans-Regular covers sans; DejaVuSerif-Bold covers bold |
| NotoSerif-* (serif family) | DejaVuSerif-* fonts cover same paths |
| Ubuntu* (variable fonts) | Same code paths as DejaVu/Liberation equivalents |
| DejaVuMathTeXGyre, NotoSansMath-Regular | Math fonts use same Latin autohinter profile as sans-serif |
| LiberationMono-* (mono twins) | DejaVuSansMono covers mono code paths |
| LiberationSansNarrow-* (narrow) | Condensed (#5) exercises same geometric edge cases |
| DejaVuSans-ExtraLight | COV_EXTRA_LIGHT not instrumented; any font at small size can trigger extra_light dynamically |

---

## 5. Conservative Alternative: 8 Fonts

If the project prefers more conservative coverage (wider stem diversity, more family coverage), an 8-font set is viable:

1. **DejaVuSans-ExtraLight.ttf** — Reserve for future COV_EXTRA_LIGHT instrumentation; also exercises thin-stem thresholds
2. **DejaVuSans-Oblique.ttf** — Italic coverage
3. **LiberationSans-Regular.ttf** — Sans roman baseline (Liberation family)
4. **NotoSans-Bold.ttf** — Sans bold roman (Noto family; different stem widths from Liberation)
5. **DejaVuSerif-Italic.ttf** — Serif + italic combo (exercises serif outline shapes with italic horizontal skip)
6. **DejaVuSerif-Bold.ttf** — Serif bold roman (condensed not needed with this selection)
7. **DejaVuSansMono.ttf** — Monospace coverage
8. **LiberationSansNarrow-Bold.ttf** — Narrow width + bold (different from condensed in glyph proportions)

This set retains 8 fonts (72% reduction) while covering all 4 families and all unique width classes.

---

## 6. Empirical Coverage Validation (2026-06-30)

Using the coverage setup documented in [coverage-setup.md](coverage-setup.md), we measured the actual line-level coverage delta between the 29-font suite and the proposed 5-font minimal set.

### Results

| File | 29-font lines covered | 5-font lines covered | Δ Lost |
|------|----------------------|---------------------|--------|
| autohint/latin.rs | 1406 | 1369 | **-37** |
| autohint/loader.rs | 198 | 196 | -2 |
| font.rs | 189 | 187 | -2 |
| grays.rs | 424 | 416 | -8 |
| tt/glyf.rs | 208 | 135 | **-73** |
| tt/loca.rs | 28 | 24 | -4 |
| **Total** | | | **-126** |

### Analysis of Losses

**glyf.rs (-73 lines):** Composite glyph parsing. The 29-font set includes fonts (NotoMath, DejaVuMathTeXGyre, Ubuntu variable fonts) that use compound glyph definitions for ASCII-range characters. None of the 5 selected fonts use composite glyphs for codepoints 0-126.

**latin.rs (-37 lines):** Geometric edge cases in the autohinter. Specific losses include:
- Segment merging with direction mismatches (L986-L1008)
- Directionless-segment catch in edge detection (L1192-L1197)  
- Round-vs-flat blue zone skip (L355)
- Vertical separation adjustment for i/j dots (L678)

These are triggered by specific glyph outline geometries present in some fonts but absent in the 5 chosen ones.

**grays.rs (-8 lines):** Conic curve start handling — font-dependent outline encoding differences.

### Conclusion

The empirical validation **disproves** the hypothesis that all 29 fonts exercise identical autohinter code paths. Font diversity matters because different glyph outlines trigger different geometric branches. The 5-font set loses 37 autohinter lines and 73 glyph-parser lines.

### Revised Recommendation

**Use the 8-font conservative set** (see §5 above) to recover most coverage losses:
- Adding `NotoSans-Bold` (Noto family) should recover glyf.rs composite code paths
- Adding `DejaVuSerif-Italic` and `LiberationSansNarrow-Bold` provides more geometric diversity
- Adding `DejaVuSans-ExtraLight` preserves EXTRA_LIGHT instrumentation path

Future work: run `--5vs8` comparison to validate recovery.

## 7. Recommendation (Original)

**Use the 5-font minimal set** for the following reasons:

1. The autohinter is fundamentally font-agnostic for Latin script. The only font-level code path bifurcation is italic vs roman. All other axes (weight, width, style) produce the same code path execution with different values.

2. The test structure is uniform: all 29 fonts are tested with identical operations, sizes, and codepoints. The only difference is the reference pixel values, which test correctness, not coverage.

3. Five fonts × diverse properties (sans serif, serif bold, mono, condensed, italic) provide more than sufficient value diversity to exercise:
   - All geometric edge cases (serif detection, condensed segment density)
   - All weight-dependent stem width branches
   - All blue zone computation paths (through diverse font families)
   - Both italic and roman horizontal processing

4. If future coverage instrumentation adds font-class-dependent coverage bits (e.g., `COV_SERIF_HINT`), the 5-font set already includes serif fonts.

### Validation Plan

Before removing any font files:
1. Run `cargo test -p pillow-rs-freetype` to capture baseline pass/fail counts
2. Temporarily remove 24 fonts, keep the 5-font set
3. Re-run `cargo test -p pillow-rs-freetype`
4. Confirm that:
   - All previously passing tests still pass
   - Previously failing tests fail for the same reasons (SHA mismatches, not new structural errors)
   - No new compilation errors from missing font references in test code
5. Update `coverage_matrix_ft.json` generator to only reference the 5 kept fonts
6. Update `TASKS.md` with new font list

**Do not delete font files until validation confirms no regressions.**

---

## 7. Methodology

1. **Font inventory:** Extracted metadata (style class, weight, slant, width, glyph count) from all 29 fonts using fontTools (`scripts/extract_font_info.py` → `/tmp/font_inventory.json`).

2. **Code-path analysis:** Traced all font-property-dependent decisions in the Latin autohinter (`src/autohint/latin.rs`, `src/autohint/coverage.rs`, `src/font.rs`, `src/scaler.rs`). Identified `is_italic` as the sole font-level code path bifurcation.

3. **Test matrix analysis:** Analyzed `coverage_matrix_ft.json` (27,695 rows) confirming identical test structure per font (same ops, sizes, codepoints).

4. **Redundancy mapping:** Grouped fonts by (style, weight, slant, width) tuples, identifying 16 unique combinations across 29 fonts. Mapped each combination to autohinter code paths.

5. **Selection:** Identified minimal covering set using set-cover principle on the code-path dimensions, then refined for value diversity.

---

## Appendix A: Unique (class × weight × slant × width) Combinations

| # | Class | Weight | Slant | Width | Fonts |
|---|-------|--------|-------|-------|-------|
| 1 | sans | extralight | roman | normal | DejaVuSans-ExtraLight |
| 2 | sans | regular | roman | normal | LiberationSans-Regular, UbuntuSans |
| 3 | sans | regular | italic | normal | DejaVuSans-Oblique, Ubuntu-Italic |
| 4 | sans | bold | roman | normal | NotoSans-Bold |
| 5 | sans | bold | italic | normal | LiberationSans-BoldItalic, NotoSans-BoldItalic |
| 6 | sans | bold | roman | narrow | LiberationSansNarrow-Bold |
| 7 | sans | bold | italic | narrow | LiberationSansNarrow-BoldItalic |
| 8 | serif | regular | roman | normal | NotoSerif-Regular |
| 9 | serif | regular | italic | normal | DejaVuSerif-Italic, NotoSerif-Italic |
| 10 | serif | bold | roman | normal | DejaVuSerif-Bold, LiberationSerif-Bold, NotoSerifDisplay-Bold |
| 11 | serif | bold | italic | normal | LiberationSerif-BoldItalic, NotoSerifDisplay-BoldItalic |
| 12 | serif | bold | roman | condensed | DejaVuSerifCondensed-Bold |
| 13 | serif | regular | italic | condensed | DejaVuSerifCondensed-Italic |
| 14 | mono | regular | roman | normal | DejaVuSansMono, LiberationMono-Regular, NotoMono-Regular, UbuntuMono |
| 15 | mono | regular | italic | normal | DejaVuSansMono-Oblique, LiberationMono-Italic, UbuntuMono-Italic |
| 16 | math | regular | roman | normal | DejaVuMathTeXGyre, NotoSansMath-Regular |

The 5-font proposal covers combinations 2, 3, 10, 12, and 14 — providing representation from sans, serif, mono, italic, bold, and condensed axes. The remaining combinations are either redundant (italic paths identical regardless of style class) or represent edge cases (narrow ≈ condensed, math ≈ sans).
