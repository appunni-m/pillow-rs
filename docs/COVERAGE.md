# pillow-rs Coverage Report

> Auto-generated: 2026-06-14 21:10:30 | Pillow parity tested

## Trust Summary

| Metric | Value |
|--------|-------|
| **Total tests** | 564 |
| **Passing** | 564 |
| **Failed** | 0 |
| **Skipped** | 0 |
| **Implemented functions** | 145 |
| **Trusted (PIL parity tested)** | 145 |
| **Untested** | 0 |
| **Stubs** | 5 |
| **Trust score** | **145/145 (100%)** |

## Performance Benchmarks

*Multiple = PIL time / pillow-rs time. >1.0 = pillow-rs is faster.*

| Operation | Speedup | Faster? |
|-----------|---------|---------|
| resize_2k_to_1k | 33.07× | ✅ |
| crop_2k | 0.65× | ❌ |
| convert_2k_RGB_to_L | 2.00× | ✅ |
| transpose_2k_FLIP | 3.51× | ✅ |
| filter_2k_BLUR | 73.62× | ✅ |
| paste_2k | 1.03× | ✅ |
| invert_2k | 2.96× | ✅ |

**Average speedup: 16.69×**

## Module Status

| Module | Implemented | Trusted | Untested | Trust % |
|--------|------------|---------|----------|---------|
| Image | 48 | 48 | 0 | 100% |
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
| `convert` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `copy` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `crop` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `draft` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `effect_spread` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `entropy` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `filter` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | N/A | N/A | N/A |
| `frombytes` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `get_child_images` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `get_flattened_data` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getbands` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getbbox` | ⬜ | ✅ | ✅ | ⬜ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getchannel` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | ⬜ | N/A | N/A | N/A |
| `getcolors` | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getdata` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getexif` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getextrema` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `getim` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getpalette` | N/A | ✅ | N/A | ⬜ | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getpixel` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getprojection` | ⬜ | ✅ | ✅ | ⬜ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `getxmp` | N/A | ✅ | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `histogram` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `load` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `new` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `open` | N/A | ✅ | ⬜ | N/A | ✅ | ⬜ | N/A | N/A | N/A | N/A |
| `paste` | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ | N/A | N/A | N/A |
| `point` | ⬜ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | N/A | N/A | N/A | N/A |
| `putalpha` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `putdata` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `putpalette` | N/A | ⬜ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A | N/A |
| `putpixel` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `quantize` | N/A | ✅ | N/A | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `reduce` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `remap_palette` | N/A | ✅ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A | N/A |
| `resize` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A |
| `rotate` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `seek` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `split` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `tell` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `thumbnail` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `tobitmap` | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `tobytes` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `transform` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `transpose` | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `verify` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A |

### ImageChops

| Operation | 1 | L | LA | P | RGB | RGBA |
|-----------|-------|-------|-------|-------|-------|-------|
| `add` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `add_modulo` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `blend` | N/A | ✅ | N/A | N/A | ✅ | N/A |
| `composite` | N/A | ✅ | N/A | ✅ | ✅ | N/A |
| `constant` | N/A | ✅ | N/A | ✅ | ✅ | N/A |
| `darker` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `difference` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `duplicate` | N/A | ✅ | N/A | ✅ | ✅ | N/A |
| `hard_light` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `invert` | N/A | ✅ | N/A | ✅ | ✅ | N/A |
| `lighter` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `logical_and` | ✅ | N/A | N/A | N/A | N/A | N/A |
| `logical_or` | ✅ | N/A | N/A | N/A | N/A | N/A |
| `logical_xor` | ✅ | N/A | N/A | N/A | N/A | N/A |
| `multiply` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `offset` | N/A | ✅ | N/A | N/A | ✅ | ✅ |
| `overlay` | N/A | ✅ | N/A | N/A | ✅ | N/A |
| `screen` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `soft_light` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `subtract` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |
| `subtract_modulo` | N/A | ✅ | ✅ | N/A | ✅ | ✅ |

### ImageColor

| Operation | L | RGB |
|-----------|-------|-------|
| `getcolor` | ✅ | ✅ |
| `getrgb` | ✅ | ✅ |

### ImageDraw

| Operation | 1 | L | LA | P | RGB | RGBA | CMYK |
|-----------|-------|-------|-------|-------|-------|-------|-------|
| `arc` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `bitmap` | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ |
| `chord` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `circle` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `ellipse` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `line` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `multiline_text` | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ |
| `pieslice` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `point` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `polygon` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `rectangle` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
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

| Operation | 1 | L | LA | RGB | RGBA | CMYK | HSV | I |
|-----------|-------|-------|-------|-------|-------|-------|-------|-------|
| `BLUR` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A |
| `BoxBlur` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `CONTOUR` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `DETAIL` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `EDGE_ENHANCE` | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `EDGE_ENHANCE_MORE` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `EMBOSS` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `FIND_EDGES` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `GaussianBlur` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `Kernel` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `MaxFilter` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `MedianFilter` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `MinFilter` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `ModeFilter` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `RankFilter` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `SHARPEN` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `SMOOTH` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `SMOOTH_MORE` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `UnsharpMask` | N/A | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |

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
| `blend` | N/A | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `composite` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `effect_noise` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `eval` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A |
| `frombytes` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | N/A | N/A | N/A |
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
| `grayscale` | ✅ | ✅ | ⬜ | ✅ | ✅ |
| `invert` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `mirror` | ✅ | ⬜ | N/A | ✅ | ✅ |
| `pad` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `posterize` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `scale` | ✅ | ⬜ | N/A | ✅ | ⬜ |
| `solarize` | ✅ | ⬜ | N/A | ✅ | ⬜ |

### ImagePalette

| Operation | L | P | RGB |
|-----------|-------|-------|-------|
| `copy` | ✅ | ⬜ | ✅ |
| `getcolor` | ✅ | ⬜ | ✅ |
| `getdata` | ✅ | ⬜ | ✅ |
| `save` | ✅ | ⬜ | ✅ |
| `tobytes` | ✅ | ⬜ | ✅ |

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
