# pillow-rs Benchmarks

> Auto-generated: 2026-06-12 10:10:37 | commit `6220b3e` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 88 |
| Outliers flagged ⚠️ | 18 |
| Average CPU speedup vs Pillow | 1.50× |
| Native CPU benchmarks run | 107 |
| Missing (no data yet) | 8 |

## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)

> Chaining 20 image operations end-to-end. Measures scheduling, coherence, and clone avoidance.

| Variant | Time (ms) | vs Pillow |
|---------|-----------|-----------|
| ST | 189.08ms | 0.31× |
| Pillow (reference) | 59.0ms | — |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.51× | — | 1.53× | — | 1.20× | 1.94× |
| Image.crop | 0.98× | — | 3.37× | — | 3.40× | 3.72× |
| Image.rotate | 5.00× | — | 19.80× | — | 12.79× | 13.29× |
| Image.transpose | 0.69× | — | 2.27× | — | 2.02× | 2.03× |
| Image.thumbnail | 0.23× | — | 1.12× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.20× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.convert | 0.65× | — | 2.09× | — | 1.74× | 1.66× |
| Image.filter | 1.10× | — | 2.22× | — | 2.59× | 2.69× |
| Image.open | 0.54× | — | 1.93× | — | ⚠️ | ⚠️ |
| Image.save | 3.12× | — | 2.92× | — | 3.48× | 3.48× |
| Image.tobytes | 17.33× | — | — | — | — | — |

## Performance-Critical Operations

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.51× | — | 1.53× | — | 1.20× | 1.94× |
| Image.crop | 0.98× | — | 3.37× | — | 3.40× | 3.72× |
| Image.rotate | 5.00× | — | 19.80× | — | 12.79× | 13.29× |
| Image.transpose | 0.69× | — | 2.27× | — | 2.02× | 2.03× |
| Image.thumbnail | 0.23× | — | 1.12× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.20× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | 0.24× | — | — | — | — | — |
| Image.convert | 0.65× | — | 2.09× | — | 1.74× | 1.66× |
| Image.draft | ⚠️ | — | — | — | — | — |
| Image.effect_spread | ⚠️ | — | — | — | — | — |
| Image.entropy | 0.32× | — | — | — | — | — |
| Image.filter | 1.10× | — | 2.22× | — | 2.59× | 2.69× |
| Image.frombytes | — | — | — | — | — | — |
| Image.open | 0.54× | — | 1.93× | — | ⚠️ | ⚠️ |
| Image.point | 0.09× | — | — | — | — | — |
| Image.putalpha | 14.59× | — | — | — | — | — |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | ⚠️ | — | — | — | — | — |
| Image.putpixel | ⚠️ | — | ⚠️ | — | 3.86× | 3.85× |
| Image.quantize | 2.10× | — | — | — | — | — |
| Image.reduce | 0.37× | — | 1.33× | — | 2.32× | 1.97× |
| Image.remap_palette | 0.28× | — | — | — | — | — |
| Image.save | 3.12× | — | 2.92× | — | 3.48× | 3.48× |
| Image.split | 0.30× | — | 1.05× | — | 9.45× | 9.38× |
| Image.tobitmap | 0.04× | — | — | — | — | — |
| Image.transform | — | — | — | — | — | — |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | 0.40× | — | — | — | — | — |
| ImageChops.add_modulo | 0.30× | — | — | — | — | — |
| ImageChops.blend | — | — | — | — | — | — |
| ImageChops.composite | — | — | — | — | — | — |
| ImageChops.constant | 0.53× | — | — | — | — | — |
| ImageChops.darker | 0.31× | — | — | — | — | — |
| ImageChops.difference | 0.28× | — | — | — | — | — |
| ImageChops.duplicate | — | — | — | — | — | — |
| ImageChops.hard_light | 0.40× | — | — | — | — | — |
| ImageChops.invert | 0.54× | — | — | — | — | — |
| ImageChops.lighter | 0.31× | — | — | — | — | — |
| ImageChops.logical_and | 0.32× | — | — | — | — | — |
| ImageChops.logical_or | 0.36× | — | — | — | — | — |
| ImageChops.logical_xor | 0.39× | — | — | — | — | — |
| ImageChops.multiply | 0.31× | — | — | — | — | — |
| ImageChops.offset | 0.74× | — | — | — | — | — |
| ImageChops.overlay | 0.40× | — | — | — | — | — |
| ImageChops.screen | 0.37× | — | — | — | — | — |
| ImageChops.soft_light | 0.43× | — | — | — | — | — |
| ImageChops.subtract | 0.36× | — | — | — | — | — |
| ImageChops.subtract_modulo | 0.28× | — | — | — | — | — |

### ImageEnhance

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageEnhance.Brightness | ⚠️ | — | 2.43× | — | 5.00× | 5.26× |
| ImageEnhance.Color | ⚠️ | — | — | — | — | — |
| ImageEnhance.Contrast | ⚠️ | — | — | — | — | — |
| ImageEnhance.Sharpness | ⚠️ | — | — | — | — | — |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | 1.87× | — | — | — | — | — |
| ImageFilter.BoxBlur | — | — | — | — | — | — |
| ImageFilter.CONTOUR | 1.18× | — | — | — | — | — |
| ImageFilter.Color3DLUT | — | — | — | — | — | — |
| ImageFilter.DETAIL | 1.13× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE | 1.23× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE_MORE | 1.11× | — | — | — | — | — |
| ImageFilter.EMBOSS | 1.01× | — | — | — | — | — |
| ImageFilter.FIND_EDGES | 1.05× | — | — | — | — | — |
| ImageFilter.GaussianBlur | — | — | 2.01× | — | 2.15× | 2.31× |
| ImageFilter.Kernel | — | — | — | — | — | — |
| ImageFilter.MaxFilter | — | — | — | — | — | — |
| ImageFilter.MedianFilter | — | — | — | — | — | — |
| ImageFilter.MinFilter | — | — | — | — | — | — |
| ImageFilter.ModeFilter | — | — | — | — | — | — |
| ImageFilter.RankFilter | — | — | — | — | — | — |
| ImageFilter.SHARPEN | 1.18× | — | — | — | — | — |
| ImageFilter.SMOOTH | 1.10× | — | — | — | — | — |
| ImageFilter.SMOOTH_MORE | 2.00× | — | — | — | — | — |
| ImageFilter.UnsharpMask | — | — | — | — | — | — |

### ImageModule

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageModule.new | — | — | — | — | — | — |
| ImageModule.alpha_composite | — | — | — | — | — | — |
| ImageModule.blend | 0.11× | — | — | — | — | — |
| ImageModule.composite | 0.06× | — | — | — | — | — |
| ImageModule.effect_noise | — | — | — | — | — | — |
| ImageModule.eval | — | — | — | — | — | — |
| ImageModule.fromarray | — | — | — | — | — | — |
| ImageModule.frombytes | — | — | — | — | — | — |
| ImageModule.merge | — | — | — | — | — | — |
| ImageModule.open | — | — | — | — | — | — |

### ImageOps

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageOps.crop | 0.50× | — | — | — | — | — |
| ImageOps.autocontrast | 0.11× | — | — | — | — | — |
| ImageOps.colorize | 0.53× | — | — | — | — | — |
| ImageOps.contain | 0.94× | — | — | — | — | — |
| ImageOps.cover | — | — | — | — | — | — |
| ImageOps.deform | — | — | — | — | — | — |
| ImageOps.equalize | 0.13× | — | — | — | — | — |
| ImageOps.exif_transpose | — | — | — | — | — | — |
| ImageOps.expand | 0.56× | — | — | — | — | — |
| ImageOps.fit | — | — | — | — | — | — |
| ImageOps.flip | 4.48× | — | — | — | — | — |
| ImageOps.grayscale | 3.70× | — | 2.14× | — | 4.49× | 4.82× |
| ImageOps.invert | 0.54× | — | 0.29× | — | 0.94× | 0.96× |
| ImageOps.mirror | 4.25× | — | — | — | — | — |
| ImageOps.pad | 0.47× | — | — | — | — | — |
| ImageOps.posterize | 0.59× | — | — | — | — | — |
| ImageOps.scale | 0.91× | — | — | — | — | — |
| ImageOps.solarize | 0.37× | — | — | — | — | — |

## Non-Performance-Critical Operations

> Metadata, I/O, analysis, drawing, and font operations. Not benchmarked for speed — 
> use CPU path timing as reference.

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.close | ⚠️ | — | — | — | — | — |
| Image.copy | 0.64× | — | — | — | — | — |
| Image.get_child_images | ⚠️ | — | — | — | — | — |
| Image.get_flattened_data | 34.44× | — | — | — | — | — |
| Image.getbands | 0.38× | — | 2.53× | — | ⚠️ | ⚠️ |
| Image.getbbox | 0.71× | — | 2.66× | — | 15.74× | 14.82× |
| Image.getchannel | 0.65× | — | — | — | — | — |
| Image.getcolors | 0.17× | — | — | — | — | — |
| Image.getdata | 0.64× | — | — | — | — | — |
| Image.getexif | ⚠️ | — | — | — | — | — |
| Image.getextrema | 0.62× | — | 2.22× | — | 8.06× | 7.96× |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | ⚠️ | — | — | — | — | — |
| Image.getpixel | 0.19× | — | 2.71× | — | ⚠️ | ⚠️ |
| Image.getprojection | 0.09× | — | — | — | — | — |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | 0.79× | — | 2.60× | — | 7.66× | 7.58× |
| Image.load | 0.68× | — | — | — | — | — |
| Image.seek | ⚠️ | — | — | — | — | — |
| Image.show | — | — | — | — | — | — |
| Image.tell | ⚠️ | — | — | — | — | — |
| Image.tobytes | 17.33× | — | — | — | — | — |
| Image.verify | 0.08× | — | — | — | — | — |
| ImageColor.getcolor | 0.39× | — | — | — | — | — |
| ImageColor.getrgb | 0.34× | — | — | — | — | — |
| ImageDraw.arc | 0.66× | — | — | — | — | — |
| ImageDraw.bitmap | ⚠️ | — | — | — | — | — |
| ImageDraw.chord | 0.12× | — | — | — | — | — |
| ImageDraw.circle | 0.16× | — | — | — | — | — |
| ImageDraw.ellipse | 0.78× | — | — | — | — | — |
| ImageDraw.getfont | — | — | — | — | — | — |
| ImageDraw.line | 0.58× | — | — | — | — | — |
| ImageDraw.multiline_text | — | — | — | — | — | — |
| ImageDraw.multiline_textbbox | — | — | — | — | — | — |
| ImageDraw.pieslice | 0.13× | — | — | — | — | — |
| ImageDraw.point | — | — | — | — | — | — |
| ImageDraw.polygon | 0.12× | — | — | — | — | — |
| ImageDraw.rectangle | 0.88× | — | — | — | — | — |
| ImageDraw.regular_polygon | — | — | — | — | — | — |
| ImageDraw.rounded_rectangle | 0.16× | — | — | — | — | — |
| ImageDraw.text | — | — | — | — | — | — |
| ImageDraw.textbbox | — | — | — | — | — | — |
| ImageDraw.textlength | — | — | — | — | — | — |
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
| ImageFont.load_default | ⚠️ | — | — | — | — | — |
| ImageFont.load_default_imagefont | — | — | — | — | — | — |
| ImageFont.load_path | — | — | — | — | — | — |
| ImageFont.truetype | ⚠️ | — | — | — | — | — |
| ImagePalette.copy | — | — | — | — | — | — |
| ImagePalette.getcolor | — | — | — | — | — | — |
| ImagePalette.getdata | — | — | — | — | — | — |
| ImagePalette.save | — | — | — | — | — | — |
| ImagePalette.tobytes | ⚠️ | — | — | — | — | — |
| ImageSequence.Iterator | — | — | — | — | — | — |
| ImageStat.Stat | 0.72× | — | — | — | — | — |

## ⚠️ Suspicious Ratios (>5× or <0.1×)

| Function | Source | Ratio |
|----------|--------|-------|
| Image.rotate | CPU | 5.00× |
| Image.rotate | WASM | 19.80× |
| Image.close | CPU | 155.51× |
| Image.draft | CPU | 185.49× |
| Image.effect_spread | CPU | 98.31× |
| Image.get_child_images | CPU | 198.59× |
| Image.get_flattened_data | CPU | 34.44× |
| Image.getexif | CPU | 296.64× |
| Image.getpalette | CPU | 939.60× |
| Image.getprojection | CPU | 0.09× |
| Image.point | CPU | 0.09× |
| Image.putalpha | CPU | 14.59× |
| Image.putpalette | CPU | 806.19× |
| Image.putpixel | CPU | 5156.50× |
| Image.putpixel | WASM | 69.34× |
| Image.seek | CPU | 146.70× |
| Image.tell | CPU | 169.36× |
| Image.tobitmap | CPU | 0.04× |
| Image.tobytes | CPU | 17.33× |
| Image.verify | CPU | 0.08× |
| ... | ... | +9 more |

## PIL Parity Tests

**202 passed, 0 failed** (Pillow 12.2.0)
