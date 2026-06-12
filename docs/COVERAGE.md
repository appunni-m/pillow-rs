# pillow-rs Coverage Report

> Auto-generated: 2026-06-12 15:43:25 | Pillow 12.2.0

## Trust Summary

| Metric | Value |
|--------|-------|
| **Total tests** | 296 |
| **Passing** | 294 |
| **Failed** | 2 |
| **Implemented functions** | 135 |
| **Trusted (PIL parity tested)** | 115 |
| **Untested** | 20 |
| **Stubs** | 5 |
| **Trust score** | **115/135 (85%)** |

## Performance Benchmarks

*Multiple = PIL time / pillow-rs time. >1.0 = pillow-rs is faster.*

| Operation | Speedup | Faster? |
|-----------|---------|---------|
| resize_2k_to_1k | 31.31× | ✅ |
| crop_2k | 0.67× | ❌ |
| convert_2k_RGB_to_L | 2.09× | ✅ |
| transpose_2k_FLIP | 3.29× | ✅ |
| filter_2k_BLUR | 72.27× | ✅ |
| paste_2k | 1.11× | ✅ |
| invert_2k | 3.15× | ✅ |

**Average speedup: 16.27×**

## Module Status

| Module | Implemented | Trusted | Untested | Trust % |
|--------|------------|---------|----------|---------|
| Image | 51 | 44 | 7 | 86% |
| ImageChops | 21 | 13 | 8 | 62% |
| ImageColor | 2 | 2 | 0 | 100% |
| ImageDraw | 18 | 18 | 0 | 100% |
| ImageFont | 10 | 5 | 5 | 50% |
| ImageModule | 10 | 10 | 0 | 100% |
| ImageOps | 18 | 18 | 0 | 100% |
| ImagePalette | 5 | 5 | 0 | 100% |

## ⚠️ Untested Functions

- `Image.effect_spread`
- `Image.entropy`
- `Image.getcolors`
- `Image.getprojection`
- `Image.point`
- `Image.quantize`
- `Image.rotate`
- `ImageChops.add`
- `ImageChops.darker`
- `ImageChops.difference`
- `ImageChops.invert`
- `ImageChops.lighter`
- `ImageChops.multiply`
- `ImageChops.screen`
- `ImageChops.subtract`
- `ImageFont.getbbox`
- `ImageFont.getlength`
- `ImageFont.getmask`
- `ImageFont.getmetrics`
- `ImageFont.getname`

## ⬜ Remaining Stubs

- `Image.toqimage`
- `Image.toqpixmap`
- `ImageDraw.shape`
- `ImageModule.effect_mandelbrot`
- `ImageModule.frombuffer`

## 🔍 Tests Not in Coverage Map

- `tests/test_error_parity.py::test_convert_from_rgb_to_nonstandard[CMYK]`
- `tests/test_error_parity.py::test_convert_from_rgb_to_nonstandard[F]`
- `tests/test_error_parity.py::test_convert_from_rgb_to_nonstandard[HSV]`
- `tests/test_error_parity.py::test_convert_from_rgb_to_nonstandard[I]`
- `tests/test_error_parity.py::test_convert_from_rgb_to_nonstandard[YCbCr]`
- `tests/test_error_parity.py::test_crop_nonstandard_modes[CMYK]`
- `tests/test_error_parity.py::test_crop_nonstandard_modes[F]`
- `tests/test_error_parity.py::test_crop_nonstandard_modes[HSV]`
- `tests/test_error_parity.py::test_crop_nonstandard_modes[I]`
- `tests/test_error_parity.py::test_crop_nonstandard_modes[YCbCr]`
- `tests/test_error_parity.py::test_filter_nonstandard_modes[1]`
- `tests/test_error_parity.py::test_filter_nonstandard_modes[CMYK]`
- `tests/test_error_parity.py::test_filter_nonstandard_modes[P]`
- `tests/test_error_parity.py::test_new_nonstandard_modes[CMYK]`
- `tests/test_error_parity.py::test_new_nonstandard_modes[F]`
- `tests/test_error_parity.py::test_new_nonstandard_modes[HSV]`
- `tests/test_error_parity.py::test_new_nonstandard_modes[I]`
- `tests/test_error_parity.py::test_new_nonstandard_modes[YCbCr]`
- `tests/test_error_parity.py::test_resize_nonstandard_modes[CMYK]`
- `tests/test_error_parity.py::test_resize_nonstandard_modes[F]`
- ... and 96 more

## Reverse Verification

Every test in the trust report validates PIL-RSPIL parity:
- Tests create identical inputs for both `PIL.Image` and `pillow_rs.Image`
- Apply the same operation with identical parameters
- Assert pixel-exact binary equality or value equality
- No tests verify only signature existence or stub behavior

**Verification method:** `assert_images_equal(rs_img, pil_img)` for image output,
`assert_values_equal(rs_val, pil_val)` for non-image values.

## Mode × Operation Coverage Matrix

* ✅ = tested (parity passes), ⬜ = untested, N/A = PIL doesn't support this mode*

### Image

| Operation | L | LA | RGB | RGBA | 1 | P | CMYK | YCbCr | HSV | I | F |
|-----------|---|---|---|---|---|---|---|---|---|---|---|
| `open` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `new` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ |
| `resize` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `crop` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `rotate` | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | N/A | N/A | N/A | N/A |
| `transpose` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `convert` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `paste` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `filter` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A |
| `copy` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `split` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `getbands` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `thumbnail` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `tobytes` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `alpha_composite` | N/A | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `close` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getbbox` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getchannel` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `getcolors` | ⬜ | N/A | ⬜ | ⬜ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `getdata` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `getextrema` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `getpixel` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `getprojection` | ⬜ | N/A | ⬜ | ⬜ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `histogram` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `load` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `point` | ⬜ | N/A | ⬜ | ⬜ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `putalpha` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `putdata` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `putpixel` | ✅ | ✅ | ✅ | ✅ | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `quantize` | N/A | N/A | ⬜ | ⬜ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `reduce` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `seek` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `tell` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `transform` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `verify` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `effect_spread` | ⬜ | N/A | ⬜ | ⬜ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `entropy` | ⬜ | N/A | ⬜ | ⬜ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `draft` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `remap_palette` | N/A | N/A | N/A | N/A | N/A | ✅ | N/A | N/A | N/A | N/A | N/A |
| `tobitmap` | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageModule

| Operation | L | LA | RGB | RGBA | 1 | P | CMYK | YCbCr | HSV | I | F |
|-----------|---|---|---|---|---|---|---|---|---|---|---|
| `merge` | ✅ | ✅ | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `blend` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `composite` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `eval` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `fromarray` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `frombytes` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `effect_noise` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageDraw

| Operation | L | LA | RGB | RGBA | 1 | P | CMYK | YCbCr | HSV | I | F |
|-----------|---|---|---|---|---|---|---|---|---|---|---|
| `arc` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `line` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `rectangle` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `ellipse` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `polygon` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `text` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `multiline_text` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `circle` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `rounded_rectangle` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `regular_polygon` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `chord` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `pieslice` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `bitmap` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `point` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `textbbox` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `multiline_textbbox` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| `textlength` | ✅ | N/A | ✅ | ✅ | N/A | N/A | N/A | N/A | N/A | N/A | N/A |


*Report generated by `scripts/coverage/generate_coverage_page.py`*
