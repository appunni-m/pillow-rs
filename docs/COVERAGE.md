# pillow-rs Coverage Report

> Auto-generated: 2026-06-14 11:51:03 | Pillow parity tested

## Trust Summary

| Metric | Value |
|--------|-------|
| **Total tests** | 566 |
| **Passing** | 486 |
| **Failed** | 0 |
| **Skipped** | 80 |
| **Implemented functions** | 145 |
| **Trusted (PIL parity tested)** | 143 |
| **Untested** | 2 |
| **Stubs** | 5 |
| **Trust score** | **143/145 (99%)** |

## Performance Benchmarks

*Multiple = PIL time / pillow-rs time. >1.0 = pillow-rs is faster.*

| Operation | Speedup | Faster? |
|-----------|---------|---------|
| resize_2k_to_1k | 25.34× | ✅ |
| crop_2k | 0.87× | ❌ |
| convert_2k_RGB_to_L | 2.33× | ✅ |
| transpose_2k_FLIP | 2.62× | ✅ |
| filter_2k_BLUR | 65.19× | ✅ |
| paste_2k | 1.20× | ✅ |
| invert_2k | 3.82× | ✅ |

**Average speedup: 14.48×**

## Module Status

| Module | Implemented | Trusted | Untested | Trust % |
|--------|------------|---------|----------|---------|
| Image | 48 | 46 | 2 | 96% |
| ImageChops | 21 | 21 | 0 | 100% |
| ImageColor | 2 | 2 | 0 | 100% |
| ImageDraw | 14 | 14 | 0 | 100% |
| ImageEnhance | 4 | 4 | 0 | 100% |
| ImageFilter | 19 | 19 | 0 | 100% |
| ImageFont | 5 | 5 | 0 | 100% |
| ImageModule | 9 | 9 | 0 | 100% |
| ImageOps | 16 | 16 | 0 | 100% |
| ImagePalette | 5 | 5 | 0 | 100% |
| ImageSequence | 1 | 1 | 0 | 100% |
| ImageStat | 1 | 1 | 0 | 100% |

## ⚠️ Untested Functions

- `Image.getdata`
- `Image.quantize`

## ⬜ Remaining Stubs

- `Image.toqimage`
- `Image.toqpixmap`
- `ImageDraw.shape`
- `ImageModule.effect_mandelbrot`
- `ImageModule.frombuffer`

## Mode × Operation Coverage Matrix

*✅ = passing, ⚠️ = xfailed (in progress), ⬜ = supported but not tested, N/A = PIL doesn't support this mode*

### Image

| Operation | 1 | L | LA | P | RGB | RGBA | CMYK | HSV | I | F |
|-----------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|
| `alpha_composite` | N/A | N/A | N/A | N/A | N/A | ⬜ | N/A | N/A | N/A | N/A |
| `apply_transparency` | N/A | ✅ | ⬜ | N/A | ✅ | ⬜ | N/A | N/A | N/A | N/A |
| `convert` | N/A | ✅ | ✅ | ⚠️ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `copy` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `crop` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `draft` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `effect_spread` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `entropy` | ✅ | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `filter` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | N/A | N/A | N/A |
| `frombytes` | N/A | ⚠️ | ⚠️ | N/A | ✅ | ⚠️ | N/A | N/A | N/A | N/A |
| `get_child_images` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `get_flattened_data` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getbands` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getbbox` | ⬜ | ✅ | ✅ | ⬜ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getchannel` | N/A | N/A | ✅ | N/A | ✅ | ✅ | ⬜ | N/A | N/A | N/A |
| `getcolors` | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getdata` | N/A | ⚠️ | ⚠️ | N/A | ⚠️ | ⚠️ | N/A | N/A | N/A | N/A |
| `getexif` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getextrema` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getim` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getpalette` | N/A | ✅ | N/A | ⬜ | N/A | N/A | N/A | N/A | N/A | N/A |
| `getpixel` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getprojection` | ⬜ | ✅ | N/A | ⬜ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getxmp` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `histogram` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `load` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `new` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `open` | N/A | ⚠️ | ⬜ | N/A | ✅ | ⬜ | N/A | N/A | N/A | N/A |
| `paste` | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ | N/A | N/A | N/A |
| `point` | ⬜ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | N/A | N/A | N/A | N/A |
| `putalpha` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `putdata` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `putpalette` | N/A | ⬜ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A | N/A |
| `putpixel` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `quantize` | N/A | ⚠️ | N/A | N/A | ⚠️ | ⚠️ | N/A | N/A | N/A | N/A |
| `reduce` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `remap_palette` | N/A | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A | N/A |
| `resize` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `rotate` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `seek` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `split` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `tell` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `thumbnail` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `tobitmap` | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `tobytes` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `transform` | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ✅ | N/A | N/A | N/A | N/A |
| `transpose` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `verify` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |

### ImageChops

| Operation | 1 | L | RGB |
|-----------|-------|-------|-------|
| `add` | N/A | ✅ | ✅ |
| `add_modulo` | N/A | ✅ | ✅ |
| `blend` | N/A | ✅ | ✅ |
| `composite` | N/A | ✅ | ✅ |
| `constant` | N/A | ✅ | ✅ |
| `darker` | N/A | ✅ | ✅ |
| `difference` | N/A | ✅ | ✅ |
| `duplicate` | N/A | ✅ | ✅ |
| `hard_light` | N/A | ✅ | ✅ |
| `invert` | N/A | ✅ | ✅ |
| `lighter` | N/A | ✅ | ✅ |
| `logical_and` | ✅ | N/A | N/A |
| `logical_or` | ✅ | N/A | N/A |
| `logical_xor` | ✅ | N/A | N/A |
| `multiply` | N/A | ✅ | ✅ |
| `offset` | N/A | ✅ | ✅ |
| `overlay` | N/A | ✅ | ✅ |
| `screen` | N/A | ✅ | ✅ |
| `soft_light` | N/A | ✅ | ✅ |
| `subtract` | N/A | ✅ | ✅ |
| `subtract_modulo` | N/A | ✅ | ✅ |

### ImageColor

| Operation | L | RGB |
|-----------|-------|-------|
| `getcolor` | ✅ | ✅ |
| `getrgb` | N/A | ✅ |

### ImageDraw

| Operation | 1 | L | LA | P | RGB | RGBA | CMYK |
|-----------|-------|-------|-------|-------|-------|-------|-------|
| `arc` | ✅ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| `bitmap` | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ |
| `chord` | ⚠️ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| `circle` | ⚠️ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| `ellipse` | ✅ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| `line` | ✅ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| `multiline_text` | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ |
| `pieslice` | ⚠️ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| `point` | ✅ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| `polygon` | ✅ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| `rectangle` | ⚠️ | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| `regular_polygon` | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ |
| `rounded_rectangle` | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ |
| `text` | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ |

### ImageEnhance

| Operation | L | LA | RGB | RGBA |
|-----------|-------|-------|-------|-------|
| `Brightness` | ✅ | ⬜ | ✅ | ⬜ |
| `Color` | ✅ | ⬜ | ✅ | ⬜ |
| `Contrast` | ✅ | ⬜ | ✅ | ⬜ |
| `Sharpness` | ✅ | ⬜ | ✅ | ⬜ |

### ImageFilter

| Operation | L | LA | RGB | RGBA |
|-----------|-------|-------|-------|-------|
| `BLUR` | ✅ | ✅ | ✅ | ✅ |
| `BoxBlur` | ✅ | ✅ | ✅ | ✅ |
| `CONTOUR` | ✅ | ✅ | ✅ | ✅ |
| `DETAIL` | ✅ | ✅ | ✅ | ✅ |
| `EDGE_ENHANCE` | ✅ | ✅ | ✅ | ✅ |
| `EDGE_ENHANCE_MORE` | ✅ | ✅ | ✅ | ✅ |
| `EMBOSS` | ✅ | ⚠️ | ✅ | ⚠️ |
| `FIND_EDGES` | ✅ | ⚠️ | ✅ | ⚠️ |
| `GaussianBlur` | ✅ | ✅ | ✅ | ✅ |
| `Kernel` | ✅ | N/A | ✅ | N/A |
| `MaxFilter` | ✅ | N/A | ✅ | N/A |
| `MedianFilter` | ✅ | N/A | ✅ | N/A |
| `MinFilter` | ✅ | N/A | ✅ | N/A |
| `ModeFilter` | ✅ | N/A | ✅ | N/A |
| `RankFilter` | ✅ | N/A | ✅ | N/A |
| `SHARPEN` | ✅ | ✅ | ✅ | ✅ |
| `SMOOTH` | ✅ | ✅ | ✅ | ✅ |
| `SMOOTH_MORE` | ✅ | ✅ | ✅ | ✅ |
| `UnsharpMask` | ✅ | ⚠️ | ✅ | ⚠️ |

### ImageFont

| Operation | L | RGB | RGBA |
|-----------|-------|-------|-------|
| `load` | ✅ | ✅ | ⬜ |
| `load_default` | ✅ | ✅ | ⬜ |
| `load_default_imagefont` | ✅ | ✅ | ⬜ |
| `load_path` | ✅ | ✅ | ⬜ |
| `truetype` | ✅ | ✅ | ⬜ |

### ImageModule

| Operation | 1 | L | LA | P | RGB | RGBA | CMYK | HSV | I | F |
|-----------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|
| `alpha_composite` | N/A | ⬜ | N/A | N/A | ⬜ | ⬜ | N/A | N/A | N/A | N/A |
| `blend` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `composite` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `effect_noise` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `eval` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `frombytes` | N/A | ⬜ | N/A | N/A | ⬜ | ⬜ | N/A | N/A | N/A | N/A |
| `merge` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `new` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| `open` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |

### ImageOps

| Operation | L | LA | P | RGB | RGBA |
|-----------|-------|-------|-------|-------|-------|
| `autocontrast` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `colorize` | ✅ | N/A | N/A | ⬜ | N/A |
| `contain` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `cover` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `crop` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `equalize` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `expand` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `fit` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `flip` | ✅ | ⬜ | N/A | ✅ | ✅ |
| `grayscale` | ✅ | ✅ | ⚠️ | ✅ | ✅ |
| `invert` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `mirror` | ✅ | ⬜ | N/A | ✅ | ✅ |
| `pad` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `posterize` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `scale` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `solarize` | ✅ | ⬜ | N/A | ✅ | ⬜ |

### ImagePalette

| Operation | P |
|-----------|-------|
| `copy` | ⬜ |
| `getcolor` | ⬜ |
| `getdata` | ⬜ |
| `save` | ⬜ |
| `tobytes` | ⬜ |

### ImageSequence

| Operation | L | RGB |
|-----------|-------|-------|
| `Iterator` | ✅ | ✅ |

### ImageStat

| Operation | L | RGB | RGBA |
|-----------|-------|-------|-------|
| `Stat` | ✅ | ✅ | ⬜ |


## Reverse Verification

Every test in the trust report validates PIL-RSPIL parity:
- Tests create identical inputs for both `PIL.Image` and `pillow_rs.Image`
- Apply the same operation with identical parameters
- Assert pixel-exact binary equality or value equality
- No tests verify only signature existence or stub behavior

**Verification method:** `assert_images_equal(rs_img, pil_img)` for image output,
`assert_values_equal(rs_val, pil_val)` for non-image values. Fixture tests use
SHA-256 hash comparison with tolerance for lossy operations.

## How Coverage Mapping Works

Coverage mapping derives from two auto-discovered sources — no separate mapping file:

1. **Fixture JSONs** (365 files in `tests/fixtures/`): Each fixture declares
   `operation.module` + `operation.target` in its JSON metadata.
   The test runner (`test_fixture_parity.py`) auto-generates
   `@pytest.mark.covers` markers from this metadata at collection time.

2. **Static decorators**: Tests in `tests/test_*.py` files with
   `@pytest.mark.covers("Module.function")` decorators are parsed directly.

*Report generated by `scripts/coverage/compute_coverage.py --md`*
