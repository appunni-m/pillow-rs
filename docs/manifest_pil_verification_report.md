# Manifest.yaml vs PIL API — Verification Report

> **Generated:** 2026-06-15 | **PIL Version:** 12.2.0 | **Source:** Official Pillow docs + installed PIL source at `.venv/lib/python3.12/site-packages/PIL/`

## Executive Summary

The manifest.yaml was verified against actual PIL 12.2.0 source code and documentation by 8 parallel research agents. Each agent fetched official docs and read installed PIL source to cross-reference.

**Overall assessment:** The manifest is ~85-90% accurate. The most impactful issues are:

1. **`open`/`new` wrongly classified as `Image.class_methods`** — they are module-level functions in PIL
2. **`supported_modes` are systematically under-reported** across most entries
3. **Several functions have placeholder `(*args, **kwargs)` signatures** that should have real signatures
4. **Missing functions:** `has_transparency_data()`, `linear_gradient()`, `radial_gradient()`, `ImageSequence.all_frames()`, FreeTypeFont methods, `TransposedFont` class
5. **Mode listing inconsistencies** between ImageChops and ImageModule for the same underlying function

---

## Module-by-Module Findings

### 1. Image Class — 12 issues found

| # | Severity | Issue | Detail |
|---|----------|-------|--------|
| 1 | **HIGH** | `open`/`new` classified as `class_methods` | In PIL these are **module-level functions only**, not methods on the `Image.Image` class. Remove from `class_methods`, keep in `ImageModule.functions` |
| 2 | **HIGH** | `has_transparency_data()` missing | Real PIL method: `has_transparency_data(self) -> bool`. Not in manifest at all |
| 3 | **HIGH** | `linear_gradient()`, `radial_gradient()` missing from ImageModule | Real PIL module-level functions for creating gradients |
| 4 | **MEDIUM** | `supported_modes` incomplete for ~25 methods | Most methods work on ALL modes but manifest only lists a subset. E.g., `copy()`, `split()`, `getbands()`, `getpixel()`, `putpixel()`, `load()`, `tobytes()` all work on every mode |
| 5 | **MEDIUM** | 8 methods have placeholder `(*args, **kwargs)` signatures | `getexif`, `getim`, `getxmp`, `getpalette`, `putpalette`, `apply_transparency`, `get_child_images`, `get_flattened_data` all have real signatures in PIL |
| 6 | **LOW** | `resize` size type missing `NumpyArray` | PIL accepts `NumpyArray` for size |
| 7 | **LOW** | `transform` method type missing `SupportsGetData` | PIL accepts more method types |
| 8 | **LOW** | `open` `fp` param missing `IO[bytes]` | PIL accepts file objects, not just paths |
| 9 | **LOW** | `open` formats list too narrow | Manifest: 7 formats. PIL supports 30+ (AVIF, EPS, PDF, PSD, DDS, etc.) |
| 10 | **LOW** | `frombytes` instance method missing default | `decoder_name="raw"` default not shown |
| 11 | **LOW** | `resize` missing `F` mode | PIL supports F mode for resize |
| 12 | **LOW** | `rotate`/`thumbnail`/`transpose`/`reduce`/`transform`/`effect_spread` missing modes | Missing: F, HSV, I, YCbCr in various combinations |

**Missing from ImageModule.functions:** `fromarrow`, `fromqimage`, `fromqpixmap`, `getmodebase`, `getmodetype`, `getmodebandnames`, `getmodebands`

---

### 2. ImageDraw — 6 issues found

| # | Severity | Issue | Detail |
|---|----------|-------|--------|
| 1 | **MEDIUM** | `getfont` signature wrong | Manifest: `getfont(*args, **kwargs)`. PIL: `getfont()` — no parameters |
| 2 | **LOW** | `shape` signature wrong | Manifest: `shape(*args, **kwargs)`. PIL: `shape(shape, fill=None, outline=None)` |
| 3 | **LOW** | `text()` missing `font_size` param | PIL accepts `font_size` via `**kwargs` and documents it |
| 4 | **LOW** | Drawing modes (`&id003`) missing `I`, `F` | PIL ImageDraw has explicit handling for I and F modes |
| 5 | **LOW** | `textbbox`/`textlength`/`multiline_textbbox` modes too narrow | Manifest: `RGB, RGBA, L`. These are measurement-only methods — any mode works |
| 6 | **LOW** | Missing module-level functions | `Draw()`, `floodfill()`, `getdraw()` exist in PIL |

**All 19 methods are real PIL methods** — no fake entries.

---

### 3. ImageFilter — systematic mode under-reporting

**Root cause:** The manifest assumes each built-in filter has different mode support, but PIL implements all built-in constants identically through the same `MultibandFilter` base class. They share one code path.

| Filter Group | Manifest Modes | Actual PIL Modes | Missing |
|-------------|---------------|------------------|---------|
| **All 10 built-in constants** (BLUR, CONTOUR, DETAIL, EDGE_ENHANCE, EDGE_ENHANCE_MORE, EMBOSS, FIND_EDGES, SHARPEN, SMOOTH, SMOOTH_MORE) | Varies (mostly `*id002` = L,LA,RGB,RGBA) | `1, L, LA, RGB, RGBA, CMYK, I, PA, HSV, YCbCr` | Varies — most are missing `1, CMYK, I, PA, HSV, YCbCr` |
| **GaussianBlur, BoxBlur, UnsharpMask** | `*id002` (L,LA,RGB,RGBA) | `L, LA, RGB, RGBA, CMYK` | Missing `CMYK` |
| **Kernel** | L,LA,RGB,RGBA | `1, L, LA, RGB, RGBA, CMYK, I, PA, HSV, YCbCr` | Missing `1, CMYK, I, PA, HSV, YCbCr` |
| **MaxFilter, MinFilter, MedianFilter, RankFilter** | L,LA,RGB,RGBA | `1, L, LA, RGB, RGBA, CMYK, I, F, PA, HSV, YCbCr` | Missing `1, CMYK, I, F, PA, HSV, YCbCr` |
| **ModeFilter** | L,LA,RGB,RGBA | `1, L, LA, RGB, RGBA, CMYK, P, PA, HSV, YCbCr` | Missing `1, CMYK, P, PA, HSV, YCbCr` |
| **Color3DLUT** | *(none listed)* | `RGB, RGBA, CMYK, HSV, YCbCr` | No modes listed at all |

**Also:** `Image.filter()` param_variants missing 3 filters: `RankFilter`, `Kernel`, `Color3DLUT`.

---

### 4. ImageEnhance — all 4 classes missing CMYK

| Class | Manifest Modes | Actual PIL Modes | Missing |
|-------|---------------|------------------|---------|
| Brightness | `*id002` (L,LA,RGB,RGBA) | `L, LA, RGB, RGBA, CMYK` | Missing `CMYK` |
| Color | `*id002` (L,LA,RGB,RGBA) | `L, LA, RGB, RGBA, CMYK` | Missing `CMYK` |
| Contrast | `*id002` (L,LA,RGB,RGBA) | `L, LA, RGB, RGBA, CMYK` | Missing `CMYK` |
| Sharpness | `*id002` (L,LA,RGB,RGBA) | `L, LA, RGB, RGBA, CMYK` | Missing `CMYK` |

---

### 5. ImageOps — 3 mode issues

| # | Severity | Issue | Detail |
|---|----------|-------|--------|
| 1 | **CRITICAL** | `invert` missing modes | Manifest: L,LA,RGB,RGBA. PIL also handles **CMYK, I, F, 1** |
| 2 | **LOW** | `posterize` missing P mode | PIL handles P mode (converts to RGB internally) |
| 3 | **LOW** | `solarize` missing I, F modes | PIL handles these |

All 18 functions exist in PIL and signatures are accurate. No missing functions.

---

### 6. ImageChops — 5 mode inconsistencies

| # | Severity | Issue | Detail |
|---|----------|-------|--------|
| 1 | **MEDIUM** | `blend` modes too narrow | Manifest: `*id001` (L,RGB). PIL: matches `ImageModule.blend` (CMYK, L, LA, RGB, RGBA). **Internally inconsistent with own ImageModule entry** |
| 2 | **MEDIUM** | `composite` modes too narrow | Manifest: L,P,RGB. PIL: matches `ImageModule.composite` (1, CMYK, L, LA, P, RGB, RGBA). **Internally inconsistent** |
| 3 | **LOW** | `duplicate` too narrow | Manifest: L,P,RGB. PIL `image.copy()` supports all modes |
| 4 | **LOW** | `invert` missing LA, RGBA | PIL C-level `chop_invert` supports these |
| 5 | **LOW** | `constant` modes misleading | PIL ignores input mode entirely — only reads `.size`, output is always L |

All 21 functions exist in PIL with correct signatures. No fake functions. `hard_light`, `overlay`, `soft_light` are all real/verified.

---

### 7. ImageColor — PASS (0 issues)

Both `getrgb` and `getcolor` match PIL exactly. No discrepancies.

---

### 8. ImagePalette — PASS on methods, 5 unlisted module functions

All 5 listed methods match PIL. However PIL has module-level factory functions not tracked:
- `negative(mode="RGB") -> ImagePalette`
- `random(mode="RGB") -> ImagePalette`
- `sepia(white="#fff0c0") -> ImagePalette`
- `wedge(mode="RGB") -> ImagePalette`
- `load(filename) -> tuple[bytes, str]`

These are debatable — they live in `PIL.ImagePalette` namespace but aren't imported at top level.

---

### 9. ImageFont — 2 HIGH signature issues + 6 missing FreeTypeFont methods

| # | Severity | Issue | Detail |
|---|----------|-------|--------|
| 1 | **HIGH** | `load_default_imagefont` signature wrong | Manifest: `(*args, **kwargs)`. PIL: `load_default_imagefont()` — **zero parameters** |
| 2 | **HIGH** | `load_path` signature wrong | Manifest: `(*args, **kwargs)`. PIL: `load_path(filename: str \| bytes) -> ImageFont` |
| 3 | **MEDIUM** | 6 FreeTypeFont methods missing | `getmask2`, `font_variant`, `get_variation_names`, `set_variation_by_name`, `get_variation_axes`, `set_variation_by_axes` |
| 4 | **MEDIUM** | `TransposedFont` class entirely missing | PIL has this class with `getmask`, `getbbox`, `getlength` methods |

---

### 10. ImageModule — 5 issues

| # | Severity | Issue | Detail |
|---|----------|-------|--------|
| 1 | **HIGH** | `new`/`open` duplicated from class_methods | These are module-level functions only — remove from `Image.class_methods` |
| 2 | **MEDIUM** | `alpha_composite` stub signature | Manifest: `(*args, **kwargs)`. PIL: `alpha_composite(im1: Image, im2: Image) -> Image` |
| 3 | **MEDIUM** | `effect_mandelbrot` stub signature | Manifest: `(*args, **kwargs)`. PIL: `effect_mandelbrot(size, extent, quality) -> Image` (3 required params) |
| 4 | **MEDIUM** | `frombuffer` stub signature | Manifest: `(*args, **kwargs)`. PIL: `frombuffer(mode, size, data, decoder_name="raw", *args)` |
| 5 | **LOW** | `effect_noise` modes wrong | Output is always L mode (noise centered at 128), not the 7 modes listed |
| 6 | **LOW** | `frombytes` data type missing `SupportsArrayInterface` | PIL accepts array-like objects |

**Missing module-level functions:** `linear_gradient`, `radial_gradient`, `fromarrow`, `getmodebase`, `getmodetype`, `getmodebandnames`, `getmodebands`

---

### 11. ImageStat — 2 issues

| # | Severity | Issue | Detail |
|---|----------|-------|--------|
| 1 | **MEDIUM** | `supported_modes` too restrictive | Manifest: L, RGB, RGBA. PIL: **all modes** (zero mode checking in `Stat.__init__`) |
| 2 | **LOW** | Constructor not documented | `Stat(image_or_list, mask=None)` accepts Image OR precomputed histogram list |

---

### 12. ImageSequence — 3 issues

| # | Severity | Issue | Detail |
|---|----------|-------|--------|
| 1 | **HIGH** | `all_frames()` function missing | PIL: `all_frames(im, func=None) -> list[Image]` — entirely absent from manifest |
| 2 | **MEDIUM** | Iterator methods not listed | PIL Iterator has `__getitem__`, `__iter__`, `__next__` — manifest lists bare class |
| 3 | **MEDIUM** | Iterator `supported_modes` wrong | Manifest: `*id001` (L, RGB). PIL: ANY mode (only checks `hasattr(im, "seek")`) |

---

## Cross-Cutting Issues

### YAML Anchor/Reference Chain

The manifest uses YAML anchors `&id001`, `&id002`, `&id003` for mode lists:
- `&id001` = `[L, RGB]` — used by get_child_images, get_flattened_data, getexif, getim, getxmp, ImageChops blend/overlay, ImageSequence Iterator
- `&id002` = `[L, LA, RGB, RGBA]` — used by apply_transparency, frombytes (Image), most ImageFilter entries, all ImageEnhance entries, most ImageOps entries
- `&id003` = `[1, L, LA, P, RGB, RGBA, CMYK]` — used by all ImageDraw methods

**Issue:** These anchors encode mode lists that are inaccurate for many of their consumers (see module sections above). Fixing the anchors would fix multiple entries at once, but the correct mode list often differs per function.

### "N/A" in COVERAGE.md vs Manifest

The COVERAGE.md matrix marks many mode×operation cells as "N/A = PIL doesn't support this mode." Our research shows many of these N/A claims are **incorrect** — PIL actually supports those modes. Examples:

| Operation | Mode marked N/A | PIL actually supports? |
|-----------|----------------|----------------------|
| `copy` | CMYK, HSV, I, F, YCbCr | YES — copy() works on every mode |
| `split` | CMYK, HSV, I, F, YCbCr | YES — split() uses `self.im.bands` |
| `getbands` | CMYK, HSV, I, F, YCbCr | YES — works on every mode |
| `load` | CMYK, HSV, I, F, YCbCr | YES — works on every mode |
| `tobytes` | CMYK, HSV, I, F, YCbCr | YES — works on every mode |
| `getpixel` | CMYK, HSV, I, F, YCbCr | YES — works on every mode |
| `putpixel` | CMYK, F, I | YES — works on every mode |

### Duplicate Listings

Several functions appear in multiple places:
- `open`/`new`: both `Image.class_methods` AND `ImageModule.functions` → should be module-level only
- `alpha_composite`: both `Image.methods` AND `ImageModule.functions` → both exist in PIL, correct
- `frombytes`: both `Image.methods` AND `ImageModule.functions` → both exist, correct
- `composite`, `blend`: ImageModule functions AND ImageChops wrappers → both real, correct

---

## Quick-Fix Prioritization

### High Priority (wrong API surface)
1. Remove `open`/`new` from `Image.class_methods` — keep only in `ImageModule.functions`
2. Add `has_transparency_data()` to Image.methods
3. Add `linear_gradient()` and `radial_gradient()` to ImageModule.functions
4. Add `all_frames()` to ImageSequence
5. Fix `load_default_imagefont()` and `load_path()` signatures (remove `*args, **kwargs`)
6. Add missing FreeTypeFont methods

### Medium Priority (inaccurate modes)
7. Update `&id001` and `&id002` anchors with correct mode lists
8. Fix ImageChops mode lists to match ImageModule equivalents
9. Fix ImageFilter mode lists (all built-in constants support same modes)
10. Fix ImageStat and ImageSequence mode lists

### Low Priority (minor type issues)
11. Replace placeholder `(*args, **kwargs)` signatures with real ones
12. Fix `text()` missing `font_size` parameter
13. Add `SupportsArrayInterface` to `frombytes` data type
14. Add `NumpyArray` to `resize` size type

---

## Agents Deployed

| Agent | Module(s) | Status |
|-------|-----------|--------|
| 1 | Image class (48 entries) | ✅ Complete |
| 2 | ImageDraw (19 entries) | ✅ Complete |
| 3 | ImageFilter (20) + ImageEnhance (4) | ✅ Complete |
| 4 | ImageOps (18 functions) | ✅ Complete |
| 5 | ImageChops (21 functions) | ✅ Complete |
| 6 | ImageColor + ImagePalette + ImageFont | ✅ Complete |
| 7 | ImageModule (12 functions) | ✅ Complete |
| 8 | ImageStat + ImageSequence | ✅ Complete |
