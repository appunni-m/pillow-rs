# pillow-rs Benchmarks

> Auto-generated: 2026-06-11 09:51:27 | commit 34e2717 | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Average CPU speedup vs Pillow | 0.00× |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| resize | — | — | 1.79× | — | 1.29× | 1.75× |
| crop | — | — | 3.35× | — | 3.10× | 3.11× |
| crop | — | — | 3.35× | — | 3.10× | 3.11× |
| rotate | — | — | — | — | — | — |
| transpose | — | — | 2.39× | — | 2.29× | 2.35× |
| thumbnail | — | — | 1.61× | — | 1.54× | 1.59× |
| new | — | — | 1.04× | — | 0.89× | — |
| new | — | — | 1.04× | — | 0.89× | — |
| paste | — | — | 100.53× | — | — | — |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| resize | — | — | 1.79× | — | 1.29× | 1.75× |
| crop | — | — | 3.35× | — | 3.10× | 3.11× |
| rotate | — | — | — | — | — | — |
| transpose | — | — | 2.39× | — | 2.29× | 2.35× |
| thumbnail | — | — | 1.61× | — | 1.54× | 1.59× |
| new | — | — | 1.04× | — | 0.89× | — |
| paste | — | — | 100.53× | — | — | — |
| alpha_composite | — | — | — | — | — | — |
| apply_transparency | — | — | — | — | — | — |
| close | — | — | — | — | — | — |
| convert | — | — | 1.70× | — | 1.77× | 1.62× |
| copy | — | — | — | — | — | — |
| draft | — | — | — | — | — | — |
| effect_spread | — | — | — | — | — | — |
| entropy | — | — | — | — | — | — |
| filter | — | — | 2.45× | — | — | — |
| frombytes | — | — | — | — | — | — |
| get_child_images | — | — | — | — | — | — |
| get_flattened_data | — | — | — | — | — | — |
| getbands | — | — | 49.57× | — | 53.56× | — |
| getbbox | — | — | 10.76× | — | 11.81× | — |
| getchannel | — | — | — | — | — | — |
| getcolors | — | — | — | — | — | — |
| getdata | — | — | — | — | — | — |
| getexif | — | — | — | — | — | — |
| getextrema | — | — | 7.40× | — | 7.71× | — |
| getim | — | — | — | — | — | — |
| getpalette | — | — | — | — | — | — |
| getpixel | — | — | 11731.14× | — | 17596.71× | — |
| getprojection | — | — | — | — | — | — |
| getxmp | — | — | — | — | — | — |
| histogram | — | — | 9.24× | — | 6.09× | — |
| load | — | — | — | — | — | — |
| open | — | — | — | — | — | — |
| point | — | — | — | — | — | — |
| putalpha | — | — | — | — | — | — |
| putdata | — | — | — | — | — | — |
| putpalette | — | — | — | — | — | — |
| putpixel | — | — | 3.37× | — | 3.51× | — |
| quantize | — | — | — | — | — | — |
| reduce | — | — | 2.08× | — | 1.94× | 2.13× |
| remap_palette | — | — | — | — | — | — |
| save | — | — | — | — | — | — |
| seek | — | — | — | — | — | — |
| show | — | — | — | — | — | — |
| split | — | — | 8.49× | — | 8.06× | — |
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
| getbbox | — | — | 10.76× | — | 11.81× | — |
| getbbox | — | — | 10.76× | — | 11.81× | — |
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
| new | — | — | 1.04× | — | 0.89× | — |
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
| crop | — | — | 3.35× | — | 3.10× | 3.11× |
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
