# WASM Test Infrastructure — Review Issues

Generated 2026-06-15. Covers `wasm_backend.mjs`, `pillow-rs-js/src/lib.rs`, `test.html`, `run_wasm_test.mjs`, `execution_engine.mjs`, `test_fixture_parity.py`.

## Current Status (2026-06-15 EOD)

| Config | Pass | Fail | Skip | Total |
|--------|------|------|------|-------|
| Node.js WASM | 829 | 51 | 0 | 880 |
| Browser WASM | 428 | 4 | 132 | 564 |
| Python parity | 668 | 0 | 31 xfail | 699 |

**51 Node.js failures:** ~35 core algorithm gaps (xfail in Python), ~5 remaining JS fixes (getpalette, getcolors, getfont, alpha_composite RGBA).

---

## 1. CRITICAL — Test Validation Broken

### 1.1 Browser value comparison always passes
**File:** `pillow-rs-js/tests/browser/wasm_browser.test.mjs:278-287`

```javascript
if (valuesEqual(result.value, expected.value)) {
    passed++;
} else {
    if (sv !== ev) { passed++; } else { passed++; }
    // ^ BOTH branches increment passed unconditionally
}
```
All 251 value-type fixtures in the browser pass without any validation.

### 1.2 Node.js object comparison always passes
**File:** `pillow-rs-js/tests/run_wasm_test.mjs:212-214`

```javascript
const expStr = String(expVal);  // "[object Object]"
const actStr = String(actual);  // "[object Object]"
if (expStr === actStr) return { pass: true };  // always true
```
Affects 13 fixtures: all `ImageStat_Stat_*` (11) + `Image_getxmp_*` (2).

### 1.3 Python test suite uses xfail for all failures
**File:** `tests/test_fixture_parity.py:64,75,78,85,88,121,135`

Every failure path calls `pytest.xfail()` — never `pytest.fail()` or `assert`. Suite always exits 0 even if every test is broken. No quality gate.

### 1.4 Null/undefined results silently skipped
**File:** `pillow-rs-js/tests/run_wasm_test.mjs:321-324`

```javascript
if ((result === null || result === undefined) && expected.result_type !== 'value') {
    skipped++;  // should be failed++
}
```

---

## 2. HIGH — Wrong Values / Stubs

### 2.1 ImageStat backend returns hardcoded stubs
**File:** `pillow-rs-js/tests/wasm_backend.mjs:1248-1258`

| Field | WASM returns | Should return |
|-------|-------------|---------------|
| median | `stat.mean` | `stat.median` |
| rms | `stat.mean` | `stat.rms` |
| var | `[0]` | `stat.var` |
| stddev | `[0]` | `stat.stddev` |
| extrema | `[[0, 0]]` | `stat.extrema` |

### 2.2 ImageStat class in WASM bindings is a complete stub
**File:** `pillow-rs-js/src/lib.rs:791-812`

`count()` always 0, `sum()` always 0.0, `mean()` always 0.0. Should delegate to `core::Image::stat()`.

### 2.3 ImagePalette class in WASM bindings is non-functional
**File:** `pillow-rs-js/src/lib.rs`

Constructor initializes empty data. `tobytes()` returns empty `[]`. `getpalette()` returns debug string, not palette bytes. `putpalette()` is a no-op. Should delegate to core.

### 2.4 `point()` default LUT shifts pixel values
**File:** `pillow-rs-js/tests/wasm_backend.mjs:432-434`

```javascript
lut[i] = Math.min(255, i + 50);  // wrong default — should be identity
```

### 2.5 RankFilter default rank = 0 (should be 2)
**File:** `pillow-rs-js/tests/wasm_backend.mjs:692`

### 2.6 Kernel default scale = 1.0 (should auto-scale)
**File:** `pillow-rs-js/tests/wasm_backend.mjs:697`

PIL auto-scales kernel weights to sum. WASM hardcodes `1.0`.

---

## 3. HIGH — Business Logic in JS, Should Be in Rust

### 3.1 `_resolveColorName` — JS color name resolution
**File:** `pillow-rs-js/tests/wasm_backend.mjs:115-137`

Only supports 9 named colors. PIL supports 140+. Core has `parse_color_str()` that handles all of them. JS should delegate.

### 3.2 `regular_polygon` vertex computation in JS
**File:** `pillow-rs-js/tests/wasm_backend.mjs:1007-1053`

40 lines of cos/sin, PIL rounding, vertex calculation. Duplicates core `Draw::regular_polygon()`.

### 3.3 Color assembly in WASM bindings
**File:** `pillow-rs-js/src/lib.rs` (every draw method)

```rust
let fill = fr.map(|r| (r, fg.unwrap_or(0), fb.unwrap_or(0), fa.unwrap_or(255)));
```
Defaulting logic for missing color channels lives in the binding. Should be in `core::color`.

### 3.4 `crop` coordinate conversion in WASM binding
**File:** `pillow-rs-js/src/lib.rs:59`

```rust
self.inner.crop((l, t, r - l, b - t))  // math in binding
```
Python passes the box as-is to core.

### 3.5 `call_draw` switch statement — 14 manual cases
**File:** `pillow-rs-js/tests/wasm_backend.mjs:816-1068` (253 lines)

Each draw target manually extracts coords via `_extractCoords`, converts colors via `_colorToRGBA`, assembles positional args. Python does this in 6 lines with `getattr(draw, target)(**p)`. The WASM bindings' positional-only API forces this duplication.

---

## 4. HIGH — Hardcoded Values in WASM Bindings

### 4.1 `transform` fill hardcoded to black
**File:** `pillow-rs-js/src/lib.rs:410`
```rust
self.inner.transform_affine(..., (0, 0, 0, 255))  // always black
```
Python accepts `fillcolor` parameter.

### 4.2 Draw methods hardcode width = 1
**File:** `pillow-rs-js/src/lib.rs` (line, rectangle, ellipse, polygon, arc, chord, pieslice, circle, rounded_rectangle)

All 9 draw methods pass hardcoded `1` for width. Python exposes `width` as optional parameter.

### 4.3 `rotate` hardcodes `expand=false, fillcolor=None`
**File:** `pillow-rs-js/src/lib.rs:66`

### 4.4 `quantize` hardcodes `kmeans=0, palette=None, dither=true`
**File:** `pillow-rs-js/src/lib.rs:339`

### 4.5 `thumbnail` hardcodes `filter=None`
**File:** `pillow-rs-js/src/lib.rs:377`

### 4.6 ImageChops `add`/`subtract` hardcode `scale=1.0, offset=0.0`
**File:** `pillow-rs-js/src/lib.rs:910,916`

### 4.7 `ImageOps::fit` hardcodes `filter=None, bleed=0.0, centering=(0.5,0.5)`
**File:** `pillow-rs-js/src/lib.rs:1108-1109`

### 4.8 `ImageOps::pad` hardcodes `filter=None, centering=(0.5,0.5)`
**File:** `pillow-rs-js/src/lib.rs:1114-1116`

### 4.9 `ImageOps::contain`/`cover`/`scale` missing `filter` parameter
**File:** `pillow-rs-js/src/lib.rs:1096,1102,1121`

---

## 5. MEDIUM — Behavioral Differences vs Python

### 5.1 Browser has independent execution engine reimplementation
**File:** `pillow-rs-js/tests/browser/test.html`

The browser defines its own `execute()` and all dispatch methods inline. No code shared with `execution_engine.mjs` + `wasm_backend.mjs`. Fixes must be applied in both places.

### 5.2 `putpixel` always passes 4 values regardless of mode
**File:** `pillow-rs-js/tests/wasm_backend.mjs:367`
Python adjusts to mode band count (1 for L, 2 for LA, 3 for RGB, 4 for RGBA).

### 5.3 `draft` hardcoded no-op
**File:** `pillow-rs-js/tests/wasm_backend.mjs:379`
Python passes actual params via `getattr(img, target)(**p)`.

### 5.4 Dual 1-bit prep missing `dither="NONE"`
**File:** `pillow-rs-js/tests/wasm_backend.mjs:726`
WASM calls `img.convert("1")` without explicit dither. Python uses `convert("1", dither="NONE")`.

### 5.5 `alpha_composite` returns null (should return mutated image)
**File:** `pillow-rs-js/tests/wasm_backend.mjs:314`
Python returns the mutated image for result chaining.

### 5.6 `ImageChops.colorize` color resolution differs
Python hardcodes `"black"` and `"white"` (ignores fixture params). WASM reads params. They'll differ when params specify custom colors.

### 5.7 Text metric stubs hardcoded to `[0, 0, 50, 15]`
**File:** `pillow-rs-js/tests/wasm_backend.mjs:821`
Works for test fixtures that don't compare text metrics, wrong for anything that does.

### 5.8 `eval` passes precomputed LUT instead of callable
**File:** `pillow-rs-js/tests/wasm_backend.mjs:1153`
Works only because core accepts both. Fragile.

### 5.9 ImageFont class name stubs hardcoded strings
**File:** `pillow-rs-js/tests/wasm_backend.mjs:1234-1241`
Returns `"FreeTypeFont"` / `"ImageFont"` strings instead of loading actual font module.

### 5.10 `getexif` returns hardcoded empty TIFF header
**File:** `pillow-rs-js/tests/wasm_backend.mjs:1312-1314`

### 5.11 `size` called as function `img.size()` — may fail if property
**File:** `pillow-rs-js/tests/wasm_backend.mjs:1285`

### 5.12 `text`/`multiline_text` hardcoded default font
**File:** `pillow-rs-js/tests/wasm_backend.mjs:989`
Ignores any font family/size from fixture params.

### 5.13 `ImageOps.colorize` color resolution in JS
**File:** `pillow-rs-js/tests/wasm_backend.mjs` (`_resolveColorName`)
Should delegate to core `parse_color_str`.

---

## 6. MEDIUM — Test Runner Issues

### 6.1 Lossy tolerance threshold generous (5% at >2 byte diff)
**File:** `pillow-rs-js/tests/run_wasm_test.mjs:73-101`
For 100x100 RGB (30,000 bytes): up to 1,500 bytes can differ by >2 levels. May mask real bugs.

### 6.2 6 ImagePalette fixtures lack `reference_bytes`
**File:** `tests/fixtures/ImagePalette_copy_*.json`, `ImagePalette_tobytes_*.json`
Lossy tolerance path would misinterpret hash string as pixel data if these fixtures were added to `LOSSY_OPS`.

### 6.3 Dead code: unused `hash()` function
**File:** `pillow-rs-js/tests/run_wasm_test.mjs:46-57`

---

## 7. LOW — Missing WASM Bindings vs Python

### 7.1 Missing `explicit_mode` on Image
Python exposes `Image.explicit_mode` — WASM has no equivalent.

### 7.2 Missing `format` getter on Image
Python exposes `Image.format` → `format_name()`. WASM not present.

### 7.3 Missing `getpixel_formatted` (mode-aware return)
WASM `getpixel` always returns 4-element `Vec<u8>`. Python returns mode-appropriate values.

### 7.4 Missing `stat()` method on Image
Python has `Image.stat()` and `Image.stat_formatted()`. WASM has a separate `ImageStat` stub class.

### 7.5 Missing `multiline_text` on ImageDraw
WASM has no `multiline_text` binding. Python implements it fully.

### 7.6 Missing `paste` mask parameter
WASM splits into `pasteImage`/`pasteColor`, both hardcode `mask=None`. Python handles mask.

---

## 8. Known Core Algorithm Differences (xfail in Python)

These are real algorithm gaps — WASM matches Python (same core), but both differ from PIL:

| Operation | Mode | Issue |
|-----------|------|-------|
| Draw (all shapes) | I | 32-bit int precision lost in RGBA roundtrip |
| Enhance (all) | CMYK | CMYK enhancement doesn't match PIL |
| Filter DETAIL/EMBOSS/SMOOTH_MORE | I | I-mode precision |
| ImageChops invert | LA, RGBA | Alpha channel not inverted |
| ImageOps expand | LA, RGBA | Fill color for alpha modes |
| ImageOps grayscale | P | P-mode grayscale |
| ImageChops difference/subtract/add_modulo/subtract_modulo | LA, RGBA | Alpha channel handling |
| ImageFilter EMBOSS/FIND_EDGES/UnsharpMask/ModeFilter | LA, RGBA | Alpha channel handling |
| ImageFilter BLUR | 1, CMYK | Mode-specific filter |
| ImageFilter SHARPEN | CMYK, I | Mode-specific filter |
| effect_spread | all | glibc rand() replaced with GlibcRand PRNG |
| remap_palette | L, P | Inverse-LUT construction |
| quantize | all | Median-cut vs PIL NeuQuant |
| transform | all | Affine algorithm |
