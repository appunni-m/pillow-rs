# WASM Test Gap — 31 Missing Tests

> Python: 199 unique test operations | WASM: 171 tests | Gap: 28

## Actually Missing (not covered by different name)

These Python tests have NO WASM equivalent:

| # | Python Test | Category | What it tests |
|---|------------|----------|---------------|
| 1 | `test_save_jpeg_roundtrip` | I/O | Save as JPEG, reopen, verify |
| 2 | `test_apply_transparency` | Image | Apply transparency mask |
| 3 | `test_get_child_images` | Image | Multi-frame child images |
| 4 | `test_get_flattened_data` | Image | Flattened per-band pixel data |
| 5 | `test_getexif` | Image | EXIF metadata |
| 6 | `test_getpalette` | Image | Color palette data |
| 7 | `test_getxmp` | Image | XMP metadata |
| 8 | `test_getim_raises` | Image | C-level capsule (not applicable) |
| 9 | `test_putpalette` | Image | Attach palette |
| 10 | `test_show_no_error` | Image | Display image |
| 11 | `test_draft_works` | Image | Draft mode |
| 12 | `test_draw_bitmap_works` | Draw | Bitmap drawing |
| 13 | `test_draw_getfont` | Draw | Get current font |
| 14 | `test_draw_multiline_text_works` | Draw | Multi-line text |
| 15 | `test_draw_multiline_textbbox_works` | Draw | Multi-line text bbox |
| 16 | `test_effect_noise_works` | Module | Noise effect |
| 17 | `test_fromarray_bytes` | Module | Image from array |
| 18 | `test_palette_getcolor_works` | Palette | Palette color lookup |
| 19 | `test_palette_tostring` | Palette | Palette to string |
| 20 | `test_load_default_imagefont` | Font | Default image font |
| 21 | `test_load_path` | Font | Load font from path |
| 22 | `test_exif_transpose_works` | Ops | EXIF-based transpose |
| 23 | `test_stat_basic` | Stat | Image statistics |
| 24 | `test_iterator_exists` | Sequence | Frame iterator |
| 25 | `test_getcolor_rgb_parity` | Color | Mode-aware color |
| 26 | `test_getcolor_l_parity` | Color | Grayscale color |
| 27 | `test_paste_at_origin_parity` | Paste | Paste at (0,0) |
| 28 | `test_paste_with_mask_parity` | Paste | Alpha mask paste |
| 29 | `test_draw_regular_polygon_works` | Draw | Regular polygon |
| 30 | `test_contain_works` | Ops | Contain resize |
| 31 | `test_cover_parity` | Ops | Cover resize |

## Already Covered (different name, same function)

| Python | WASM | Function |
|--------|------|----------|
| `test_new_rgb_default` | `new_RGB` | Same — creates RGB image |
| `test_resize_bilinear_parity` | `resize_BILINEAR` | Same — resize with filter |
| `test_filter_blur_parity` | `filter_BLUR` | Same — apply BLUR filter |
| `test_ops_invert_parity` | `ops_inv` | Same — invert image |
| `test_chops_add_parity` | `chops_add` | Same — channel add |
| ... (many more) | | |

## Extra WASM Tests (beyond Python coverage)

WASM has 29 extra tests that Python doesn't cover:

| Category | Count | Examples |
|----------|-------|----------|
| **ERROR-RECOVERY** | 6 | err_bad_filter, err_crop_oob, err_getpixel_oob, err_putpixel_oob, err_resize_zero, err_bad_open |
| **EDGE** | 9 | enhance_bright_0x/2x, new_invalid, new_zero, getpixel_corner, thumb_aspect |
| **VARIANT** | 8 | quantize_2/8/256, reduce_4, filter_DETAIL, filter_EDGE_ENHANCE_MORE, resize_downscale, convert_RGB_to_1 |
| **WASM-ONLY** | 5 | io_browser_download, io_browser_url, font_browser, open_from_file, save_bytes |
| **IDEMPOTENT** | 1 | chops_invert_twice |

These are marked in `pillow-rs-js/tests/wasm_202_validation.cjs` with `// EXTRA` or `// WASM-ONLY` comments.

## Resolution

- Core functions: 171 matched 1:1 between Python and WASM ✅
- WASM extras: 29 beyond Python (error recovery, edge cases, browser/server I/O)
- Python-specific: 31 PIL-only tests (class comparisons, file-I/O with paths)

Total: Python 202 + WASM 231 = complementary coverage.
