# pillow-rs Benchmarks

> Auto-generated: 2026-06-10 21:23:03 | commit 9088f0f | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Average CPU speedup vs Pillow | 0.00× |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| resize | — | — | — | — | — | — |
| crop | — | — | — | — | — | — |
| crop | — | — | — | — | — | — |
| rotate | — | — | — | — | — | — |
| transpose | — | — | — | — | — | — |
| thumbnail | — | — | — | — | — | — |
| new | — | — | — | — | — | — |
| new | — | — | — | — | — | — |
| paste | — | — | — | — | — | — |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| resize | — | — | — | — | — | — |
| crop | — | — | — | — | — | — |
| rotate | — | — | — | — | — | — |
| transpose | — | — | — | — | — | — |
| thumbnail | — | — | — | — | — | — |
| new | — | — | — | — | — | — |
| paste | — | — | — | — | — | — |
| alpha_composite | — | — | — | — | — | — |
| apply_transparency | — | — | — | — | — | — |
| close | — | — | — | — | — | — |
| convert | — | — | — | — | — | — |
| copy | — | — | — | — | — | — |
| draft | — | — | — | — | — | — |
| effect_spread | — | — | — | — | — | — |
| entropy | — | — | — | — | — | — |
| filter | — | — | — | — | — | — |
| frombytes | — | — | — | — | — | — |
| get_child_images | — | — | — | — | — | — |
| get_flattened_data | — | — | — | — | — | — |
| getbands | — | — | — | — | — | — |
| getbbox | — | — | — | — | — | — |
| getchannel | — | — | — | — | — | — |
| getcolors | — | — | — | — | — | — |
| getdata | — | — | — | — | — | — |
| getexif | — | — | — | — | — | — |
| getextrema | — | — | — | — | — | — |
| getim | — | — | — | — | — | — |
| getpalette | — | — | — | — | — | — |
| getpixel | — | — | — | — | — | — |
| getprojection | — | — | — | — | — | — |
| getxmp | — | — | — | — | — | — |
| histogram | — | — | — | — | — | — |
| load | — | — | — | — | — | — |
| open | — | — | — | — | — | — |
| point | — | — | — | — | — | — |
| putalpha | — | — | — | — | — | — |
| putdata | — | — | — | — | — | — |
| putpalette | — | — | — | — | — | — |
| putpixel | — | — | — | — | — | — |
| quantize | — | — | — | — | — | — |
| reduce | — | — | — | — | — | — |
| remap_palette | — | — | — | — | — | — |
| save | — | — | — | — | — | — |
| seek | — | — | — | — | — | — |
| show | — | — | — | — | — | — |
| split | — | — | — | — | — | — |
| tell | — | — | — | — | — | — |
| tobitmap | — | — | — | — | — | — |
| tobytes | — | — | — | — | — | — |
| transform | — | — | — | — | — | — |
| verify | — | — | — | — | — | — |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| add | — | — | — | — | — | — |
| add_modulo | — | — | — | — | — | — |
| blend | — | — | — | — | — | — |
| composite | — | — | — | — | — | — |
| constant | — | — | — | — | — | — |
| darker | — | — | — | — | — | — |
| difference | — | — | — | — | — | — |
| duplicate | — | — | — | — | — | — |
| hard_light | — | — | — | — | — | — |
| invert | — | — | — | — | — | — |
| lighter | — | — | — | — | — | — |
| logical_and | — | — | — | — | — | — |
| logical_or | — | — | — | — | — | — |
| logical_xor | — | — | — | — | — | — |
| multiply | — | — | — | — | — | — |
| offset | — | — | — | — | — | — |
| overlay | — | — | — | — | — | — |
| screen | — | — | — | — | — | — |
| soft_light | — | — | — | — | — | — |
| subtract | — | — | — | — | — | — |
| subtract_modulo | — | — | — | — | — | — |

### ImageColor

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| getcolor | — | — | — | — | — | — |
| getrgb | — | — | — | — | — | — |

### ImageDraw

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| arc | — | — | — | — | — | — |
| bitmap | — | — | — | — | — | — |
| chord | — | — | — | — | — | — |
| circle | — | — | — | — | — | — |
| ellipse | — | — | — | — | — | — |
| getfont | — | — | — | — | — | — |
| line | — | — | — | — | — | — |
| multiline_text | — | — | — | — | — | — |
| multiline_textbbox | — | — | — | — | — | — |
| pieslice | — | — | — | — | — | — |
| point | — | — | — | — | — | — |
| polygon | — | — | — | — | — | — |
| rectangle | — | — | — | — | — | — |
| regular_polygon | — | — | — | — | — | — |
| rounded_rectangle | — | — | — | — | — | — |
| text | — | — | — | — | — | — |
| textbbox | — | — | — | — | — | — |
| textlength | — | — | — | — | — | — |

### ImageEnhance

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Brightness | — | — | — | — | — | — |
| Color | — | — | — | — | — | — |
| Contrast | — | — | — | — | — | — |
| Sharpness | — | — | — | — | — | — |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| BLUR | — | — | — | — | — | — |
| BoxBlur | — | — | — | — | — | — |
| CONTOUR | — | — | — | — | — | — |
| Color3DLUT | — | — | — | — | — | — |
| DETAIL | — | — | — | — | — | — |
| EDGE_ENHANCE | — | — | — | — | — | — |
| EDGE_ENHANCE_MORE | — | — | — | — | — | — |
| EMBOSS | — | — | — | — | — | — |
| FIND_EDGES | — | — | — | — | — | — |
| GaussianBlur | — | — | — | — | — | — |
| Kernel | — | — | — | — | — | — |
| MaxFilter | — | — | — | — | — | — |
| MedianFilter | — | — | — | — | — | — |
| MinFilter | — | — | — | — | — | — |
| ModeFilter | — | — | — | — | — | — |
| RankFilter | — | — | — | — | — | — |
| SHARPEN | — | — | — | — | — | — |
| SMOOTH | — | — | — | — | — | — |
| SMOOTH_MORE | — | — | — | — | — | — |
| UnsharpMask | — | — | — | — | — | — |

### ImageFont

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| FreeTypeFont | — | — | — | — | — | — |
| ImageFont | — | — | — | — | — | — |
| getbbox | — | — | — | — | — | — |
| getbbox | — | — | — | — | — | — |
| getlength | — | — | — | — | — | — |
| getlength | — | — | — | — | — | — |
| getmask | — | — | — | — | — | — |
| getmask | — | — | — | — | — | — |
| getmetrics | — | — | — | — | — | — |
| getname | — | — | — | — | — | — |
| load | — | — | — | — | — | — |
| load_default | — | — | — | — | — | — |
| load_default_imagefont | — | — | — | — | — | — |
| load_path | — | — | — | — | — | — |
| truetype | — | — | — | — | — | — |

### ImageModule

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| new | — | — | — | — | — | — |
| alpha_composite | — | — | — | — | — | — |
| blend | — | — | — | — | — | — |
| composite | — | — | — | — | — | — |
| effect_noise | — | — | — | — | — | — |
| eval | — | — | — | — | — | — |
| fromarray | — | — | — | — | — | — |
| frombytes | — | — | — | — | — | — |
| merge | — | — | — | — | — | — |
| open | — | — | — | — | — | — |

### ImageOps

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| crop | — | — | — | — | — | — |
| autocontrast | — | — | — | — | — | — |
| colorize | — | — | — | — | — | — |
| contain | — | — | — | — | — | — |
| cover | — | — | — | — | — | — |
| deform | — | — | — | — | — | — |
| equalize | — | — | — | — | — | — |
| exif_transpose | — | — | — | — | — | — |
| expand | — | — | — | — | — | — |
| fit | — | — | — | — | — | — |
| flip | — | — | — | — | — | — |
| grayscale | — | — | — | — | — | — |
| invert | — | — | — | — | — | — |
| mirror | — | — | — | — | — | — |
| pad | — | — | — | — | — | — |
| posterize | — | — | — | — | — | — |
| scale | — | — | — | — | — | — |
| solarize | — | — | — | — | — | — |

### ImagePalette

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| copy | — | — | — | — | — | — |
| getcolor | — | — | — | — | — | — |
| getdata | — | — | — | — | — | — |
| save | — | — | — | — | — | — |
| tobytes | — | — | — | — | — | — |

### ImageSequence

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Iterator | — | — | — | — | — | — |

### ImageStat

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Stat | — | — | — | — | — | — |
