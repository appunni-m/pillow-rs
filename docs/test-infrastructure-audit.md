# Test Infrastructure Audit — PIL vs RSPIL Parity

Date: 2026-06-16

## Purpose

This audit reviews every test in the pillow-rs test suite to verify that each test genuinely compares RSPIL Rust code against PIL Python output. It identifies hardcoded stubs, weak assertions, and gaps where Rust code paths are never exercised.

## Architecture Overview

```
manifest.yaml ──→ ops_registry.py ──→ generate_fixtures.py ──→ fixtures/*.json
                     │                        │                      │
                     │                   [pil_backend.py]            │
                     │                   PIL executes op            │
                     │                   captures hash              │
                     │                                              │
                     └──── test_fixture_parity.py ←─────────────────┘
                              │
                         [rspil_backend.py]
                         RSPIL executes op
                         compares hash to fixture
```

The expected flow:
1. Fixture generator runs PIL backend → PIL executes operation → captures SHA-256 hash of output
2. Test runs RSPIL backend → RSPIL executes operation → compares hash to fixture
3. `rspil_backend.py` dispatches to actual `pillow_rs` Rust code

## 🔴 CRITICAL: Tests that bypass Rust code entirely

These tests use hardcoded return values in `rspil_backend.py` — the Rust implementation is **never reached**.

### ImageColor.getcolor (`rspil_backend.py:394`)

```python
if module == "ImageColor":
    return (255, 0, 0)  # stub
```

- **Ignores** the `color` and `mode` parameters completely
- Rust `getcolor()` at `lib.rs:1800` is **not registered** in the Python module (missing `m.add_function`)
- `ImageColor.getcolor('red', 'RGB')` **crashes at runtime** with `module 'pillow_rs._core' has no attribute 'getcolor'`
- Test only passes because fixture uses `color='red', mode='RGB'` which coincidentally equals `(255,0,0)`
- **Any other color would fail**

### ImageColor.getrgb

Same stub pattern. `getrgb` IS registered in lib.rs but the rspil_backend never calls it.

### ImagePalette (`rspil_backend.py:396-400`)

```python
if target == "getcolor": return 0           # hardcoded
if target == "getdata": return ['RGB', '']  # hardcoded
if target == "copy": return bytes()         # hardcoded
if target == "tobytes": return bytes()      # hardcoded
```

- All 5 ImagePalette tests are **fake** — zero Rust coverage
- `palette_getcolor` exists in Rust (`color.rs`) but is never tested via fixtures

### ImageDraw.getfont (`rspil_backend.py:186-195`)

```python
if target == "getfont":
    import PIL.ImageFont
    font = PIL.ImageFont.load_default()
    glyph_img = PIL.Image.new('L', (50, 50), 0)
    d = PIL.ImageDraw.Draw(glyph_img)
    d.text((5, 5), 'Ay', font=font, fill=255)
    return glyph_img.tobytes()
```

- **Uses PIL to render the glyph**, not RSPIL
- This is a PIL-vs-PIL comparison, not RSPIL-vs-PIL
- Rust `Font::load_default()` + `render_text()` code path: **0% fixture coverage**

### textlength / textbbox (`rspil_backend.py:292`)

```python
if target in ("textbbox", "multiline_textbbox", "textlength"):
    return (0, 0, 50, 15) if "bbox" in target or "length" in target else None
```

- Returns hardcoded `(0, 0, 50, 15)` — Rust text measurement is **never called**

### alpha_composite (`rspil_backend.py:109-123`)

Manual PIL-based workaround instead of calling Rust `alpha_composite`. The Rust code path is not tested via fixtures.

### Stub-only properties

| Property | Hardcoded value | Location |
|----------|----------------|----------|
| `has_transparency_data` | `False` | L352 |
| `is_animated` | `False` | L350 |
| `n_frames` | `1` | L351 |
| `get_child_images` | `[]` | L348 |
| `getxmp` | `{}` | L346 |
| `palette` | `None` | L349 |
| `apply_transparency` | `None` | L353 |
| `show` | `None` | L353 |
| `close` | `None` | L353 |

## 🟡 MODERATE: Weak assertions that skip real comparison

### Image.split

```python
# test_fixture_parity.py L128-134
if len(actual) > 0 and hasattr(actual[0], 'tobytes') and isinstance(val, list):
    if len(actual) != len(val):
        pytest.xfail(f"split: expected {len(val)} bands, got {len(actual)}")
    for i, band in enumerate(actual):
        try: band.tobytes()
        except: pytest.xfail(f"split: band {i} has no tobytes")
    return  # Split result is valid (same band count, images have bytes)
```

- Checks **band count** matches
- Checks `tobytes()` doesn't throw
- **Never compares pixel data** — split() could produce wrong pixels and still pass

### Image.getdata

```python
# test_fixture_parity.py L117-118
if isinstance(val, str) and val.startswith("<ImagingCore") and isinstance(actual, list):
    return
```

- Type-checks `ImagingCore` vs `list`
- **Never compares actual data values**

### Image.load

```python
# test_fixture_parity.py L114-115
if isinstance(val, str) and val.startswith("<PixelAccess") and hasattr(actual, '__str__') and str(actual).startswith("<PixelAccess"):
    return
```

- String prefix match only — any object returning `"<PixelAccess...`"` passes

### Image.getim

```python
# test_fixture_parity.py L120-121
if isinstance(val, str) and val.startswith("<capsule object") and isinstance(actual, str) and actual.startswith("<capsule object"):
    return
```

- Capsule address changes each run, so type-match is the best we can do — **acceptable**

### Float tolerance

```python
if abs(actual - val) < 0.01: return
```

- Acceptable for float point operations, but no documentation of which ops produce floats

## 🟠 MODERATE: Backend logic divergence

### `_to_rgb_fill` inconsistency

**pil_backend.py** (L79-90):
```python
def _to_rgb_fill(mode, params, keys):
    p = copy.deepcopy(params)
    for k in keys:
        if k in p and mode in ("RGB", "RGBA") and isinstance(p[k], int):
            p[k] = (0, 255, 0)  # visible green for test visibility
    return p
```
PIL handles int fills natively via `_getink`. Only converts for RGB/RGBA.

**rspil_backend.py** (L24-44):
```python
for k in keys:
    if k in p and isinstance(p[k], int):
        v = p[k]
        if mode in ("RGB", "RGBA"):
            p[k] = (0, 255, 0)
        elif is_text and mode in ("LA", "CMYK", "1", "P"):
            p[k] = (v, v, v, 255)  # ← DIFFERENT: converts int→tuple
```
Converts int fills to tuples for text on LA, CMYK, 1, P modes.

**Impact**: For P-mode text, PIL receives `fill=200` (int, direct palette index) while RSPIL receives `fill=(200,200,200,255)` (tuple). The Rust text_compose_direct code strips `fill.0` to get 200. **Works by coincidence** — the fill value maps to the same palette index. Change the fill and it could diverge.

### bitmap convert("1") diverges

**pil_backend.py** (L247-248):
```python
if target == "bitmap":
    bmp = img.convert("1", dither=PIL.Image.Dither.NONE) if img.mode != "1" else make_image("1")
```
Has special case for mode "1" input (creates fresh image instead of converting).

**rspil_backend.py** (L201-203):
```python
if target == "bitmap":
    bmp = img.convert("1", dither="NONE")
```
Always converts from the source image. Could differ for mode "1" inputs.

## 🔵 GAPS: Manifest / Registry / Fixture alignment

| Metric | Count |
|--------|-------|
| Implemented ops in manifest.yaml | 182 |
| Ops in ops_registry REGISTRY | 178 |
| Ops with JSON fixtures | 153 |
| Ops without fixtures | 25 |
| Fixture ops not in manifest | 2 |

### 2 fixture ops not tracked in manifest:
- `Image.has_transparency_data`
- `ImageSequence.all_frames`

These appear in coverage warnings:
```
UNKNOWN op 'ImageSequence.all_frames' in @covers
UNKNOWN op 'Image.has_transparency_data' in @covers
```

### Unregistered Python binding

`getcolor` is defined at `pillow-rs-py/src/lib.rs:1800` but **never registered** with `m.add_function()`. `getrgb` IS registered at L823. The Python `ImageColor.getcolor()` crashes because `_core.getcolor` doesn't exist.

## 🟢 VERIFIED: Genuine RSPIL-vs-PIL tests

These operation categories genuinely exercise Rust code against PIL output:

| Category | How tested | Status |
|----------|-----------|--------|
| Image.resize/crop/rotate/transpose | `getattr(img, target)(**params)` → Rust | ✓ Genuine |
| Image.convert | calls `img.convert(mode)` → Rust | ✓ Genuine |
| Image.filter (all filters) | calls `img.filter(type)` → Rust | ✓ Genuine |
| Image.quantize | calls `img.quantize(colors)` → Rust | ✓ Genuine |
| ImageDraw (vector ops) | calls `draw.line/circle/etc()` → Rust | ✓ Genuine |
| ImageDraw.text (L, LA, CMYK, RGB, RGBA) | calls `draw.text()` → Rust via fontdue/BitmapFont | ✓ Genuine |
| ImageDraw.text (1, P) | calls `draw.text()` with binary glyphs → Rust | ✓ Genuine (since fix) |
| ImageDraw.bitmap | calls `draw.bitmap()` → Rust | ✓ Genuine |
| ImageOps (grayscale, autocontrast, etc.) | calls `ImageOps.xxx(img)` → Rust | ✓ Genuine |
| ImageEnhance | calls `ImageEnhance.xxx(img).enhance(f)` → Rust | ✓ Genuine |
| ImageChops | calls `ImageChops.xxx(img1, img2)` → Rust | ✓ Genuine |
| ImageStat | calls `ImageStat.Stat(img)` → Rust | ✓ Genuine |
| Image.new / Image.frombytes | calls `Image.new/frombytes()` → Rust | ✓ Genuine |

## Recommended Fixes (priority order)

### 1. Register `getcolor` in lib.rs (blocks all ImageColor tests)

```rust
// Add this near the other m.add_function calls:
m.add_function(wrap_pyfunction!(getcolor, m)?)?;
```

### 2. Remove hardcoded stubs from rspil_backend.py

Replace each stub with actual dispatch to pillow_rs:

| Current stub | Replace with |
|-------------|-------------|
| `return (255, 0, 0)  # stub` | `return ImageColor.getcolor(params['color'], params.get('mode', 'RGB'))` |
| `if target == "getcolor": return 0` | Call `ImagePalette` method |
| `if target == "getdata": return ['RGB', '']` | Call `ImagePalette` method |
| `return (0, 0, 50, 15)` | Call `font.textbbox(text)` or `font.textlength(text)` |
| PIL glyph rendering for getfont | Use `ImageFont.load_default()` from RSPIL, render with RSPIL Draw |

### 3. Strengthen weak assertions

- **split**: Compare each band's `tobytes()` hash against expected per-band hashes
- **getdata**: Compare the actual data list against expected values
- **load**: Compare pixel access values, not just string prefix

### 4. Unify `_to_rgb_fill` between backends

Either:
- Pass int fills directly to Rust (let Rust handle per-mode semantics like PIL does), OR
- Both backends use the same conversion logic

### 5. Add fixtures for 25 missing ops

Run `generate_fixtures.py` after fixing the registry, then verify the new fixtures pass.

### 6. Add manifest entries for untracked ops

Add `Image.has_transparency_data` and `ImageSequence.all_frames` to manifest.yaml with proper mode lists and status.

## Test Count History

| Date | Passed | XFAIL | Notes |
|------|--------|-------|-------|
| 2026-06-15 (before) | 642 | 12 | bitmap CMYK/P, text 1/P, etc. |
| 2026-06-16 (round 1) | 650 | 4 | Fixed convert("1"), hsv_to_rgb |
| 2026-06-16 (round 2) | 670 | 6 | Fixed binary fontmode for text on 1/P |
| Target | 676 | 0 | After fixing all identified stubs |
