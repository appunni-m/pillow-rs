# pillow-rs Benchmarks

> Auto-generated: 2026-06-12 08:10:12 | commit `23148b4` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 52 |
| Outliers flagged ⚠️ | 9 |
| Average CPU speedup vs Pillow | 10.93× |
| Native CPU benchmarks run | 61 |
| Missing (no data yet) | 9 |

## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)

> Chaining 20 image operations end-to-end. Measures scheduling, coherence, and clone avoidance.

| Variant | Time (ms) | vs Pillow |
|---------|-----------|-----------|
| ST | 189.98ms |  |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.46× | 0.46× | 1.49× | 0.46× | 0.46× | 0.46× |
| Image.crop | 1.04× | 1.04× | 3.50× | 1.04× | 1.04× | 1.04× |
| Image.rotate | 5.14× | 5.14× | 19.82× | 5.14× | 5.14× | 5.14× |
| Image.transpose | 0.65× | 0.65× | 2.20× | 0.65× | 0.65× | 0.65× |
| Image.thumbnail | — | — | 1.08× | — | — | — |
| Image.new | — | — | 0.23× | — | — | — |
| Image.paste | — | — | — | — | — | — |
| Image.convert | 0.64× | 0.64× | 2.03× | 0.64× | 0.64× | 0.64× |
| Image.filter | 1.18× | 1.18× | 2.42× | 1.18× | 1.18× | 1.18× |
| Image.open | 0.57× | 0.57× | 1.94× | 0.57× | 0.57× | 0.57× |
| Image.save | 3.06× | 3.06× | 2.84× | 3.06× | 3.06× | 3.06× |
| Image.tobytes | 16.13× | 16.13× | 16.13× | 16.13× | 16.13× | 16.13× |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.46× | 0.46× | 1.49× | 0.46× | 0.46× | 0.46× |
| Image.crop | 1.04× | 1.04× | 3.50× | 1.04× | 1.04× | 1.04× |
| Image.rotate | 5.14× | 5.14× | 19.82× | 5.14× | 5.14× | 5.14× |
| Image.transpose | 0.65× | 0.65× | 2.20× | 0.65× | 0.65× | 0.65× |
| Image.thumbnail | — | — | 1.08× | — | — | — |
| Image.new | — | — | 0.23× | — | — | — |
| Image.paste | — | — | — | — | — | — |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | 0.22× | 0.22× | 0.22× | 0.22× | 0.22× | 0.22× |
| Image.close | 90.71× | 90.71× | 90.71× | 90.71× | 90.71× | 90.71× |
| Image.convert | 0.64× | 0.64× | 2.03× | 0.64× | 0.64× | 0.64× |
| Image.copy | 0.77× | 0.77× | 0.77× | 0.77× | 0.77× | 0.77× |
| Image.draft | 158.99× ⚠️ | 158.99× ⚠️ | 158.99× ⚠️ | 158.99× ⚠️ | 158.99× ⚠️ | 158.99× ⚠️ |
| Image.effect_spread | 96.48× | 96.48× | 96.48× | 96.48× | 96.48× | 96.48× |
| Image.entropy | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× |
| Image.filter | 1.18× | 1.18× | 2.42× | 1.18× | 1.18× | 1.18× |
| Image.frombytes | — | — | — | — | — | — |
| Image.get_child_images | 99.30× | 99.30× | 99.30× | 99.30× | 99.30× | 99.30× |
| Image.get_flattened_data | 38.25× | 38.25× | 38.25× | 38.25× | 38.25× | 38.25× |
| Image.getbands | 0.38× | 0.38× | 2.67× | 0.38× | 0.38× | 0.38× |
| Image.getbbox | 0.72× | 0.72× | 2.62× | 0.72× | 0.72× | 0.72× |
| Image.getchannel | 0.66× | 0.66× | 0.66× | 0.66× | 0.66× | 0.66× |
| Image.getcolors | 0.19× | 0.19× | 0.19× | 0.19× | 0.19× | 0.19× |
| Image.getdata | 0.65× | 0.65× | 0.65× | 0.65× | 0.65× | 0.65× |
| Image.getexif | 98.88× | 98.88× | 98.88× | 98.88× | 98.88× | 98.88× |
| Image.getextrema | 0.68× | 0.68× | 2.44× | 0.68× | 0.68× | 0.68× |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | 1610.75× ⚠️ | 1610.75× ⚠️ | 1610.75× ⚠️ | 1610.75× ⚠️ | 1610.75× ⚠️ | 1610.75× ⚠️ |
| Image.getpixel | 0.19× | 0.19× | 2.98× | 0.19× | 0.19× | 0.19× |
| Image.getprojection | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | 0.71× | 0.71× | 2.71× | 0.71× | 0.71× | 0.71× |
| Image.load | 0.71× | 0.71× | 0.71× | 0.71× | 0.71× | 0.71× |
| Image.open | 0.57× | 0.57× | 1.94× | 0.57× | 0.57× | 0.57× |
| Image.point | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× |
| Image.putalpha | 14.72× | 14.72× | 14.72× | 14.72× | 14.72× | 14.72× |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | 483.72× ⚠️ | 483.72× ⚠️ | 483.72× ⚠️ | 483.72× ⚠️ | 483.72× ⚠️ | 483.72× ⚠️ |
| Image.putpixel | 1752.70× ⚠️ | 1752.70× ⚠️ | 92.51× | 1752.70× ⚠️ | 1752.70× ⚠️ | 1752.70× ⚠️ |
| Image.quantize | 2.10× | 2.10× | 2.10× | 2.10× | 2.10× | 2.10× |
| Image.reduce | 0.41× | 0.41× | 1.23× | 0.41× | 0.41× | 0.41× |
| Image.remap_palette | 0.30× | 0.30× | 0.30× | 0.30× | 0.30× | 0.30× |
| Image.save | 3.06× | 3.06× | 2.84× | 3.06× | 3.06× | 3.06× |
| Image.seek | 73.35× | 73.35× | 73.35× | 73.35× | 73.35× | 73.35× |
| Image.show | — | — | — | — | — | — |
| Image.split | 0.30× | 0.30× | 1.09× | 0.30× | 0.30× | 0.30× |
| Image.tell | 101.61× ⚠️ | 101.61× ⚠️ | 101.61× ⚠️ | 101.61× ⚠️ | 101.61× ⚠️ | 101.61× ⚠️ |
| Image.tobitmap | 0.04× | 0.04× | 0.04× | 0.04× | 0.04× | 0.04× |
| Image.tobytes | 16.13× | 16.13× | 16.13× | 16.13× | 16.13× | 16.13× |
| Image.transform | — | — | — | — | — | — |
| Image.verify | 0.13× | 0.13× | 0.13× | 0.13× | 0.13× | 0.13× |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | — | — | — | — | — | — |
| ImageChops.add_modulo | — | — | — | — | — | — |
| ImageChops.blend | — | — | — | — | — | — |
| ImageChops.composite | — | — | — | — | — | — |
| ImageChops.constant | — | — | — | — | — | — |
| ImageChops.darker | — | — | — | — | — | — |
| ImageChops.difference | — | — | — | — | — | — |
| ImageChops.duplicate | — | — | — | — | — | — |
| ImageChops.hard_light | — | — | — | — | — | — |
| ImageChops.invert | — | — | — | — | — | — |
| ImageChops.lighter | — | — | — | — | — | — |
| ImageChops.logical_and | — | — | — | — | — | — |
| ImageChops.logical_or | — | — | — | — | — | — |
| ImageChops.logical_xor | — | — | — | — | — | — |
| ImageChops.multiply | — | — | — | — | — | — |
| ImageChops.offset | — | — | — | — | — | — |
| ImageChops.overlay | — | — | — | — | — | — |
| ImageChops.screen | — | — | — | — | — | — |
| ImageChops.soft_light | — | — | — | — | — | — |
| ImageChops.subtract | — | — | — | — | — | — |
| ImageChops.subtract_modulo | — | — | — | — | — | — |

### ImageColor

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageColor.getcolor | 0.32× | 0.32× | 0.32× | 0.32× | 0.32× | 0.32× |
| ImageColor.getrgb | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× |

### ImageDraw

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageDraw.arc | — | — | — | — | — | — |
| ImageDraw.bitmap | — | — | — | — | — | — |
| ImageDraw.chord | — | — | — | — | — | — |
| ImageDraw.circle | — | — | — | — | — | — |
| ImageDraw.ellipse | — | — | — | — | — | — |
| ImageDraw.getfont | — | — | — | — | — | — |
| ImageDraw.line | — | — | — | — | — | — |
| ImageDraw.multiline_text | — | — | — | — | — | — |
| ImageDraw.multiline_textbbox | — | — | — | — | — | — |
| ImageDraw.pieslice | — | — | — | — | — | — |
| ImageDraw.point | — | — | — | — | — | — |
| ImageDraw.polygon | — | — | — | — | — | — |
| ImageDraw.rectangle | — | — | — | — | — | — |
| ImageDraw.regular_polygon | — | — | — | — | — | — |
| ImageDraw.rounded_rectangle | — | — | — | — | — | — |
| ImageDraw.text | — | — | — | — | — | — |
| ImageDraw.textbbox | — | — | — | — | — | — |
| ImageDraw.textlength | — | — | — | — | — | — |

### ImageEnhance

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageEnhance.Brightness | 12399.75× ⚠️ | 12399.75× ⚠️ | 2.26× | 12399.75× ⚠️ | 12399.75× ⚠️ | 12399.75× ⚠️ |
| ImageEnhance.Color | 16998.00× ⚠️ | 16998.00× ⚠️ | 16998.00× ⚠️ | 16998.00× ⚠️ | 16998.00× ⚠️ | 16998.00× ⚠️ |
| ImageEnhance.Contrast | 19232.07× ⚠️ | 19232.07× ⚠️ | 19232.07× ⚠️ | 19232.07× ⚠️ | 19232.07× ⚠️ | 19232.07× ⚠️ |
| ImageEnhance.Sharpness | 36974.88× ⚠️ | 36974.88× ⚠️ | 36974.88× ⚠️ | 36974.88× ⚠️ | 36974.88× ⚠️ | 36974.88× ⚠️ |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | — | — | — | — | — | — |
| ImageFilter.BoxBlur | — | — | — | — | — | — |
| ImageFilter.CONTOUR | — | — | — | — | — | — |
| ImageFilter.Color3DLUT | — | — | — | — | — | — |
| ImageFilter.DETAIL | — | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE | — | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE_MORE | — | — | — | — | — | — |
| ImageFilter.EMBOSS | — | — | — | — | — | — |
| ImageFilter.FIND_EDGES | — | — | — | — | — | — |
| ImageFilter.GaussianBlur | — | — | 2.01× | — | — | — |
| ImageFilter.Kernel | — | — | — | — | — | — |
| ImageFilter.MaxFilter | — | — | — | — | — | — |
| ImageFilter.MedianFilter | — | — | — | — | — | — |
| ImageFilter.MinFilter | — | — | — | — | — | — |
| ImageFilter.ModeFilter | — | — | — | — | — | — |
| ImageFilter.RankFilter | — | — | — | — | — | — |
| ImageFilter.SHARPEN | — | — | — | — | — | — |
| ImageFilter.SMOOTH | — | — | — | — | — | — |
| ImageFilter.SMOOTH_MORE | — | — | — | — | — | — |
| ImageFilter.UnsharpMask | — | — | — | — | — | — |

### ImageFont

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFont.FreeTypeFont | — | — | — | — | — | — |
| ImageFont.ImageFont | — | — | — | — | — | — |
| ImageFont.FreeTypeFont.getbbox | — | — | — | — | — | — |
| ImageFont.ImageFont.getbbox | — | — | — | — | — | — |
| ImageFont.FreeTypeFont.getlength | — | — | — | — | — | — |
| ImageFont.ImageFont.getlength | — | — | — | — | — | — |
| ImageFont.FreeTypeFont.getmask | — | — | — | — | — | — |
| ImageFont.ImageFont.getmask | — | — | — | — | — | — |
| ImageFont.FreeTypeFont.getmetrics | — | — | — | — | — | — |
| ImageFont.FreeTypeFont.getname | — | — | — | — | — | — |
| ImageFont.load | — | — | — | — | — | — |
| ImageFont.load_default | — | — | — | — | — | — |
| ImageFont.load_default_imagefont | — | — | — | — | — | — |
| ImageFont.load_path | — | — | — | — | — | — |
| ImageFont.truetype | — | — | — | — | — | — |

### ImageModule

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageModule.new | — | — | — | — | — | — |
| ImageModule.alpha_composite | — | — | — | — | — | — |
| ImageModule.blend | — | — | — | — | — | — |
| ImageModule.composite | — | — | — | — | — | — |
| ImageModule.effect_noise | — | — | — | — | — | — |
| ImageModule.eval | — | — | — | — | — | — |
| ImageModule.fromarray | — | — | — | — | — | — |
| ImageModule.frombytes | — | — | — | — | — | — |
| ImageModule.merge | — | — | — | — | — | — |
| ImageModule.open | — | — | — | — | — | — |

### ImageOps

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageOps.crop | 0.44× | 0.44× | 0.44× | 0.44× | 0.44× | 0.44× |
| ImageOps.autocontrast | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× |
| ImageOps.colorize | 0.93× | 0.93× | 0.93× | 0.93× | 0.93× | 0.93× |
| ImageOps.contain | 0.82× | 0.82× | 0.82× | 0.82× | 0.82× | 0.82× |
| ImageOps.cover | — | — | — | — | — | — |
| ImageOps.deform | — | — | — | — | — | — |
| ImageOps.equalize | 0.13× | 0.13× | 0.13× | 0.13× | 0.13× | 0.13× |
| ImageOps.exif_transpose | — | — | — | — | — | — |
| ImageOps.expand | 0.51× | 0.51× | 0.51× | 0.51× | 0.51× | 0.51× |
| ImageOps.fit | — | — | — | — | — | — |
| ImageOps.flip | 3.89× | 3.89× | 3.89× | 3.89× | 3.89× | 3.89× |
| ImageOps.grayscale | 3.83× | 3.83× | 2.27× | 3.83× | 3.83× | 3.83× |
| ImageOps.invert | 0.59× | 0.59× | 0.31× | 0.59× | 0.59× | 0.59× |
| ImageOps.mirror | 4.09× | 4.09× | 4.09× | 4.09× | 4.09× | 4.09× |
| ImageOps.pad | 0.40× | 0.40× | 0.40× | 0.40× | 0.40× | 0.40× |
| ImageOps.posterize | 0.54× | 0.54× | 0.54× | 0.54× | 0.54× | 0.54× |
| ImageOps.scale | 0.84× | 0.84× | 0.84× | 0.84× | 0.84× | 0.84× |
| ImageOps.solarize | 0.32× | 0.32× | 0.32× | 0.32× | 0.32× | 0.32× |

### ImagePalette

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImagePalette.copy | — | — | — | — | — | — |
| ImagePalette.getcolor | — | — | — | — | — | — |
| ImagePalette.getdata | — | — | — | — | — | — |
| ImagePalette.save | — | — | — | — | — | — |
| ImagePalette.tobytes | — | — | — | — | — | — |

### ImageSequence

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageSequence.Iterator | — | — | — | — | — | — |

### ImageStat

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageStat.Stat | — | — | — | — | — | — |
