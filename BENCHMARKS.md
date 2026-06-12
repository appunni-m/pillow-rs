# pillow-rs Benchmarks

> Auto-generated: 2026-06-12 08:27:01 | commit `b31fbb2` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 50 |
| Outliers flagged ⚠️ | 11 |
| Average CPU speedup vs Pillow | 6.52× |
| Native CPU benchmarks run | 61 |
| Missing (no data yet) | 9 |

## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)

> Chaining 20 image operations end-to-end. Measures scheduling, coherence, and clone avoidance.

| Variant | Time (ms) | vs Pillow |
|---------|-----------|-----------|
| ST | 182.80ms |  |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.46× | 0.46× | 1.42× | 0.46× | 0.46× | 0.46× |
| Image.crop | 1.01× | 1.01× | 3.71× | 1.01× | 1.01× | 1.01× |
| Image.rotate | 5.24× | 5.24× | 19.79× | 5.24× | 5.24× | 5.24× |
| Image.transpose | 0.73× | 0.73× | 2.15× | 0.73× | 0.73× | 0.73× |
| Image.thumbnail | — | — | 1.02× | — | — | — |
| Image.new | — | — | 0.20× | — | — | — |
| Image.paste | — | — | — | — | — | — |
| Image.convert | 0.56× | 0.56× | 1.95× | 0.56× | 0.56× | 0.56× |
| Image.filter | 1.17× | 1.17× | 2.25× | 1.17× | 1.17× | 1.17× |
| Image.open | 0.53× | 0.53× | 1.98× | 0.53× | 0.53× | 0.53× |
| Image.save | 3.09× | 3.09× | 2.99× | 3.09× | 3.09× | 3.09× |
| Image.tobytes | 17.48× | 17.48× | 17.48× | 17.48× | 17.48× | 17.48× |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.46× | 0.46× | 1.42× | 0.46× | 0.46× | 0.46× |
| Image.crop | 1.01× | 1.01× | 3.71× | 1.01× | 1.01× | 1.01× |
| Image.rotate | 5.24× | 5.24× | 19.79× | 5.24× | 5.24× | 5.24× |
| Image.transpose | 0.73× | 0.73× | 2.15× | 0.73× | 0.73× | 0.73× |
| Image.thumbnail | — | — | 1.02× | — | — | — |
| Image.new | — | — | 0.20× | — | — | — |
| Image.paste | — | — | — | — | — | — |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | 0.25× | 0.25× | 0.25× | 0.25× | 0.25× | 0.25× |
| Image.close | 136.07× ⚠️ | 136.07× ⚠️ | 136.07× ⚠️ | 136.07× ⚠️ | 136.07× ⚠️ | 136.07× ⚠️ |
| Image.convert | 0.56× | 0.56× | 1.95× | 0.56× | 0.56× | 0.56× |
| Image.copy | 0.78× | 0.78× | 0.78× | 0.78× | 0.78× | 0.78× |
| Image.draft | 185.49× ⚠️ | 185.49× ⚠️ | 185.49× ⚠️ | 185.49× ⚠️ | 185.49× ⚠️ | 185.49× ⚠️ |
| Image.effect_spread | 98.22× | 98.22× | 98.22× | 98.22× | 98.22× | 98.22× |
| Image.entropy | 0.30× | 0.30× | 0.30× | 0.30× | 0.30× | 0.30× |
| Image.filter | 1.17× | 1.17× | 2.25× | 1.17× | 1.17× | 1.17× |
| Image.frombytes | — | — | — | — | — | — |
| Image.get_child_images | 54.16× | 54.16× | 54.16× | 54.16× | 54.16× | 54.16× |
| Image.get_flattened_data | 29.65× | 29.65× | 29.65× | 29.65× | 29.65× | 29.65× |
| Image.getbands | 0.37× | 0.37× | 2.62× | 0.37× | 0.37× | 0.37× |
| Image.getbbox | 0.70× | 0.70× | 2.73× | 0.70× | 0.70× | 0.70× |
| Image.getchannel | 0.63× | 0.63× | 0.63× | 0.63× | 0.63× | 0.63× |
| Image.getcolors | 0.20× | 0.20× | 0.20× | 0.20× | 0.20× | 0.20× |
| Image.getdata | 0.66× | 0.66× | 0.66× | 0.66× | 0.66× | 0.66× |
| Image.getexif | 237.31× ⚠️ | 237.31× ⚠️ | 237.31× ⚠️ | 237.31× ⚠️ | 237.31× ⚠️ | 237.31× ⚠️ |
| Image.getextrema | 0.67× | 0.67× | 2.24× | 0.67× | 0.67× | 0.67× |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | 1025.02× ⚠️ | 1025.02× ⚠️ | 1025.02× ⚠️ | 1025.02× ⚠️ | 1025.02× ⚠️ | 1025.02× ⚠️ |
| Image.getpixel | 0.20× | 0.20× | 2.85× | 0.20× | 0.20× | 0.20× |
| Image.getprojection | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | 0.75× | 0.75× | 2.59× | 0.75× | 0.75× | 0.75× |
| Image.load | 0.70× | 0.70× | 0.70× | 0.70× | 0.70× | 0.70× |
| Image.open | 0.53× | 0.53× | 1.98× | 0.53× | 0.53× | 0.53× |
| Image.point | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× |
| Image.putalpha | 34.09× | 34.09× | 34.09× | 34.09× | 34.09× | 34.09× |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | 691.02× ⚠️ | 691.02× ⚠️ | 691.02× ⚠️ | 691.02× ⚠️ | 691.02× ⚠️ | 691.02× ⚠️ |
| Image.putpixel | 4743.98× ⚠️ | 4743.98× ⚠️ | 86.93× | 4743.98× ⚠️ | 4743.98× ⚠️ | 4743.98× ⚠️ |
| Image.quantize | 1.98× | 1.98× | 1.98× | 1.98× | 1.98× | 1.98× |
| Image.reduce | 0.41× | 0.41× | 1.20× | 0.41× | 0.41× | 0.41× |
| Image.remap_palette | 0.28× | 0.28× | 0.28× | 0.28× | 0.28× | 0.28× |
| Image.save | 3.09× | 3.09× | 2.99× | 3.09× | 3.09× | 3.09× |
| Image.seek | 114.10× ⚠️ | 114.10× ⚠️ | 114.10× ⚠️ | 114.10× ⚠️ | 114.10× ⚠️ | 114.10× ⚠️ |
| Image.show | — | — | — | — | — | — |
| Image.split | 0.31× | 0.31× | 1.20× | 0.31× | 0.31× | 0.31× |
| Image.tell | 50.81× | 50.81× | 50.81× | 50.81× | 50.81× | 50.81× |
| Image.tobitmap | 0.04× | 0.04× | 0.04× | 0.04× | 0.04× | 0.04× |
| Image.tobytes | 17.48× | 17.48× | 17.48× | 17.48× | 17.48× | 17.48× |
| Image.transform | — | — | — | — | — | — |
| Image.verify | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× |

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
| ImageColor.getcolor | 0.45× | 0.45× | 0.45× | 0.45× | 0.45× | 0.45× |
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
| ImageEnhance.Brightness | 22087.06× ⚠️ | 22087.06× ⚠️ | 2.38× | 22087.06× ⚠️ | 22087.06× ⚠️ | 22087.06× ⚠️ |
| ImageEnhance.Color | 16998.00× ⚠️ | 16998.00× ⚠️ | 16998.00× ⚠️ | 16998.00× ⚠️ | 16998.00× ⚠️ | 16998.00× ⚠️ |
| ImageEnhance.Contrast | 21903.19× ⚠️ | 21903.19× ⚠️ | 21903.19× ⚠️ | 21903.19× ⚠️ | 21903.19× ⚠️ | 21903.19× ⚠️ |
| ImageEnhance.Sharpness | 40336.24× ⚠️ | 40336.24× ⚠️ | 40336.24× ⚠️ | 40336.24× ⚠️ | 40336.24× ⚠️ | 40336.24× ⚠️ |

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
| ImageFilter.GaussianBlur | — | — | 2.04× | — | — | — |
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
| ImageOps.crop | 0.52× | 0.52× | 0.52× | 0.52× | 0.52× | 0.52× |
| ImageOps.autocontrast | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× |
| ImageOps.colorize | 1.03× | 1.03× | 1.03× | 1.03× | 1.03× | 1.03× |
| ImageOps.contain | 0.93× | 0.93× | 0.93× | 0.93× | 0.93× | 0.93× |
| ImageOps.cover | — | — | — | — | — | — |
| ImageOps.deform | — | — | — | — | — | — |
| ImageOps.equalize | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× |
| ImageOps.exif_transpose | — | — | — | — | — | — |
| ImageOps.expand | 0.58× | 0.58× | 0.58× | 0.58× | 0.58× | 0.58× |
| ImageOps.fit | — | — | — | — | — | — |
| ImageOps.flip | 4.36× | 4.36× | 4.36× | 4.36× | 4.36× | 4.36× |
| ImageOps.grayscale | 4.05× | 4.05× | 2.07× | 4.05× | 4.05× | 4.05× |
| ImageOps.invert | 0.62× | 0.62× | 0.30× | 0.62× | 0.62× | 0.62× |
| ImageOps.mirror | 4.01× | 4.01× | 4.01× | 4.01× | 4.01× | 4.01× |
| ImageOps.pad | 0.46× | 0.46× | 0.46× | 0.46× | 0.46× | 0.46× |
| ImageOps.posterize | 0.55× | 0.55× | 0.55× | 0.55× | 0.55× | 0.55× |
| ImageOps.scale | 0.96× | 0.96× | 0.96× | 0.96× | 0.96× | 0.96× |
| ImageOps.solarize | 0.36× | 0.36× | 0.36× | 0.36× | 0.36× | 0.36× |

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
