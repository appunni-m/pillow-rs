# pillow-rs-font — Pure-Rust FreeType 2.6.x Compatible Font Renderer

**Status:** Design  
**Date:** 2026-06-21  
**Target:** Replace `fontdue` with pixel-identical pure-Rust FreeType 2.6.x rendering  
**Pattern:** Mirrors `pillow-rs-image` architecture, coverage matrix, and test infrastructure

---

## 1. Problem

`pillow-rs` currently uses `fontdue` for TrueType font rendering. This causes parity failures
against PIL because fontdue produces different glyph metrics, rasterized pixel values, and
anti-aliasing than PIL's bundled FreeType 2.6.x. The `ImageFont` module has 0% parity coverage
(8 functions, 0 passing tests).

As measured on DejaVu Sans 16pt glyph "A":
| | mask size | gray levels | advance | bbox |
|---|---|---|---|---|
| PIL FreeType 2.6.x | 11×12 | 51 distinct (0-255 range) | ~8.1px | (0,3,11,15) |
| RSPIL fontdue | 7×10 | fontdue-specific | different | different |

PIL 12.2.0 uses the **smooth rasterizer** (`ftgrays.c`), producing 256-level anti-aliasing
with exact cell-coverage computation — not the 5-level standard rasterizer.

## 2. Approach: Mirror pillow-rs-image Exactly

`pillow-rs-image` established the pattern:

```
manifest.yaml          → single source of truth (API surface + edge case matrix)
coverage_matrix.json   → auto-generated test matrix (driven by manifest)
scripts/generate_*.py  → runs PIL to produce SHA-256 references
tests/coverage_matrix_tests.rs → single test runner against matrix
```

`pillow-rs-font` follows this identically:

```
manifest.yaml                    → API surface + font×size×glyph×operation matrix
scripts/generate_font_refs.py    → PIL FreeType → coverage_matrix.json + raw pixel dumps
tests/coverage_matrix_tests.rs   → iterates rows, compares SHA-256 against PIL refs
tests/fixtures/coverage_matrix.json → committed, auto-generated, single source of truth
tests/fixtures/outputs/raws/     → PIL FreeType pixel dumps per glyph (committed)
tests/fixtures/input/fonts/      → test fonts (DejaVuSans.ttf, LiberationSerif-Regular.ttf)
```

## 3. Architecture

### 3.1 Crate Layout

```
pillow-rs-font/                         (new workspace crate — zero external font deps)
├── Cargo.toml                          (deps: log, thiserror; dev: serde, serde_json, sha2)
├── manifest.yaml                       (API surface + coverage dimensions)
├── src/
│   ├── lib.rs                          (pub API: Font, GlyphMask, FontError)
│   ├── error.rs                        (thiserror FontError enum)
│   ├── parser/
│   │   ├── mod.rs                      (table directory parsing, dispatch)
│   │   ├── cmap.rs                     (formats 0, 2, 4, 6, 12 — char→glyph index)
│   │   ├── head.rs                     (units_per_em, flags, mac_style, index_to_loc_format)
│   │   ├── hhea.rs                     (ascent, descent, line_gap in font units)
│   │   ├── hmtx.rs                     (advance_width, lsb per glyph)
│   │   ├── maxp.rs                     (num_glyphs)
│   │   ├── name.rs                     (family, style — platform 3, encoding 1 preferred)
│   │   ├── os2.rs                      (sTypoAscender, sTypoDescender, usWinAscent/Descent)
│   │   ├── post.rs                     (underline_position, underline_thickness)
│   │   ├── loca_glyf.rs               (glyph outline extraction — quadratic Bézier)
│   │   └── kern.rs                     (kerning pairs, optional)
│   ├── scaler.rs                       (26.6 fixed-point scaling = tt_size_reset)
│   ├── hinting.rs                      (ppem thresholds, cvt, blue zones)
│   ├── raster.rs                       (cell-based rasterizer = ftgrays.c)
│   └── metrics.rs                      (getbbox, getmetrics, getlength compositor)
├── scripts/
│   └── generate_font_refs.py           (PIL FreeType → coverage_matrix.json + raws/)
├── tests/
│   ├── coverage_matrix_tests.rs         (single test driver against matrix)
│   └── fixtures/
│       ├── coverage_matrix.json         (auto-generated, committed)
│       ├── input/fonts/                 (test font files)
│       └── outputs/raws/               (PIL .bin pixel dumps per glyph)
```

### 3.2 Module Dependency Graph

```
error.rs  ←── all modules (FontError)

parser/   ←── lib.rs (font loading)
  ├── cmap.rs, head.rs, maxp.rs        (load first — index needed)
  ├── hhea.rs, hmtx.rs, os2.rs, post.rs (metrics)
  ├── name.rs                          (font identification)
  ├── loca_glyf.rs                    (glyph outlines, depends on maxp+head)
  └── kern.rs                         (optional)

scaler.rs ←── lib.rs (glyph loading) — uses parser tables

hinting.rs ←── scaler.rs (applied before rasterization)

raster.rs  ←── scaler.rs → lib.rs (glyph rendering)

metrics.rs ←── lib.rs (public API) — uses scaler + raster results
```

### 3.3 Public API Surface

```rust
// lib.rs

/// A loaded TrueType/OpenType font.
pub struct Font { /* parser tables + size */ }

/// Rendered glyph bitmap with metrics.
pub struct GlyphMask {
    pub width: u32,
    pub height: u32,
    /// Row-major alpha pixels (0-255). 256-level anti-aliased.
    pub pixels: Vec<u8>,
    /// Horizontal offset (26.6) for compositing.
    pub xmin: i32,
    /// Vertical offset (26.6) for compositing.
    pub ymin: i32,
    /// Advance width in pixels (26.6).
    pub advance_width: f32,
}

impl Font {
    /// Load from raw TrueType font bytes at given point size.
    /// Parses all required tables immediately.
    pub fn truetype(data: &[u8], size_pt: f32) -> Result<Self, FontError>;

    /// Render a glyph as alpha mask (PIL: getmask).
    pub fn getmask(&self, text: &str) -> Result<GlyphMask, FontError>;

    /// Render a glyph with offset (PIL: getmask2).
    /// Returns (mask, offset_x, offset_y).
    pub fn getmask2(&self, text: &str) -> Result<(GlyphMask, i32, i32), FontError>;

    /// Bounding box of text (PIL: getbbox).
    /// Returns (left, top, right, bottom).
    pub fn getbbox(&self, text: &str) -> Result<(i32, i32, i32, i32), FontError>;

    /// Font metrics (PIL: getmetrics).
    /// Returns (ascent, descent) in pixels.
    pub fn getmetrics(&self) -> (u32, u32);

    /// Font family and style name (PIL: getname).
    pub fn getname(&self) -> (&str, &str);

    /// Sum of advance widths (PIL: getlength).
    pub fn getlength(&self, text: &str) -> Result<f32, FontError>;

    /// Create font variant with overridden parameters (PIL: font_variant).
    pub fn font_variant(&self, size: Option<f32>) -> Font;
}
```

### 3.4 Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("Invalid TrueType font: {0}")]
    InvalidFont(String),
    #[error("Unsupported cmap table format: {0}")]
    UnsupportedCmapFormat(u16),
    #[error("Rasterizer buffer overflow")]
    RasterOverflow,
    #[error("Invalid glyph outline: {0}")]
    InvalidOutline(String),
}
```

**Glyph-not-found semantics:** Unmapped codepoints fall back to glyph 0 (`.notdef`)
matching FreeType behavior. No error is returned — a blank/box glyph is rendered.
The `GlyphNotFound` error variant is intentionally absent; the only errors are
malformed font data, unsupported table formats, and rasterizer resource exhaustion.

### 3.5 Internal Data Sharing

`Font` holds parsed table data behind `Arc<FontData>` to enable cheap `font_variant()`
(creates a new Font with shared tables but different size). All `&str` return values
from `getname()` borrow from `Arc`-owned data — valid for `'static` lifetime of the
loaded font.

## 4. Core Algorithms

### 4.1 Table Parser (`parser/`)

Pure-Rust binary parsing. No `ttf-parser` crate. Matches FreeType 2.6's internal
struct layouts exactly.

**Tables parsed and their use:**

| Table | Tag | Used For |
|-------|-----|----------|
| `cmap` | `'cmap'` | char → glyph index mapping (formats 0, 2, 4, 6, 12) |
| `head` | `'head'` | `units_per_em`, `index_to_loc_format`, flags |
| `hhea` | `'hhea'` | `ascent`, `descent`, `line_gap`, `num_hmetrics` |
| `hmtx` | `'hmtx'` | `advance_width`, `lsb` for each glyph |
| `maxp` | `'maxp'` | `num_glyphs` |
| `name` | `'name'` | family, style strings (prefer platform 3, encoding 1 = Windows Unicode BMP) |
| `OS/2`  | `'OS/2'` | `sTypoAscender`, `sTypoDescender`, `usWinAscent`, `usWinDescent` |
| `post`  | `'post'` | `underline_position`, `underline_thickness` |
| `loca`  | `'loca'` | glyph data offsets (short: /2, long: direct) |
| `glyf`  | `'glyf'` | glyph outline data (quadratic Bézier curves, composites) |
| `kern`  | `'kern'` | kerning pairs (format 0 only) |

**cmap format priority:** format 12 (Unicode full) → format 4 (BMP segment) → format 6 (trimmed) → format 2 (mixed 8/16) → format 0 (byte encoding).

### 4.2 Glyph Scaler (`scaler.rs`)

26.6 fixed-point — 1 pixel = 64 sub-units. Matches `tt_size_reset` exactly.

**Fixed-point math (matching `ftcalc.h`):**

```rust
/// FT_MulFix(a, b) = (a * b + 0x8000) >> 16  (with rounding)
fn mul_fix(a: i32, b: i32) -> i32 {
    let ab = (a as i64) * (b as i64);
    ((ab + 0x8000 + (ab >> 63)) >> 16) as i32  // rounding toward +inf
}

/// FT_DivFix(a, b) = (a << 16) / b
fn div_fix(a: i32, b: i32) -> i32 {
    (((a as i64) << 16) / (b as i64)) as i32
}
```

**Scaling algorithm (matching `tt_size_reset` lines 1247-1296):**

1. `ppem = ceil(size_pt)` (assume 72 DPI → ppem = size_pt)
2. `x_scale = div_fix(ppem << 6, units_per_em)`
3. `y_scale = div_fix(ppem << 6, units_per_em)`
4. Scale glyph outline points: `scaled_x = mul_fix(funit_x, x_scale)`; `scaled_y = mul_fix(funit_y, y_scale)`
5. `advance = mul_fix(hmtx.advance_width, x_scale)`
6. Metrics: `ascender = pixel_round(mul_fix(face.ascender, y_scale))`; `descender = pixel_round(mul_fix(face.descender, y_scale))`

**Glyph outline loading:**
- Simple glyphs: read end_pts_of_contours → flags → x_coordinates → y_coordinates
- Supports `Repeat`, `XShortVector`, `YShortVector`, `XSame`, `YSame` flag decoding
- Quadratic Bézier on-curve/off-curve points (FreeType uses conic splines)
- Composite glyphs: recursive composition with 2×3 transformation matrix (scale, rotate, translate)

### 4.3 Cell-Based Rasterizer (`raster.rs`)

Matches `ftgrays.c` (smooth rasterizer). Produces 256-level anti-aliased output.

**Key constants:**
- `PIXEL_BITS = 8` — 256 sub-pixel units per pixel
- `ONE_PIXEL = 256`
- Coordinates: `TPos` = 26.6 fixed-point input; `TCoord` = integer pixel coordinates

**Algorithm (single-pass):**

1. **Flatten outline** — Decompose quadratic Bézier curves to line segments within 1 sub-pixel tolerance
2. **Record cells** — For each line segment, record which pixel cells it crosses and the signed area contributed above the cell
3. **Accumulate per scanline** — Y-sorted linked lists of cells. Each cell stores: `x` (pixel column), `cover` (signed coverage delta), `area` (signed area contribution)
4. **Sweep** — For each scanline y:
   - Traverse sorted x-cells
   - Accumulate running `cover` (add cell.cover at each x)
   - Accumulate `area = area + cover` for each pixel between cell x positions
   - Apply fill rule to accumulated area
5. **Fill rule** — Non-zero winding rule by default (PIL default):

```rust
fn coverage_from_area(area: i32, fill: i32) -> u8 {
    // FT_FILL_RULE macro from ftgrays.c:408-415
    let mut coverage = (area >> (PIXEL_BITS * 2 + 1 - 8)) as i32;  // area >> 9
    if coverage & fill != 0 {
        coverage = !coverage;
    }
    if coverage > 255 && fill & i32::MIN != 0 {
        coverage = 255;
    }
    coverage as u8  // clamped to 0-255
}
```

**Bézier flattening (matching ftgrays.c):**
- Quadratic Bézier: split until deviation < 1 sub-pixel
- Deviation estimate: distance from control point to chord midpoint
- Conic case: predictable convergence, number of splits can be pre-computed

### 4.4 Hinting (`hinting.rs`)

**Data-driven, not theory-first.** The FreeType bytecode interpreter (`ttinterp.c`,
7520 lines) is NOT re-implemented. Instead:

1. Phase 3 ships with **zero hinting** — rasterizer alone produces anti-aliased output
2. Phase 5 generates the coverage matrix — identifies exactly which glyph×size pairs diverge
3. Phase 6 applies targeted hinting corrections ONLY for the mismatched cases:
   - **ppem threshold decisions**: Different stem-width decisions based on ppem ranges
   - **Blue zone alignment**: Vertical zones (ascender, descender, x-height, cap-height) snap to pixel grid
   - **Stem width snapping**: Horizontal/vertical stems snap to integer widths at small ppem
   - **Overshoot suppression**: Suppress rounded tops at specific ppem thresholds

This avoids speculative work. If the unscaled rasterizer produces pixel-identical output
for most glyphs, only the edge cases need hinting.

### 4.5 Metrics Compositor (`metrics.rs`)

**getbbox:** Start at (0, 0). For each glyph: offset by previous advance_width, apply
bearing_x offset, track min/max across rendered pixels. Apply stroke_width offset.

**getmetrics:**
- `ascent = pixel_round(mul_fix(os2.sTypoAscender, y_scale))`
- `descent = -pixel_round(mul_fix(os2.sTypoDescender, y_scale))`

**getlength:** Sum `mul_fix(advance_width, x_scale)` for all glyphs, convert from
26.6 fixed-point to `f32` pixels by dividing by 64.

**getname:** Read `name` table entries, prefer platform 3 (Windows) encoding 1 (Unicode BMP),
nameID 1 (family) and nameID 2 (subfamily). Decode UTF-16BE.

## 5. Coverage Matrix

### 5.1 Dimensions

The matrix cross-multiplies five independent dimensions:

| Dimension | Values | Cardinality |
|-----------|--------|-------------|
| **Font** | DejaVuSans.ttf, LiberationSerif-Regular.ttf | 2 |
| **Size (pt)** | 10, 12, 16, 20, 24 | 5 |
| **Glyph** | 95 printable ASCII (32-126) + 5 boundary (space, tab, newline, delete, null) | 100 |
| **Operation** | getmask, getmask2, getbbox, getmetrics, getname, getlength, font_variant | 7 |
| **Mode** | L (single-channel alpha mask) | 1 |

**Total matrix rows:** 2 × 5 × 100 × 7 ≈ 7,000 rows

### 5.2 Reference Generation

`scripts/generate_font_refs.py` uses PIL to produce:

1. **Pixel dumps** (`outputs/raws/{font}_{size}_{codepoint}_getmask.bin`) — raw bytes of `font.getmask(ch)`
2. **SHA-256 refs** in `coverage_matrix.json` — for byte-perfect comparison
3. **JSON refs** for non-pixel operations:
   - `getbbox` → `[left, top, right, bottom]`
   - `getmetrics` → `[ascent, descent]`
   - `getname` → `["Family", "Style"]`
   - `getlength` → `float`
   - `font_variant` → `getname()` of variant

### 5.3 Matrix Row Status

Each row has a status:
- `active` — reference generated, test asserts against it
- `planned` — reference to generate once algorithm is ready
- `skip` — not applicable (e.g., empty font files if not present)

## 6. Test Infrastructure

### 6.1 Integration Test (`tests/coverage_matrix_tests.rs`)

```rust
use pillow_rs_font::Font;
use sha2::{Digest, Sha256};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CoverageMatrix {
    fonts: HashMap<String, FontData>,
    operations: Vec<OperationData>,
    rows: Vec<MatrixRow>,
}

#[derive(Debug, Deserialize)]
struct MatrixRow {
    id: String,
    font: String,
    size_pt: f32,
    codepoint: u32,
    operation: String,
    status: String,          // "active" | "planned" | "skip"
    ref_sha256: Option<String>,
    ref_value: Option<serde_json::Value>,  // for non-pixel ops
}

#[test]
fn test_font_coverage_matrix() {
    let matrix = load_coverage_matrix();
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;

    for row in &matrix.rows {
        if row.status != "active" {
            skipped += 1;
            continue;
        }
        let font_data = load_font_bytes(&row.font);
        let font = Font::truetype(&font_data, row.size_pt)
            .expect("font loading failed");

        let result = match row.operation.as_str() {
            "getmask" => {
                let mask = font.getmask(&char_from_cp(row.codepoint))
                    .expect("getmask failed");
                sha256(&mask.pixels)
            }
            "getbbox" => serialize_json(font.getbbox(...)),
            "getmetrics" => serialize_json(font.getmetrics()),
            "getname" => serialize_json(font.getname()),
            "getlength" => serialize_json(font.getlength(...)),
            "font_variant" => serialize_json(font.font_variant(...).getname()),
            _ => { skipped += 1; continue; }
        };

        if let Some(ref hash) = row.ref_sha256 {
            if result == *hash {
                passed += 1;
            } else {
                eprintln!("  FAIL [{}]: pixel mismatch", row.id);
                failed += 1;
            }
        }
    }

    eprintln!("\nfont matrix: {passed}/{total} passed, {failed} failed, {skipped} skipped");
    if failed > 0 {
        panic!("{failed} font test(s) failed");
    }
}
```

### 6.2 Unit Tests (per-module `#[cfg(test)] mod tests`)

| Module | Test Scope |
|--------|-----------|
| `parser/cmap.rs` | Each format: valid → correct glyph index, boundary code points, malformed → error |
| `parser/name.rs` | Platform 3 encoding 1 extraction, fallback to platform 1 encoding 0 |
| `parser/loca_glyf.rs` | Simple glyph decode, composite glyph recursion, empty glyph |
| `scaler.rs` | `mul_fix`, `div_fix` boundary values, scaling round-trip, ppem=0 rejection |
| `raster.rs` | Single line segment coverage, rectangle fill, triangle coverage, zero-size |
| `metrics.rs` | Multi-char bbox accumulation, empty string bbox, tab advance |

### 6.3 Test Naming Convention

Following Rust ch.5.1, test modules per function and descriptive names:

```rust
#[cfg(test)]
mod getmask {
    use super::*;

    #[test]
    fn single_ascii_uppercase_a_returns_nonzero_mask() { ... }
    #[test]
    fn empty_string_returns_zero_size_mask() { ... }
    #[test]
    fn space_character_yields_zero_width_but_nonzero_advance() { ... }
    #[test]
    fn null_codepoint_returns_error() { ... }
}
```

## 7. Workspace Integration

### 7.1 Cargo.toml Changes

**Workspace `Cargo.toml`:**
```toml
[workspace]
members = [
    "pillow-rs",
    "pillow-rs-py",
    "pillow-rs-js",
    "pillow-rs-image",
    "pillow-rs-font",        # ← NEW
]
```

**`pillow-rs/Cargo.toml`:**
```toml
[dependencies]
pillow-rs-image = { path = "../pillow-rs-image" }
pillow-rs-font = { path = "../pillow-rs-font" }  # ← NEW, replaces fontdue
# fontdue = "0.9"  # ← REMOVED
```

### 7.2 Code Migration in pillow-rs

1. `pillow-rs/src/font/mod.rs` — replace `fontdue::Font` with `pillow_rs_font::Font`
2. Remove `fontdue` from `pillow-rs/Cargo.toml`
3. `pillow-rs-py/src/lib.rs` — `PyFont` wraps `pillow_rs_font::Font` instead of `pillow_rs::font::Font`
4. `pillow-rs-py/python/pillow_rs/imagefont.py` — remove PIL fallback (`_pil_font`); now pure Rust matches PIL pixel-perfect

### 7.3 PIL Fallback Removal

Once `pillow-rs-font` achieves pixel-identical output, the PIL delegation hack in
`imagefont.py` (lines 76-84, 98-107) is no longer needed. The Rust path produces
identical results to PIL's FreeType.

## 8. Implementation Phases

### Phase 1: Scaffold + Table Parser (foundational)

1. Create `pillow-rs-font/` crate skeleton (`Cargo.toml`, `lib.rs`, `error.rs`)
2. Add to workspace `Cargo.toml` members
3. Implement `parser/mod.rs` — table directory parsing (offset + length)
4. Implement `parser/head.rs` — font header
5. Implement `parser/maxp.rs` — num_glyphs
6. Implement `parser/cmap.rs` — formats 4 and 12 (covers 99% of fonts)
7. Implement `parser/hhea.rs`, `parser/hmtx.rs` — metrics tables
8. Unit tests for each parser module against known font bytes

**Deliverable:** `Font::truetype()` parses tables; `getname()` and `getmetrics()` work.
~6 files, ~800 LoC.

### Phase 2: Glyph Scaling + Outline Loading

1. Implement `parser/loca_glyf.rs` — simple glyph outline extraction
2. Implement `scaler.rs` — 26.6 fixed-point math, ppem scaling
3. Implement composite glyph support (recursive)
4. Unit tests: known glyph advance_width matches FreeType

**Deliverable:** Glyph outlines load and scale correctly.
~3 files, ~600 LoC.

### Phase 3: Rasterizer

1. Implement `raster.rs` — cell-based scanline rasterizer
2. Bézier flattening within 1 sub-pixel tolerance
3. Cell recording, y-sorted linked lists
4. Sweep phase with coverage accumulation
5. FT_FILL_RULE matching ftgrays.c exactly
6. Unit tests: basic shapes produce correct coverage

**Deliverable:** `getmask("A")` produces pixel data. ~1 file, ~500 LoC.

### Phase 4: Metrics + Multi-Glyph

1. Implement `metrics.rs` — getbbox, getmetrics, getlength composers
2. Multi-glyph text rendering (glyph composition)
3. getmask2 (with offset)
4. font_variant

**Deliverable:** All 7 PIL ImageFont operations implemented. ~1 file, ~200 LoC.

### Phase 5: Reference Generation + Matrix

1. Write `scripts/generate_font_refs.py` (PIL-driven fixture generator)
2. Generate `coverage_matrix.json` (~7000 rows)
3. Write `tests/coverage_matrix_tests.rs`
4. First run → identify all mismatches
5. Iterate on scaler + rasterizer until pixel-perfect

**Deliverable:** Matrix test running, failures tracked by status field.

### Phase 6: Hinting Tuning

1. Compare mis-matching glyphs systematically
2. Tune blue zone alignment
3. Tune stem width snapping
4. Iterate until 100% of `active` rows pass

**Deliverable:** All matrix rows `active` and passing.

### Phase 7: Integration + Cleanup

1. Replace `fontdue` in `pillow-rs/Cargo.toml` with `pillow-rs-font`
2. Rewrite `pillow-rs/src/font/mod.rs` to use `pillow_rs_font::Font`
3. Remove PIL fallback from `imagefont.py`
4. Run full parity test suite
5. `cargo clippy --all-targets --all-features --locked -- -D warnings`

**Deliverable:** All font parity tests pass, no PIL delegation needed.

## 9. Constraints Summary

| # | Constraint | Source |
|---|-----------|--------|
| 1 | `thiserror` for errors, no `unwrap()`/`expect()` in production | workspace lint + CLAUDE.md |
| 2 | `&str` not `String`, `&[u8]` not `Vec<u8>` in params | Rust ch.1.1 |
| 3 | 26.6 fixed-point as newtypes (`F26Dot6`, `SubPixel`) | Rust ch.1 (newtype) |
| 4 | `Copy` for ≤24-byte plain-data types | Rust ch.1.2 |
| 5 | Test names: `module::should_behavior_when_condition` | Rust ch.5.1 |
| 6 | Matrix test pattern = pillow-rs-image exactly | project convention |
| 7 | `pub(crate)` internals, narrow `pub` surface | Rust ch.1.7 |
| 8 | `#[expect(lint, reason = "...")]`, never `#[allow()]` | Rust ch.2.4 + workspace |
| 9 | Zero `unsafe` code | workspace lint `unsafe_code = "deny"` |
| 10 | `log` crate, never `eprintln!`/`println!` | CLAUDE.md |
| 11 | Integration tests in `tests/`, unit tests with `#[cfg(test)]` | Rust ch.5.3 |
| 12 | No premature optimization; profile before optimizing | Rust ch.3 |
| 13 | Zero external font dependencies (no ttf-parser, fontdue, rusttype) | design requirement |
| 14 | Workspace `rustfmt.toml` inherited: `max_width=100`, 4-space, Unix LF, stable-only; no per-crate `rustfmt.toml` override | workspace file |

### Format Compliance

Workspace `rustfmt.toml` settings inherited by all crates:
- `max_width = 100` — narrower than Rust default; all code, comments, doc-strings ≤100 columns
- `reorder_imports = true` — stable-only import reordering (no nightly `imports_granularity` or `group_imports`)
- `edition = "2021"`, `tab_spaces = 4`, `hard_tabs = false`, `newline_style = "Unix"`
- `use_small_heuristics = "Default"`
- No per-crate `rustfmt.toml` — workspace file governs all crates
- `cargo fmt --check` must pass; `scripts/lint.sh` enforces this

## 10. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Hinting divergence | Pixel-level mismatch at small sizes | Phase 6 is dedicated tuning loop; test at exact PIL sizes |
| cmap format gaps | Some fonts won't map glyphs | Prioritize format 4 + 12 (99% coverage); add format 2/6/0 only if needed |
| Composite glyph complexity | Outline loading bugs | Unit test each composite composition case |
| Bézier flattening tolerance | Coverage differs at edges | Match ftgrays.c split condition exactly (deviation < 1 sub-pixel) |
| FreeType version drift | PIL 12.2.0 vs newer FreeType differ | Generate refs from exact PIL version in .venv; freeze in coverage_matrix.json |
| Performance | Renderer slower than fontdue | Acceptable trade-off for parity; profile in Phase 7, optimize hot path if needed |

## 11. Success Criteria

1. **100% ImageFont parity coverage** — all 8 functions have ≥1 passing PIL parity test
2. **Pixel-identical getmask output** — SHA-256 matches PIL FreeType for all 7,000 matrix rows
3. **Zero external font dependencies** — `cargo tree -p pillow-rs-font` shows no `ttf-parser`, `fontdue`, `freetype-sys`, `rusttype`
4. **`cargo clippy` clean** — `-D warnings` passes on all targets
5. **PIL fallback removed** — `imagefont.py` no longer imports/uses `PIL.ImageFont`
6. **All existing text-draw tests pass** — `ImageDraw.text`, `ImageDraw.multiline_text` produce correct images
