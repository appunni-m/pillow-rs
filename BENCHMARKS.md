# pillow-rs Benchmarks

> Auto-generated: 2026-06-11 13:09:47 | commit b8a0f62 | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Average CPU speedup vs Pillow | 1180.89× |

## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)

> Chaining 20 image operations end-to-end. Measures scheduling overhead, coherence, and clone avoidance.

| Variant | Time (ms) |
|---------|-----------|
| MT | 228.01ms |
| ST | 300.00ms |
| **MT Speedup** | **1.32×** |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| resize | 0.74× | — | 1.79× | — | 1.29× | 1.75× |
| crop | 0.95× | — | 3.35× | — | 3.10× | 3.11× |
| crop | 0.95× | — | 3.35× | — | 3.10× | 3.11× |
| rotate | 4.99× | — | — | — | — | — |
| transpose | — | — | 2.39× | — | 2.29× | 2.35× |
| thumbnail | 45960.16× | — | 1.61× | — | 1.54× | 1.59× |
| new | 1.50× | — | 1.04× | — | 0.89× | — |
| new | 1.50× | — | 1.04× | — | 0.89× | — |
| paste | 0.72× | — | 100.53× | — | — | — |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| resize | 0.74× | — | 1.79× | — | 1.29× | 1.75× |
| crop | 0.95× | — | 3.35× | — | 3.10× | 3.11× |
| rotate | 4.99× | — | — | — | — | — |
| transpose | — | — | 2.39× | — | 2.29× | 2.35× |
| thumbnail | 45960.16× | — | 1.61× | — | 1.54× | 1.59× |
| new | 1.50× | — | 1.04× | — | 0.89× | — |
| paste | 0.72× | — | 100.53× | — | — | — |
| alpha_composite | — | — | — | — | — | — |
| apply_transparency | — | — | — | — | — | — |
| close | — | — | — | — | — | — |
| convert | 0.64× | — | 1.70× | — | 1.77× | 1.62× |
| copy | — | — | — | — | — | — |
| draft | — | — | — | — | — | — |
| effect_spread | — | — | — | — | — | — |
| entropy | — | — | — | — | — | — |
| filter | 1.06× | — | 2.45× | — | — | — |
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
| point | 0.64× | — | — | — | — | — |
| putalpha | 28.57× | — | — | — | — | — |
| putdata | — | — | — | — | — | — |
| putpalette | — | — | — | — | — | — |
| putpixel | 33.79× | — | 3.37× | — | 3.51× | — |
| quantize | 0.21× | — | — | — | — | — |
| reduce | 0.44× | — | 2.08× | — | 1.94× | 2.13× |
| remap_palette | — | — | — | — | — | — |
| save | — | — | — | — | — | — |
| seek | — | — | — | — | — | — |
| show | — | — | — | — | — | — |
| split | 0.37× | — | 8.49× | — | 8.06× | — |
| tell | — | — | — | — | — | — |
| tobitmap | — | — | — | — | — | — |
| tobytes | — | — | — | — | — | — |
| transform | — | — | — | — | — | — |
| verify | — | — | — | — | — | — |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| add | 0.54× | — | — | — | — | — |
| add_modulo | — | — | — | — | — | — |
| blend | — | — | — | — | — | — |
| composite | — | — | — | — | — | — |
| constant | — | — | — | — | — | — |
| darker | 0.33× | — | — | — | — | — |
| difference | 0.49× | — | — | — | — | — |
| duplicate | — | — | — | — | — | — |
| hard_light | — | — | — | — | — | — |
| invert | 0.09× | — | 1.71× | — | — | — |
| lighter | 0.33× | — | — | — | — | — |
| logical_and | — | — | — | — | — | — |
| logical_or | — | — | — | — | — | — |
| logical_xor | — | — | — | — | — | — |
| multiply | 0.36× | — | — | — | — | — |
| offset | — | — | — | — | — | — |
| overlay | — | — | — | — | — | — |
| screen | 0.38× | — | — | — | — | — |
| soft_light | — | — | — | — | — | — |
| subtract | 0.56× | — | — | — | — | — |
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
| point | 0.64× | — | — | — | — | — |
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
| Brightness | 1.16× | — | — | — | — | — |
| Color | 0.98× | — | — | — | — | — |
| Contrast | 0.94× | — | — | — | — | — |
| Sharpness | 1.49× | — | — | — | — | — |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| BLUR | — | — | — | — | — | — |
| BoxBlur | 1.40× | — | — | — | — | — |
| CONTOUR | — | — | — | — | — | — |
| Color3DLUT | — | — | — | — | — | — |
| DETAIL | — | — | — | — | — | — |
| EDGE_ENHANCE | — | — | — | — | — | — |
| EDGE_ENHANCE_MORE | — | — | — | — | — | — |
| EMBOSS | — | — | — | — | — | — |
| FIND_EDGES | — | — | — | — | — | — |
| GaussianBlur | 1.87× | — | — | — | — | — |
| Kernel | — | — | — | — | — | — |
| MaxFilter | 0.61× | — | — | — | — | — |
| MedianFilter | 0.79× | — | — | — | — | — |
| MinFilter | 0.61× | — | — | — | — | — |
| ModeFilter | 2.50× | — | — | — | — | — |
| RankFilter | — | — | — | — | — | — |
| SHARPEN | — | — | — | — | — | — |
| SMOOTH | — | — | — | — | — | — |
| SMOOTH_MORE | — | — | — | — | — | — |
| UnsharpMask | 1.34× | — | — | — | — | — |

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
| new | 1.50× | — | 1.04× | — | 0.89× | — |
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
| crop | 0.95× | — | 3.35× | — | 3.10× | 3.11× |
| autocontrast | 0.05× | — | — | — | — | — |
| colorize | — | — | — | — | — | — |
| contain | — | — | — | — | — | — |
| cover | — | — | — | — | — | — |
| deform | — | — | — | — | — | — |
| equalize | 0.06× | — | — | — | — | — |
| exif_transpose | — | — | — | — | — | — |
| expand | — | — | — | — | — | — |
| fit | — | — | — | — | — | — |
| flip | — | — | — | — | — | — |
| grayscale | — | — | — | — | — | — |
| invert | 0.09× | — | 1.71× | — | — | — |
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
