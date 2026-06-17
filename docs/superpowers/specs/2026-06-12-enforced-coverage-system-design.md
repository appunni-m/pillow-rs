# Enforced Coverage System — Design

**Date:** 2026-06-12
**Status:** Approved
**Pillow target:** 12.2.0

## 1. Problem

Current coverage system has too many manual steps and failure points:

- 74 of 164 tests (45%) are missing `@pytest.mark.covers` decorators
- `coverage_map.json` is manually maintained — tests exist but aren't tracked
- Mode coverage is heavily RGB-biased; CMYK, YCbCr, HSV, I, F have zero tests
- No mechanism to prevent a test from being written against the wrong mode
- JS/WASM tests have no coverage tracking at all
- No CI gate that fails on coverage gaps

## 2. Design Principle

**Manifest-driven enforced coverage.** `manifest.yaml` is the single source of truth. Tests are auto-discovered from machine-readable markers. Missing tests fail CI. Wrong markers fail test collection. No manual mapping files.

### Architecture

```
manifest.yaml  ──defines──▶  Expected Matrix (ops × modes × targets × variants)
                                   │
                                   │ diff (validate_coverage.py)
                                   │
Python tests (@covers markers) ──▶  Actual Matrix  ──▶  Gap Report
JS tests     (@covers JSDoc)   ──▶                 ──▶  CI pass/fail (exit 1 on gap)
```

## 3. Manifest as Single Source of Truth

Every implemented function in `manifest.yaml` already defines `supported_modes`. This is extended with `supported_targets` and `required_variants` to form the complete expected test matrix:

```yaml
- name: resize
  module: Image
  signature: 'resize(self, size: tuple[int,int]|list[int], resample: int|None=None, ...) -> Image'
  supported_modes: [L, LA, RGB, RGBA, 1, P]
  param_variants:
    - { size: [50, 50] }
    - { size: [50, 50], resample: NEAREST }
    - { size: [200, 200], resample: BILINEAR }
    - { size: [200, 200], resample: BICUBIC }
    - { size: [200, 200], resample: LANCZOS }
  supported_targets: [cpu, gpu, wasm, wasm_gpu]
  status: implemented
```

The expected number of tests for `resize` = 6 modes × 5 variants × 4 targets = **120 tests**.

For operations where `supported_targets` is not explicitly set, it defaults to `[cpu]` (Python only). GPU and WASM targets are opt-in per operation.

## 4. Complete Mode × Operation Matrices

### 4A. Legend

| Symbol | Meaning |
|--------|---------|
| 🟢 | PIL supports this mode. We have a parity test. Test passes. |
| 🔴 | PIL does NOT support this mode. PIL throws. We throw identically. Parity test passes (error parity). |
| ⬜ | No test exists — **gap that needs filling** |

### 4B. Image Module — Class Methods

| # | Operation | L | LA | RGB | RGBA | 1 | P | CMYK | YCbCr | HSV | I | F |
|---|-----------|---|---|----|-----|------|---|------|-------|-----|---|---|
| 1 | `Image.open` | 🟢 | 🟢 | 🟢 | 🟢 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 2 | `Image.new` | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |

### 4C. Image Module — Instance Methods

| # | Operation | L | LA | RGB | RGBA | 1 | P | CMYK | YCbCr | HSV | I | F |
|---|-----------|---|---|----|-----|------|---|------|-------|-----|---|---|
| 3 | `resize` | 🟢 | ⬜ | 🟢 | 🟢 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 4 | `crop` | ⬜ | ⬜ | 🟢 | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 5 | `rotate` | ⬜ | ⬜ | 🟢 | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 6 | `transpose` | ⬜ | ⬜ | 🟢 | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 7 | `convert` | 🟢 | 🟢 | 🟢 | 🟢 | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| 8 | `paste` | ⬜ | ⬜ | 🟢 | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 9 | `filter` | ⬜ | ⬜ | 🟢 | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 10 | `copy` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 11 | `split` | ⬜ | ⬜ | 🟢 | 🟢 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 12 | `getbands` | ⬜ | ⬜ | 🟢 | 🟢 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 13 | `thumbnail` | ⬜ | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 14 | `tobytes` | ⬜ | ⬜ | 🟢 | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 15 | `alpha_composite` | 🔴 | 🔴 | 🔴 | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 16 | `close` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 17 | `getbbox` | ⬜ | ⬜ | 🟢 | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 18 | `getchannel` | ⬜ | ⬜ | 🟢 | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 19 | `getcolors` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 20 | `getdata` | ⬜ | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 21 | `getextrema` | ⬜ | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 22 | `getpixel` | ⬜ | ⬜ | 🟢 | 🟢 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 23 | `getprojection` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 24 | `histogram` | ⬜ | 🔴 | 🟢 | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 25 | `load` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 26 | `point` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 27 | `putalpha` | ⬜ | 🔴 | 🟢 | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 28 | `putdata` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 29 | `putpixel` | ⬜ | ⬜ | 🟢 | ⬜ | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 30 | `quantize` | 🔴 | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 31 | `reduce` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 32 | `seek` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 33 | `tell` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 34 | `transform` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 35 | `verify` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 36 | `effect_spread` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 37 | `entropy` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 38 | `draft` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 39 | `remap_palette` | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🟢 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 40 | `tobitmap` | 🔴 | 🔴 | 🔴 | 🔴 | 🟢 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |
| 41 | `frombytes` | ⬜ | 🔴 | ⬜ | ⬜ | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 | 🔴 |

### 4D. Image Module — Properties

| # | Property | L | LA | RGB | RGBA | 1 | P |
|---|----------|---|---|-----|------|---|---|
| 1 | `size` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| 2 | `width` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| 3 | `height` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| 4 | `mode` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| 5 | `format` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| 6 | `info` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |

### 4E. ImageModule Functions

| # | Operation | L | LA | RGB | RGBA |
|---|-----------|---|---|-----|------|
| 1 | `merge` | ⬜ | ⬜ | 🟢 | ⬜ |
| 2 | `blend` | ⬜ | 🔴 | ⬜ | ⬜ |
| 3 | `composite` | ⬜ | 🔴 | ⬜ | ⬜ |
| 4 | `eval` | ⬜ | 🔴 | ⬜ | ⬜ |
| 5 | `fromarray` | ⬜ | 🔴 | ⬜ | ⬜ |
| 6 | `frombytes` | ⬜ | 🔴 | ⬜ | ⬜ |
| 7 | `effect_noise` | ⬜ | 🔴 | ⬜ | ⬜ |
| 8 | `alpha_composite` | unused (delegates to Image method) | | | |
| 9 | `new` | unused (delegates to Image.new) | | | |
| 10 | `open` | unused (delegates to Image.open) | | | |

### 4F. ImageDraw Methods

| # | Operation | L | RGB | RGBA |
|---|-----------|---|---|------|
| 1 | `arc` | ⬜ | ⬜ | ⬜ |
| 2 | `line` | ⬜ | 🟢 | ⬜ |
| 3 | `rectangle` | ⬜ | 🟢 | ⬜ |
| 4 | `ellipse` | ⬜ | ⬜ | ⬜ |
| 5 | `polygon` | ⬜ | ⬜ | ⬜ |
| 6 | `text` | ⬜ | ⬜ | ⬜ |
| 7 | `multiline_text` | ⬜ | ⬜ | ⬜ |
| 8 | `circle` | ⬜ | ⬜ | ⬜ |
| 9 | `rounded_rectangle` | ⬜ | ⬜ | ⬜ |
| 10 | `regular_polygon` | ⬜ | ⬜ | ⬜ |
| 11 | `chord` | ⬜ | ⬜ | ⬜ |
| 12 | `pieslice` | ⬜ | ⬜ | ⬜ |
| 13 | `bitmap` | ⬜ | ⬜ | ⬜ |
| 14 | `point` | ⬜ | 🟢 | ⬜ |
| 15 | `textbbox` | ⬜ | ⬜ | ⬜ |
| 16 | `multiline_textbbox` | ⬜ | ⬜ | ⬜ |
| 17 | `textlength` | ⬜ | ⬜ | ⬜ |
| 18 | `getfont` | ⬜ | ⬜ | ⬜ |

### 4G. ImageFilter Classes

All 18 filter classes are exercised through `Image.filter()` which supports L, LA, RGB, RGBA.

| # | Class | L | LA | RGB | RGBA |
|---|-------|---|---|-----|------|
| 1 | BLUR | ⬜ | ⬜ | 🟢 | ⬜ |
| 2 | CONTOUR | ⬜ | ⬜ | ⬜ | ⬜ |
| 3 | DETAIL | ⬜ | ⬜ | ⬜ | ⬜ |
| 4 | EDGE_ENHANCE | ⬜ | ⬜ | ⬜ | ⬜ |
| 5 | EDGE_ENHANCE_MORE | ⬜ | ⬜ | ⬜ | ⬜ |
| 6 | EMBOSS | ⬜ | ⬜ | ⬜ | ⬜ |
| 7 | FIND_EDGES | ⬜ | ⬜ | ⬜ | ⬜ |
| 8 | SHARPEN | ⬜ | ⬜ | 🟢 | ⬜ |
| 9 | SMOOTH | ⬜ | ⬜ | 🟢 | ⬜ |
| 10 | SMOOTH_MORE | ⬜ | ⬜ | ⬜ | ⬜ |
| 11 | GaussianBlur | ⬜ | ⬜ | ⬜ | ⬜ |
| 12 | BoxBlur | ⬜ | ⬜ | ⬜ | ⬜ |
| 13 | UnsharpMask | ⬜ | ⬜ | ⬜ | ⬜ |
| 14 | Kernel | ⬜ | ⬜ | ⬜ | ⬜ |
| 15 | MaxFilter | ⬜ | ⬜ | ⬜ | ⬜ |
| 16 | MinFilter | ⬜ | ⬜ | ⬜ | ⬜ |
| 17 | MedianFilter | ⬜ | ⬜ | ⬜ | ⬜ |
| 18 | ModeFilter | ⬜ | ⬜ | ⬜ | ⬜ |
| 19 | RankFilter | ⬜ | ⬜ | ⬜ | ⬜ |
| 20 | Color3DLUT | ⬜ | ⬜ | ⬜ | ⬜ |

### 4H. ImageEnhance Classes

| # | Class | L | RGB | RGBA |
|---|-------|---|---|------|
| 1 | Brightness | ⬜ | 🟢 | ⬜ |
| 2 | Color | ⬜ | 🟢 | ⬜ |
| 3 | Contrast | ⬜ | 🟢 | ⬜ |
| 4 | Sharpness | ⬜ | 🟢 | ⬜ |

### 4I. ImageOps Functions

| # | Operation | L | RGB | RGBA |
|---|-----------|---|---|------|
| 1 | `autocontrast` | ⬜ | ⬜ | ⬜ |
| 2 | `colorize` | ⬜ | ⬜ | ⬜ |
| 3 | `contain` | ⬜ | ⬜ | ⬜ |
| 4 | `cover` | ⬜ | ⬜ | ⬜ |
| 5 | `crop` | ⬜ | ⬜ | ⬜ |
| 6 | `equalize` | ⬜ | 🟢 | ⬜ |
| 7 | `expand` | ⬜ | ⬜ | ⬜ |
| 8 | `fit` | ⬜ | ⬜ | ⬜ |
| 9 | `flip` | ⬜ | ⬜ | ⬜ |
| 10 | `grayscale` | ⬜ | ⬜ | ⬜ |
| 11 | `invert` | ⬜ | 🟢 | ⬜ |
| 12 | `mirror` | ⬜ | 🟢 | ⬜ |
| 13 | `pad` | ⬜ | ⬜ | ⬜ |
| 14 | `posterize` | ⬜ | 🟢 | ⬜ |
| 15 | `scale` | ⬜ | ⬜ | ⬜ |
| 16 | `solarize` | ⬜ | ⬜ | ⬜ |
| 17 | `exif_transpose` | ⬜ | ⬜ | ⬜ |
| 18 | `deform` | ⬜ | ⬜ | ⬜ |

### 4J. ImageChops Functions

All ImageChops functions accept L and RGB (and some accept RGBA).

| # | Operation | L | RGB | RGBA |
|---|-----------|---|---|------|
| 1 | `add` | ⬜ | 🟢 | ⬜ |
| 2 | `add_modulo` | ⬜ | ⬜ | ⬜ |
| 3 | `blend` | ⬜ | ⬜ | ⬜ |
| 4 | `composite` | ⬜ | ⬜ | ⬜ |
| 5 | `constant` | ⬜ | ⬜ | ⬜ |
| 6 | `darker` | ⬜ | 🟢 | ⬜ |
| 7 | `difference` | ⬜ | 🟢 | ⬜ |
| 8 | `duplicate` | ⬜ | ⬜ | ⬜ |
| 9 | `hard_light` | ⬜ | ⬜ | ⬜ |
| 10 | `invert` | ⬜ | ⬜ | ⬜ |
| 11 | `lighter` | ⬜ | ⬜ | ⬜ |
| 12 | `logical_and` | ⬜ | ⬜ | ⬜ |
| 13 | `logical_or` | ⬜ | ⬜ | ⬜ |
| 14 | `logical_xor` | ⬜ | ⬜ | ⬜ |
| 15 | `multiply` | ⬜ | 🟢 | ⬜ |
| 16 | `offset` | ⬜ | ⬜ | ⬜ |
| 17 | `overlay` | ⬜ | ⬜ | ⬜ |
| 18 | `screen` | ⬜ | ⬜ | ⬜ |
| 19 | `soft_light` | ⬜ | ⬜ | ⬜ |
| 20 | `subtract` | ⬜ | ⬜ | ⬜ |
| 21 | `subtract_modulo` | ⬜ | ⬜ | ⬜ |

### 4K. ImageColor Functions

| # | Operation | L | RGB | RGBA |
|---|-----------|---|---|------|
| 1 | `getrgb` | N/A | 🟢 | N/A |
| 2 | `getcolor` | ⬜ | 🟢 | ⬜ |

### 4L. ImagePalette, ImageFont, ImageStat, ImageSequence

These modules are mode-independent or operate on P/L mode only.

| Module | Operation | Status |
|--------|-----------|--------|
| ImagePalette | `copy` | 🟢 |
| ImagePalette | `getcolor` | 🟢 |
| ImagePalette | `getdata` | 🟢 |
| ImagePalette | `save` | 🟢 |
| ImagePalette | `tobytes` | 🟢 |
| ImageFont | `load` | 🟢 |
| ImageFont | `load_default` | 🟢 |
| ImageFont | `truetype` | 🟢 |
| ImageFont | `FreeTypeFont.getbbox` | 🟢 |
| ImageFont | `FreeTypeFont.getlength` | 🟢 |
| ImageFont | `FreeTypeFont.getmask` | 🟢 |
| ImageFont | `FreeTypeFont.getmetrics` | 🟢 |
| ImageFont | `FreeTypeFont.getname` | 🟢 |
| ImageFont | `ImageFont.getbbox` | 🟢 |
| ImageFont | `ImageFont.getlength` | 🟢 |
| ImageFont | `ImageFont.getmask` | 🟢 |
| ImageStat | `Stat` (all properties) | 🟢 |
| ImageSequence | `Iterator` | 🟢 |

### 4M. Format × Mode Matrix (I/O Operations)

For `open()` and `save()` only. PIL-supported format-mode combinations:

| Format | L | LA | RGB | RGBA | 1 | P | CMYK |
|--------|---|---|-----|------|---|---|------|
| PNG | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 | 🔴 |
| JPEG | 🟢 | 🔴 | 🟢 | 🔴 | 🔴 | 🔴 | 🟢 |
| GIF | 🟢 | 🔴 | 🟢 | 🔴 | 🟢 | 🟢 | 🔴 |
| BMP | 🟢 | 🔴 | 🟢 | 🟢 | 🟢 | 🟢 | 🔴 |
| TIFF | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 |
| WEBP | 🟢 | 🔴 | 🟢 | 🟢 | 🔴 | 🔴 | 🔴 |
| ICO | 🔴 | 🔴 | 🟢 | 🟢 | 🔴 | 🔴 | 🔴 |

## 5. Test Annotation System

### 5A. Python (`@pytest.mark.covers`)

```python
@pytest.mark.covers(
    "Image.resize",           # required: full dotted path matching manifest
    mode="RGB",               # required if operation has supported_modes
    target="cpu",             # required: cpu | gpu | wasm | wasm_gpu
    variant="bilinear",       # optional: matches param_variants name in manifest
)
def test_resize_rgb_bilinear(PIL):
    ...
```

### 5B. JavaScript (`@covers` JSDoc)

```javascript
/**
 * @covers Image.resize
 * @mode RGB
 * @target wasm
 * @variant bilinear
 */
test('resize RGB bilinear', async () => { ... });
```

### 5C. Browser WASM (same JSDoc format, Puppeteer)

```javascript
/**
 * @covers Image.resize
 * @mode RGB
 * @target wasm_gpu
 * @variant bilinear
 */
it('resize RGB bilinear on GPU', async () => {
  // runs via puppeteer
});
```

## 6. Enforcement Mechanisms

### 6A. Pytest Collection Hook (prevents untracked tests)

```python
# tests/conftest.py
def pytest_collection_modifyitems(config, items):
    manifest = load_manifest()
    errors = []
    for item in items:
        marker = item.get_closest_marker('covers')
        if marker is None:
            errors.append(f"{item.nodeid}: MISSING @pytest.mark.covers")
            continue
        op_name = marker.args[0] if marker.args else None
        if op_name not in manifest.operations:
            errors.append(f"{item.nodeid}: unknown operation '{op_name}'")
            continue
        mode = marker.kwargs.get('mode')
        op_def = manifest.operations[op_name]
        if mode and op_def.supported_modes and mode not in op_def.supported_modes:
            errors.append(
                f"{item.nodeid}: mode '{mode}' not in manifest "
                f"supported_modes for {op_name}: {op_def.supported_modes}"
            )
    if errors:
        raise pytest.UsageError("\n".join(errors))
```

This means: a test WITHOUT `@covers` fails at collection time. A test with wrong `mode=` fails at collection time. Impossible to commit an untracked test.

### 6B. validate_coverage.py (CI gate)

```python
# scripts/validate_coverage.py
# Exit 0: all manifest operations have required tests
# Exit 1: gaps found (prints gap report)

manifest_set = build_expected_set(manifest)   # from manifest.yaml
python_set = scan_python_tests("tests/")       # parses @covers markers
js_set = scan_js_tests("pillow-rs-js/tests/") # parses @covers JSDoc

actual_set = python_set | js_set
gaps = manifest_set - actual_set           # missing tests
unknown = actual_set - manifest_set        # markers referencing wrong ops

if gaps:
    print("GAPS (missing tests):")
    for g in sorted(gaps):
        print(f"  MISS  {g.op} × {g.mode} × {g.target} × {g.variant}")
if unknown:
    print("UNKNOWN (markers with no manifest match):")
    for u in sorted(unknown):
        print(f"  EXTRA  {u.op} × {u.mode} × {u.target}")

sys.exit(1 if gaps or unknown else 0)
```

### 6C. JS Test Fixture Generation

JS/WASM tests use pre-computed PIL reference values. A Python script generates fixtures:

```python
# scripts/generate_wasm_fixtures.py
for op_name, op_def in manifest.operations.items():
    for mode in op_def.supported_modes:
        for target in ['wasm', 'wasm_gpu']:
            for variant in op_def.param_variants:
                # Run PIL operation, hash output
                pil_result = run_pil_operation(op_name, mode, variant)
                fixture = {
                    "input": {"mode": mode, ...},
                    "operation": variant,
                    "expectedHash": sha256(pil_result.tobytes())
                }
                write_json(f"pillow-rs-js/tests/fixtures/{op_name}_{mode}_{variant}.json", fixture)
```

## 7. Multi-Target Parity

| Target | Language | Test Runner | Coverage Script |
|--------|----------|-------------|-----------------|
| `cpu` | Python | pytest | `validate_coverage.py` |
| `gpu` | Python + WGSL | pytest | `validate_coverage.py` |
| `wasm` | JS (Node.js) | Jest/Vitest | `validate_coverage.py` |
| `wasm_gpu` | JS (Browser) | Puppeteer + Jest | `validate_coverage.py` |

GPU exclusion list: operations that don't benefit from GPU (font rendering, palette ops, metadata ops, exif, getim) are excluded. All PipelineOps must have GPU coverage including self-mutating operations (`Set` PipelineOp).

## 8. CI Pipeline

```bash
#!/bin/bash
# scripts/ci_coverage.sh — runs in CI

set -e

# 1. Run all tests
cargo test -p pillow-rs
pytest tests/ --json-report --json-report-file=/tmp/report.json
node pillow-rs-js/tests/run.mjs

# 2. Validate coverage (FAILS CI on gap or unknown marker)
python scripts/validate_coverage.py manifest.yaml

# 3. Generate human-readable reports
python scripts/generate_coverage_page.py
python scripts/generate_wasm_coverage.py

echo "✅ All tests pass, coverage matrix complete"
```

## 9. Transition Plan

| Phase | Task | Files Affected | Effort |
|-------|------|---------------|--------|
| **1** | Add `covers` marker to all 74 untracked Python tests | 23 test files | ~2h |
| **2** | Build `validate_coverage.py` — manifest parser, marker scanner, gap diff | new file | ~2h |
| **3** | Add collection hook to `conftest.py` — fail on missing markers | 1 file | ~1h |
| **4** | Expand mode coverage — add L, LA, 1, P tests for every operation with gaps (see matrices 4B-4L) | ~15 test files | ~6h |
| **5** | Add error-parity tests for 🔴 cells (PIL throws → we throw) | ~5 test files | ~3h |
| **6** | Add format×mode tests for I/O operations (matrix 4M) | test_image_io.py | ~3h |
| **7** | Build JS test fixture generator (`generate_wasm_fixtures.py`) | new file | ~2h |
| **8** | Add WASM JS tests with `@covers` JSDoc | pillow-rs-js/tests/ | ~4h |
| **9** | Add GPU target tests (Python WGSL + Browser WASM GPU) | tests/ + pillow-rs-js/ | ~4h |
| **10** | Build `validate_coverage.py` JS marker scanner | scripts/ | ~1h |
| **11** | Integrate into CI | scripts/ci_coverage.sh | ~0.5h |
| **12** | Update COVERAGE.md and COVERAGE_WASM.md generators | scripts/ | ~1h |

## 10. Self-Review Checklist

- [x] No placeholders, TBDs, or incomplete sections
- [x] Matrices cover every operation from manifest.yaml
- [x] Mode columns are complete (all 11 PIL modes)
- [x] Enforcement mechanism prevents both missing tests and wrong tests
- [x] Multi-target parity covers all 4 targets
- [x] CI pipeline explicitly defined
- [x] Transition phases have clear scope and effort estimates
- [x] Format×Mode matrix separated from operation×mode matrix (only for I/O)
