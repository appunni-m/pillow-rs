# String Assertion No-Op Report

**Date:** 2026-06-21
**Severity:** HIGH — 201 tests pass without validating any output

## 2026-07-23 Follow-up

The prefix fallback described below is no longer an accepted fix. Fixture
values now preserve scalar and container types, error types and messages are
exact, images compare mode/size/palette/pixels, and fixture integrity rejects
prefix, tolerance, substring-error, stale, missing, and orphaned assertions.

The audit also found that the earlier `instance_method_bytes` workaround did
not test `Image.getdata`, `Image.load`, or `Image.getim`; it compared the source
image bytes instead. Those operations now exercise the returned sequence,
pixel-access object, or opaque-result type directly. Font exceptions are no
longer normalized to `[]` or `None`. Mutating calls now compare both their
return values and the resulting pixels instead of accepting `None` alone.
Pillow `ImagingCore` font masks are no longer converted into `Image` objects:
font fixtures record the concrete return type, mode, size, and bytes so a
wrapper-type mismatch remains a visible parity failure.

Strict fixture coverage initially exposed 12 operations with no fixture and 32
declared-mode gaps. The coverage gate now rejects input images and top-level
mode labels that the selected call style does not consume. Exact property,
base-font, and transposed-font fixtures were added; `open`, `frombytes`,
`fromarray`, and `merge` now exercise their real modes. Unsupported or
mode-independent declarations were removed from the manifest instead of being
papered over with relabeled cases.

## Root Cause

`tests/engine.py:508-510`:

```python
"string": lambda case, result:
    str(result).startswith(case.get("prefix", ""))   # ← always True
    or repr(result) == case.get("value", ""),         # ← dead code
```

`"".startswith("")` is always `True`, so `or` short-circuits. The `value` comparison never executes. Zero fixtures define a `prefix` field.

## Impact

**201 of 1547 tests (13%) are complete no-ops.** They pass regardless of what the implementation returns.

### Category 1: Memory-address values — 97 tests

These store `<Object at 0x...>` in `value`. Even if the `or` branch ran, the address changes every run and could never match.

| Operation | Cases | Value pattern |
|-----------|-------|---------------|
| `Image.getdata` | 8 | `<ImagingCore object at 0x...>` |
| `Image.load` | 14 | `<PixelAccess object at 0x...>` |
| `Image.getim` | 4 | `<capsule object "Pillow Imaging" at 0x...>` |
| `ImageFont.getmask` | 8 | `<ImagingCore object at 0x...>` |
| `ImageFont.getmask2` | 8 | `(<ImagingCore object at 0x...>, (0, 2))` |
| `ImageDraw.getfont` | 4 | `<PIL.ImageFont.FreeTypeFont object at 0x...>` |
| `ImageFont.FreeTypeFont` | 6 | `<PIL.ImageFont.FreeTypeFont object at 0x...>` |
| `ImageFont.ImageFont` | 6 | `<PIL.ImageFont.ImageFont object at 0x...>` |
| `ImageFont.load_default` | 8 | `<PIL.ImageFont.FreeTypeFont object at 0x...>` |
| `ImageFont.load_default_imagefont` | 6 | `<PIL.ImageFont.ImageFont object at 0x...>` |
| `ImageFont.load` (suite1) | 4 | font name tuples (already in Category 2) |

**Full list — `getdata`:**
- `Image_getdata_L`, `Image_getdata_LA`, `Image_getdata_RGB`, `Image_getdata_RGBA`
- `Image_getdata_L_suite1`, `Image_getdata_LA_suite1`, `Image_getdata_RGB_suite1`, `Image_getdata_RGBA_suite1`

**Full list — `load`:**
- `Image_load_1`, `Image_load_L`, `Image_load_LA`, `Image_load_P`, `Image_load_RGB`, `Image_load_RGBA`
- `Image_load_1_suite1`, `Image_load_L_suite1`, `Image_load_LA_suite1`, `Image_load_P_suite1`, `Image_load_RGB_suite1`, `Image_load_RGBA_suite1`

**Full list — `getim`:**
- `Image_getim_L`, `Image_getim_RGB`, `Image_getim_L_suite1`, `Image_getim_RGB_suite1`

**Full list — `getfont`:**
- `ImageDraw_getfont_L`, `ImageDraw_getfont_RGB`, `ImageDraw_getfont_L_suite1`, `ImageDraw_getfont_RGB_suite1`

**Full list — `getmask`:**
- `default`, `getmask_L`, `getmask_RGB`, `getmask_RGBA`
- `default_suite1`, `getmask_L_suite1`, `getmask_RGB_suite1`, `getmask_RGBA_suite1`

**Full list — `getmask2`:**
- `default`, `getmask2_L`, `getmask2_RGB`, `getmask2_RGBA`
- `default_suite1`, `getmask2_L_suite1`, `getmask2_RGB_suite1`, `getmask2_RGBA_suite1`

**Full list — `FreeTypeFont`:**
- `FreeTypeFont_L`, `FreeTypeFont_RGBA`, `dejavu_sans_16`, `liberation_serif_20`
- `FreeTypeFont_L_suite1`, `FreeTypeFont_RGBA_suite1`, `dejavu_sans_16_suite1`, `liberation_serif_20_suite1`

**Full list — `ImageFont`:**
- `ImageFont_ImageFont_L`, `ImageFont_ImageFont_RGB`, `ImageFont_ImageFont_RGBA`
- `ImageFont_ImageFont_RGB_suite1`, `ImageFont_ImageFont_RGBA_suite1`

**Full list — `load_default`:**
- `ImageFont_load_default_L`, `ImageFont_load_default_RGB`, `ImageFont_load_default_RGBA`
- `ImageFont_load_default_L_suite1`, `ImageFont_load_default_RGB_suite1`, `ImageFont_load_default_RGBA_suite1`

**Full list — `load_default_imagefont`:**
- `ImageFont_load_default_imagefont_L`, `ImageFont_load_default_imagefont_RGB`, `ImageFont_load_default_imagefont_RGBA`
- `ImageFont_load_default_imagefont_L_suite1`, `ImageFont_load_default_imagefont_RGB_suite1`, `ImageFont_load_default_imagefont_RGBA_suite1`

### Category 2: Real values but never compared — 104 tests

These store valid expected values (tuples, strings, numbers) in `value`, but `"".startswith("")` short-circuits before `repr(result) == value` is ever reached.

| Operation | Cases | What's stored (ignored) |
|-----------|-------|-------------------------|
| `Image.getbands` | 12 | `('1',)`, `('L',)`, `('L', 'A')`, `('P',)`, `('R', 'G', 'B')`, `('R', 'G', 'B', 'A')` |
| `Image.getbbox` | 10 | `(0, 0, 256, 256)`, `(0, 0, 10, 10)`, `(0, 0, 32, 32)` |
| `Image.getextrema` | 13 | `(0, 255)`, `((0,255), (0,255), ...)`, per-mode extrema |
| `Image.getpixel` | 3 | `(255, 255)`, `(255, 255, 255)`, `(255, 255, 255, 255)` |
| `Image.getprojection` | 12 | Hash projection `[1, 1, 1, ...]` arrays |
| `Image.get_flattened_data` | 4 | Large pixel value tuples |
| `ImageColor.getcolor` | 4 | `(255, 0, 0)` |
| `ImageColor.getrgb` | 4 | `(255, 0, 0)` |
| `ImageDraw.textbbox` | 6 | `(5, 7, 30, 15)` |
| `ImageDraw.multiline_textbbox` | 4 | `(5, 7, 30, 15)` |
| `ImageFont.getbbox` | 8 | `(0, 2, 25, 10)` |
| `ImageFont.getmetrics` | 8 | `(10, 3)` |
| `ImageFont.getname` | 8 | `('Aileron', 'Regular')` |
| `ImageFont.font_variant` | 6 | `('Aileron', 'Regular')` |
| `ImageFont.load` | 8 | `('Liberation Serif', 'Regular')`, `('DejaVu Sans', 'Book')` |
| `ImageFont.load_path` | 6 | `('Liberation Serif', 'Regular')` |
| `ImageFont.truetype` | 6 | `('Liberation Serif', 'Regular')` |
| `ImagePalette.getdata` | 4 | `('RGB', b'')` |

## The Problem in Context

Most Category 1 tests (`getdata`, `load`, `getim`, font constructors) return internal Rust/FFI objects that can't be meaningfully serialized. Their fixture values are `<Object at 0x...>` — Python `repr()` of a memory address. These tests were always no-ops by design, relying on `startswith("")` to "pass" them.

Category 2 tests (`getbands`, `getbbox`, `getextrema`, etc.) have real values in their fixtures that should be validated but never are.

## Fix

In `tests/engine.py`, change the `string` assertion from:

```python
"string": lambda case, result:
    str(result).startswith(case.get("prefix", ""))
    or repr(result) == case.get("value", ""),
```

To:

```python
"string": lambda case, result:
    repr(result) == case.get("value", "")
    or str(result).startswith(case.get("prefix", "")),
```

This makes `value` comparison the primary check. The `prefix` fallback remains for cases that genuinely need it (add `"prefix"` to fixture JSON for any object-identity tests that should continue passing by prefix match).

Category 1 tests (object identity) will then need either:
- A `"prefix"` field added (e.g., `"prefix": "<ImagingCore object"`) to match by class name
- Or a different assertion method that validates the object type structurally
