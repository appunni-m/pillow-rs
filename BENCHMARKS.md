# pillow-rs Benchmarks

> Auto-generated: 2026-06-12 09:41:19 | commit `b24c2e9` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 85 |
| Outliers flagged ⚠️ | 8 |
| Average CPU speedup vs Pillow | 5.66× |
| Native CPU benchmarks run | 94 |
| Missing (no data yet) | 8 |

## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)

> Chaining 20 image operations end-to-end. Measures scheduling, coherence, and clone avoidance.

| Variant | Time (ms) | vs Pillow |
|---------|-----------|-----------|
| ST | 180.24ms |  |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.42× | 0.42× | 1.42× | 0.42× | 1.20× | 1.94× |
| Image.crop | 0.96× | 0.96× | 3.71× | 0.96× | 3.40× | 3.72× |
| Image.rotate | 4.83× | 4.83× | 19.79× | 4.83× | 12.79× | 13.29× |
| Image.transpose | 0.68× | 0.68× | 2.15× | 0.68× | 2.02× | 2.03× |
| Image.thumbnail | — | — | 1.02× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.20× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.convert | 0.62× | 0.62× | 1.95× | 0.62× | 1.74× | 1.66× |
| Image.filter | 1.11× | 1.11× | 2.25× | 1.11× | 2.59× | 2.69× |
| Image.open | 0.54× | 0.54× | 1.98× | 0.54× | 309.90× ⚠️ | 328.40× ⚠️ |
| Image.save | 2.98× | 2.98× | 2.99× | 2.98× | 3.48× | 3.48× |
| Image.tobytes | 19.66× | 19.66× | 19.66× | 19.66× | 19.66× | 19.66× |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.42× | 0.42× | 1.42× | 0.42× | 1.20× | 1.94× |
| Image.crop | 0.96× | 0.96× | 3.71× | 0.96× | 3.40× | 3.72× |
| Image.rotate | 4.83× | 4.83× | 19.79× | 4.83× | 12.79× | 13.29× |
| Image.transpose | 0.68× | 0.68× | 2.15× | 0.68× | 2.02× | 2.03× |
| Image.thumbnail | — | — | 1.02× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.20× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | 0.21× | 0.21× | 0.21× | 0.21× | 0.21× | 0.21× |
| Image.close | 57.29× | 57.29× | 57.29× | 57.29× | 57.29× | 57.29× |
| Image.convert | 0.62× | 0.62× | 1.95× | 0.62× | 1.74× | 1.66× |
| Image.copy | 0.78× | 0.78× | 0.78× | 0.78× | 0.78× | 0.78× |
| Image.draft | 17.95× | 17.95× | 17.95× | 17.95× | 17.95× | 17.95× |
| Image.effect_spread | 98.13× | 98.13× | 98.13× | 98.13× | 98.13× | 98.13× |
| Image.entropy | 0.32× | 0.32× | 0.32× | 0.32× | 0.32× | 0.32× |
| Image.filter | 1.11× | 1.11× | 2.25× | 1.11× | 2.59× | 2.69× |
| Image.frombytes | — | — | — | — | — | — |
| Image.get_child_images | 17.27× | 17.27× | 17.27× | 17.27× | 17.27× | 17.27× |
| Image.get_flattened_data | 32.96× | 32.96× | 32.96× | 32.96× | 32.96× | 32.96× |
| Image.getbands | 0.38× | 0.38× | 2.62× | 0.38× | 71.37× | 62.76× |
| Image.getbbox | 0.74× | 0.74× | 2.73× | 0.74× | 15.74× | 14.82× |
| Image.getchannel | 0.65× | 0.65× | 0.65× | 0.65× | 0.65× | 0.65× |
| Image.getcolors | 0.17× | 0.17× | 0.17× | 0.17× | 0.17× | 0.17× |
| Image.getdata | 0.67× | 0.67× | 0.67× | 0.67× | 0.67× | 0.67× |
| Image.getexif | 84.75× | 84.75× | 84.75× | 84.75× | 84.75× | 84.75× |
| Image.getextrema | 0.69× | 0.69× | 2.24× | 0.69× | 8.06× | 7.96× |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | 1610.75× ⚠️ | 1610.75× ⚠️ | 1610.75× ⚠️ | 1610.75× ⚠️ | 1610.75× ⚠️ | 1610.75× ⚠️ |
| Image.getpixel | 0.20× | 0.20× | 2.85× | 0.20× | 25114.13× ⚠️ | 22159.53× ⚠️ |
| Image.getprojection | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | 0.72× | 0.72× | 2.59× | 0.72× | 7.66× | 7.58× |
| Image.load | 0.71× | 0.71× | 0.71× | 0.71× | 0.71× | 0.71× |
| Image.open | 0.54× | 0.54× | 1.98× | 0.54× | 309.90× ⚠️ | 328.40× ⚠️ |
| Image.point | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× |
| Image.putalpha | 18.12× | 18.12× | 18.12× | 18.12× | 18.12× | 18.12× |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | 439.74× ⚠️ | 439.74× ⚠️ | 439.74× ⚠️ | 439.74× ⚠️ | 439.74× ⚠️ | 439.74× ⚠️ |
| Image.putpixel | 1778.99× ⚠️ | 1778.99× ⚠️ | 86.93× | 1778.99× ⚠️ | 3.86× | 3.85× |
| Image.quantize | 2.05× | 2.05× | 2.05× | 2.05× | 2.05× | 2.05× |
| Image.reduce | 0.35× | 0.35× | 1.20× | 0.35× | 2.32× | 1.97× |
| Image.remap_palette | 0.26× | 0.26× | 0.26× | 0.26× | 0.26× | 0.26× |
| Image.save | 2.98× | 2.98× | 2.99× | 2.98× | 3.48× | 3.48× |
| Image.seek | 60.40× | 60.40× | 60.40× | 60.40× | 60.40× | 60.40× |
| Image.show | — | — | — | — | — | — |
| Image.split | 0.31× | 0.31× | 1.20× | 0.31× | 9.45× | 9.38× |
| Image.tell | 11.55× | 11.55× | 11.55× | 11.55× | 11.55× | 11.55× |
| Image.tobitmap | 0.04× | 0.04× | 0.04× | 0.04× | 0.04× | 0.04× |
| Image.tobytes | 19.66× | 19.66× | 19.66× | 19.66× | 19.66× | 19.66× |
| Image.transform | — | — | — | — | — | — |
| Image.verify | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | 0.38× | 0.38× | 0.38× | 0.38× | 0.38× | 0.38× |
| ImageChops.add_modulo | 0.32× | 0.32× | 0.32× | 0.32× | 0.32× | 0.32× |
| ImageChops.blend | — | — | — | — | — | — |
| ImageChops.composite | — | — | — | — | — | — |
| ImageChops.constant | 0.55× | 0.55× | 0.55× | 0.55× | 0.55× | 0.55× |
| ImageChops.darker | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× |
| ImageChops.difference | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× |
| ImageChops.duplicate | — | — | — | — | — | — |
| ImageChops.hard_light | 0.42× | 0.42× | 0.42× | 0.42× | 0.42× | 0.42× |
| ImageChops.invert | 0.48× | 0.48× | 0.48× | 0.48× | 0.48× | 0.48× |
| ImageChops.lighter | 0.34× | 0.34× | 0.34× | 0.34× | 0.34× | 0.34× |
| ImageChops.logical_and | 0.41× | 0.41× | 0.41× | 0.41× | 0.41× | 0.41× |
| ImageChops.logical_or | 0.44× | 0.44× | 0.44× | 0.44× | 0.44× | 0.44× |
| ImageChops.logical_xor | 0.39× | 0.39× | 0.39× | 0.39× | 0.39× | 0.39× |
| ImageChops.multiply | 0.34× | 0.34× | 0.34× | 0.34× | 0.34× | 0.34× |
| ImageChops.offset | 0.82× | 0.82× | 0.82× | 0.82× | 0.82× | 0.82× |
| ImageChops.overlay | 0.42× | 0.42× | 0.42× | 0.42× | 0.42× | 0.42× |
| ImageChops.screen | 0.35× | 0.35× | 0.35× | 0.35× | 0.35× | 0.35× |
| ImageChops.soft_light | 0.48× | 0.48× | 0.48× | 0.48× | 0.48× | 0.48× |
| ImageChops.subtract | 0.36× | 0.36× | 0.36× | 0.36× | 0.36× | 0.36× |
| ImageChops.subtract_modulo | 0.30× | 0.30× | 0.30× | 0.30× | 0.30× | 0.30× |

### ImageColor

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageColor.getcolor | 0.08× | 0.08× | 0.08× | 0.08× | 0.08× | 0.08× |
| ImageColor.getrgb | 0.19× | 0.19× | 0.19× | 0.19× | 0.19× | 0.19× |

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
| ImageEnhance.Brightness | 15706.35× ⚠️ | 15706.35× ⚠️ | 2.38× | 15706.35× ⚠️ | 5.00× | 5.26× |
| ImageEnhance.Color | 16103.36× ⚠️ | 16103.36× ⚠️ | 16103.36× ⚠️ | 16103.36× ⚠️ | 16103.36× ⚠️ | 16103.36× ⚠️ |
| ImageEnhance.Contrast | 21903.19× ⚠️ | 21903.19× ⚠️ | 21903.19× ⚠️ | 21903.19× ⚠️ | 21903.19× ⚠️ | 21903.19× ⚠️ |
| ImageEnhance.Sharpness | 35028.84× ⚠️ | 35028.84× ⚠️ | 35028.84× ⚠️ | 35028.84× ⚠️ | 35028.84× ⚠️ | 35028.84× ⚠️ |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | 2.15× | 2.15× | 2.15× | 2.15× | 2.15× | 2.15× |
| ImageFilter.BoxBlur | — | — | — | — | — | — |
| ImageFilter.CONTOUR | 1.23× | 1.23× | 1.23× | 1.23× | 1.23× | 1.23× |
| ImageFilter.Color3DLUT | — | — | — | — | — | — |
| ImageFilter.DETAIL | 1.15× | 1.15× | 1.15× | 1.15× | 1.15× | 1.15× |
| ImageFilter.EDGE_ENHANCE | 1.32× | 1.32× | 1.32× | 1.32× | 1.32× | 1.32× |
| ImageFilter.EDGE_ENHANCE_MORE | 1.24× | 1.24× | 1.24× | 1.24× | 1.24× | 1.24× |
| ImageFilter.EMBOSS | 1.19× | 1.19× | 1.19× | 1.19× | 1.19× | 1.19× |
| ImageFilter.FIND_EDGES | 1.28× | 1.28× | 1.28× | 1.28× | 1.28× | 1.28× |
| ImageFilter.GaussianBlur | — | — | 2.04× | — | 2.15× | 2.31× |
| ImageFilter.Kernel | — | — | — | — | — | — |
| ImageFilter.MaxFilter | — | — | — | — | — | — |
| ImageFilter.MedianFilter | — | — | — | — | — | — |
| ImageFilter.MinFilter | — | — | — | — | — | — |
| ImageFilter.ModeFilter | — | — | — | — | — | — |
| ImageFilter.RankFilter | — | — | — | — | — | — |
| ImageFilter.SHARPEN | 1.30× | 1.30× | 1.30× | 1.30× | 1.30× | 1.30× |
| ImageFilter.SMOOTH | 1.19× | 1.19× | 1.19× | 1.19× | 1.19× | 1.19× |
| ImageFilter.SMOOTH_MORE | 2.07× | 2.07× | 2.07× | 2.07× | 2.07× | 2.07× |
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
| ImageModule.blend | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× |
| ImageModule.composite | 0.05× | 0.05× | 0.05× | 0.05× | 0.05× | 0.05× |
| ImageModule.effect_noise | — | — | — | — | — | — |
| ImageModule.eval | — | — | — | — | — | — |
| ImageModule.fromarray | — | — | — | — | — | — |
| ImageModule.frombytes | — | — | — | — | — | — |
| ImageModule.merge | — | — | — | — | — | — |
| ImageModule.open | — | — | — | — | — | — |

### ImageOps

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageOps.crop | 0.55× | 0.55× | 0.55× | 0.55× | 0.55× | 0.55× |
| ImageOps.autocontrast | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× |
| ImageOps.colorize | 0.99× | 0.99× | 0.99× | 0.99× | 0.99× | 0.99× |
| ImageOps.contain | 0.92× | 0.92× | 0.92× | 0.92× | 0.92× | 0.92× |
| ImageOps.cover | — | — | — | — | — | — |
| ImageOps.deform | — | — | — | — | — | — |
| ImageOps.equalize | 0.13× | 0.13× | 0.13× | 0.13× | 0.13× | 0.13× |
| ImageOps.exif_transpose | — | — | — | — | — | — |
| ImageOps.expand | 0.61× | 0.61× | 0.61× | 0.61× | 0.61× | 0.61× |
| ImageOps.fit | — | — | — | — | — | — |
| ImageOps.flip | 4.47× | 4.47× | 4.47× | 4.47× | 4.47× | 4.47× |
| ImageOps.grayscale | 3.36× | 3.36× | 2.07× | 3.36× | 4.49× | 4.82× |
| ImageOps.invert | 0.61× | 0.61× | 0.30× | 0.61× | 0.94× | 0.96× |
| ImageOps.mirror | 4.16× | 4.16× | 4.16× | 4.16× | 4.16× | 4.16× |
| ImageOps.pad | 0.47× | 0.47× | 0.47× | 0.47× | 0.47× | 0.47× |
| ImageOps.posterize | 0.63× | 0.63× | 0.63× | 0.63× | 0.63× | 0.63× |
| ImageOps.scale | 0.97× | 0.97× | 0.97× | 0.97× | 0.97× | 0.97× |
| ImageOps.solarize | 0.39× | 0.39× | 0.39× | 0.39× | 0.39× | 0.39× |

### ImagePalette

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImagePalette.copy | — | — | — | — | — | — |
| ImagePalette.getcolor | — | — | — | — | — | — |
| ImagePalette.getdata | — | — | — | — | — | — |
| ImagePalette.save | — | — | — | — | — | — |
| ImagePalette.tobytes | 0.00× ⚠️ | 0.00× ⚠️ | 0.00× ⚠️ | 0.00× ⚠️ | 0.00× ⚠️ | 0.00× ⚠️ |

### ImageSequence

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageSequence.Iterator | — | — | — | — | — | — |

### ImageStat

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageStat.Stat | 0.75× | 0.75× | 0.75× | 0.75× | 0.75× | 0.75× |
