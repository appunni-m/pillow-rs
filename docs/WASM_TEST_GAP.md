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

## Implementation Plan

1. Add WASM exports for missing functions (getexif, getxmp, show, draft, etc.)
2. Add WASM tests matching Python test names exactly  
3. Regenerate coverage to show 202/202
