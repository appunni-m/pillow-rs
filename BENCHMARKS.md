# pillow-rs Benchmarks

> Auto-generated: 2026-06-12 10:12:24 | commit `9129d7a` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 91 |
| Outliers flagged ⚠️ | 18 |
| Average CPU speedup vs Pillow | 2.53× |
| Native CPU benchmarks run | 112 |
| Missing (no data yet) | 6 |

## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)

> Chaining 20 image operations end-to-end. Measures scheduling, coherence, and clone avoidance.

| Variant | Time (ms) | vs Pillow |
|---------|-----------|-----------|
| ST | 197.43ms | 0.30× |
| Pillow (reference) | 59.0ms | — |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.48× | — | 1.44× | — | 1.20× | 1.94× |
| Image.crop | 1.01× | — | 3.38× | — | 3.40× | 3.72× |
| Image.rotate | 5.10× | — | 19.21× | — | 12.79× | 13.29× |
| Image.transpose | 0.72× | — | 1.97× | — | 2.02× | 2.03× |
| Image.thumbnail | 0.25× | — | 1.06× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.20× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.convert | 0.67× | — | 1.75× | — | 1.74× | 1.66× |
| Image.filter | 1.10× | — | 2.37× | — | 2.59× | 2.69× |
| Image.open | 0.53× | — | 1.92× | — | ⚠️ | ⚠️ |
| Image.save | 3.07× | — | 2.90× | — | 3.48× | 3.48× |
| Image.tobytes | 14.73× | — | — | — | — | — |

## Performance-Critical Operations

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.48× | — | 1.44× | — | 1.20× | 1.94× |
| Image.crop | 1.01× | — | 3.38× | — | 3.40× | 3.72× |
| Image.rotate | 5.10× | — | 19.21× | — | 12.79× | 13.29× |
| Image.transpose | 0.72× | — | 1.97× | — | 2.02× | 2.03× |
| Image.thumbnail | 0.25× | — | 1.06× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.20× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | 0.19× | — | — | — | — | — |
| Image.convert | 0.67× | — | 1.75× | — | 1.74× | 1.66× |
| Image.draft | ⚠️ | — | — | — | — | — |
| Image.effect_spread | ⚠️ | — | — | — | — | — |
| Image.entropy | 0.38× | — | — | — | — | — |
| Image.filter | 1.10× | — | 2.37× | — | 2.59× | 2.69× |
| Image.frombytes | — | — | — | — | — | — |
| Image.open | 0.53× | — | 1.92× | — | ⚠️ | ⚠️ |
| Image.point | 0.09× | — | — | — | — | — |
| Image.putalpha | 23.97× | — | — | — | — | — |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | ⚠️ | — | — | — | — | — |
| Image.putpixel | ⚠️ | — | ⚠️ | — | 3.86× | 3.85× |
| Image.quantize | 2.06× | — | — | — | — | — |
| Image.reduce | 0.40× | — | 1.23× | — | 2.32× | 1.97× |
| Image.remap_palette | 0.25× | — | — | — | — | — |
| Image.save | 3.07× | — | 2.90× | — | 3.48× | 3.48× |
| Image.split | 0.30× | — | 1.13× | — | 9.45× | 9.38× |
| Image.tobitmap | 0.04× | — | — | — | — | — |
| Image.transform | — | — | — | — | — | — |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | 0.42× | — | — | — | — | — |
| ImageChops.add_modulo | 0.29× | — | — | — | — | — |
| ImageChops.blend | — | — | — | — | — | — |
| ImageChops.composite | — | — | — | — | — | — |
| ImageChops.constant | 0.44× | — | — | — | — | — |
| ImageChops.darker | 0.33× | — | — | — | — | — |
| ImageChops.difference | 0.28× | — | — | — | — | — |
| ImageChops.duplicate | — | — | — | — | — | — |
| ImageChops.hard_light | 0.42× | — | — | — | — | — |
| ImageChops.invert | 0.51× | — | — | — | — | — |
| ImageChops.lighter | 0.30× | — | — | — | — | — |
| ImageChops.logical_and | 0.40× | — | — | — | — | — |
| ImageChops.logical_or | 0.39× | — | — | — | — | — |
| ImageChops.logical_xor | 0.35× | — | — | — | — | — |
| ImageChops.multiply | 0.29× | — | — | — | — | — |
| ImageChops.offset | 0.81× | — | — | — | — | — |
| ImageChops.overlay | 0.39× | — | — | — | — | — |
| ImageChops.screen | 0.36× | — | — | — | — | — |
| ImageChops.soft_light | 0.45× | — | — | — | — | — |
| ImageChops.subtract | 0.34× | — | — | — | — | — |
| ImageChops.subtract_modulo | 0.30× | — | — | — | — | — |

### ImageEnhance

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageEnhance.Brightness | ⚠️ | — | 2.30× | — | 5.00× | 5.26× |
| ImageEnhance.Color | ⚠️ | — | — | — | — | — |
| ImageEnhance.Contrast | ⚠️ | — | — | — | — | — |
| ImageEnhance.Sharpness | ⚠️ | — | — | — | — | — |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | 2.08× | — | — | — | — | — |
| ImageFilter.BoxBlur | — | — | — | — | — | — |
| ImageFilter.CONTOUR | 1.03× | — | — | — | — | — |
| ImageFilter.Color3DLUT | — | — | — | — | — | — |
| ImageFilter.DETAIL | 0.96× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE | 1.15× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE_MORE | 1.16× | — | — | — | — | — |
| ImageFilter.EMBOSS | 1.11× | — | — | — | — | — |
| ImageFilter.FIND_EDGES | 1.13× | — | — | — | — | — |
| ImageFilter.GaussianBlur | — | — | 2.05× | — | 2.15× | 2.31× |
| ImageFilter.Kernel | — | — | — | — | — | — |
| ImageFilter.MaxFilter | — | — | — | — | — | — |
| ImageFilter.MedianFilter | — | — | — | — | — | — |
| ImageFilter.MinFilter | — | — | — | — | — | — |
| ImageFilter.ModeFilter | — | — | — | — | — | — |
| ImageFilter.RankFilter | — | — | — | — | — | — |
| ImageFilter.SHARPEN | 1.03× | — | — | — | — | — |
| ImageFilter.SMOOTH | 1.05× | — | — | — | — | — |
| ImageFilter.SMOOTH_MORE | 1.90× | — | — | — | — | — |
| ImageFilter.UnsharpMask | — | — | — | — | — | — |

### ImageModule

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageModule.new | — | — | — | — | — | — |
| ImageModule.alpha_composite | — | — | — | — | — | — |
| ImageModule.blend | 0.10× | — | — | — | — | — |
| ImageModule.composite | 0.05× | — | — | — | — | — |
| ImageModule.effect_noise | — | — | — | — | — | — |
| ImageModule.eval | — | — | — | — | — | — |
| ImageModule.fromarray | — | — | — | — | — | — |
| ImageModule.frombytes | — | — | — | — | — | — |
| ImageModule.merge | — | — | — | — | — | — |
| ImageModule.open | — | — | — | — | — | — |

### ImageOps

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageOps.crop | 0.52× | — | — | — | — | — |
| ImageOps.autocontrast | 0.11× | — | — | — | — | — |
| ImageOps.colorize | 0.96× | — | — | — | — | — |
| ImageOps.contain | 0.86× | — | — | — | — | — |
| ImageOps.cover | — | — | — | — | — | — |
| ImageOps.deform | — | — | — | — | — | — |
| ImageOps.equalize | 0.13× | — | — | — | — | — |
| ImageOps.exif_transpose | — | — | — | — | — | — |
| ImageOps.expand | 0.57× | — | — | — | — | — |
| ImageOps.fit | — | — | — | — | — | — |
| ImageOps.flip | 4.33× | — | — | — | — | — |
| ImageOps.grayscale | 4.03× | — | 1.98× | — | 4.49× | 4.82× |
| ImageOps.invert | 0.52× | — | 0.29× | — | 0.94× | 0.96× |
| ImageOps.mirror | 3.92× | — | — | — | — | — |
| ImageOps.pad | 0.42× | — | — | — | — | — |
| ImageOps.posterize | 0.59× | — | — | — | — | — |
| ImageOps.scale | 0.83× | — | — | — | — | — |
| ImageOps.solarize | 0.39× | — | — | — | — | — |

## Non-Performance-Critical Operations

> Metadata, I/O, analysis, drawing, and font operations. Not benchmarked for speed — 
> use CPU path timing as reference.

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.close | ⚠️ | — | — | — | — | — |
| Image.copy | 0.77× | — | — | — | — | — |
| Image.get_child_images | ⚠️ | — | — | — | — | — |
| Image.get_flattened_data | 33.86× | — | — | — | — | — |
| Image.getbands | 0.37× | — | 2.67× | — | ⚠️ | ⚠️ |
| Image.getbbox | 0.74× | — | 2.69× | — | 15.74× | 14.82× |
| Image.getchannel | 0.64× | — | — | — | — | — |
| Image.getcolors | 0.18× | — | — | — | — | — |
| Image.getdata | 0.64× | — | — | — | — | — |
| Image.getexif | ⚠️ | — | — | — | — | — |
| Image.getextrema | 0.65× | — | 2.30× | — | 8.06× | 7.96× |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | ⚠️ | — | — | — | — | — |
| Image.getpixel | 0.19× | — | 2.76× | — | ⚠️ | ⚠️ |
| Image.getprojection | 0.09× | — | — | — | — | — |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | 0.71× | — | 2.91× | — | 7.66× | 7.58× |
| Image.load | 0.69× | — | — | — | — | — |
| Image.seek | ⚠️ | — | — | — | — | — |
| Image.show | — | — | — | — | — | — |
| Image.tell | 44.18× | — | — | — | — | — |
| Image.tobytes | 14.73× | — | — | — | — | — |
| Image.verify | 0.11× | — | — | — | — | — |
| ImageColor.getcolor | 0.19× | — | — | — | — | — |
| ImageColor.getrgb | 0.37× | — | — | — | — | — |
| ImageDraw.arc | 0.95× | — | — | — | — | — |
| ImageDraw.bitmap | ⚠️ | — | — | — | — | — |
| ImageDraw.chord | 0.12× | — | — | — | — | — |
| ImageDraw.circle | 0.10× | — | — | — | — | — |
| ImageDraw.ellipse | 0.78× | — | — | — | — | — |
| ImageDraw.getfont | 21.39× | — | — | — | — | — |
| ImageDraw.line | 0.71× | — | — | — | — | — |
| ImageDraw.multiline_text | — | — | — | — | — | — |
| ImageDraw.multiline_textbbox | — | — | — | — | — | — |
| ImageDraw.pieslice | 0.10× | — | — | — | — | — |
| ImageDraw.point | — | — | — | — | — | — |
| ImageDraw.polygon | 0.15× | — | — | — | — | — |
| ImageDraw.rectangle | 0.89× | — | — | — | — | — |
| ImageDraw.regular_polygon | — | — | — | — | — | — |
| ImageDraw.rounded_rectangle | 0.21× | — | — | — | — | — |
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
| ImageFont.load | 25.95× | — | — | — | — | — |
| ImageFont.load_default | ⚠️ | — | — | — | — | — |
| ImageFont.load_default_imagefont | ⚠️ | — | — | — | — | — |
| ImageFont.load_path | — | — | — | — | — | — |
| ImageFont.truetype | ⚠️ | — | — | — | — | — |
| ImagePalette.copy | — | — | — | — | — | — |
| ImagePalette.getcolor | — | — | — | — | — | — |
| ImagePalette.getdata | — | — | — | — | — | — |
| ImagePalette.save | — | — | — | — | — | — |
| ImagePalette.tobytes | ⚠️ | — | — | — | — | — |
| ImageSequence.Iterator | — | — | — | — | — | — |
| ImageStat.Stat | 0.68× | — | — | — | — | — |

## ⚠️ Suspicious Ratios (>5× or <0.1×)

| Function | Source | Ratio |
|----------|--------|-------|
| Image.rotate | CPU | 5.10× |
| Image.rotate | WASM | 19.21× |
| Image.close | CPU | 108.86× |
| Image.draft | CPU | 185.49× |
| Image.effect_spread | CPU | 98.47× |
| Image.get_child_images | CPU | 198.59× |
| Image.get_flattened_data | CPU | 33.86× |
| Image.getexif | CPU | 56.50× |
| Image.getpalette | CPU | 1073.83× |
| Image.getprojection | CPU | 0.09× |
| Image.point | CPU | 0.09× |
| Image.putalpha | CPU | 23.97× |
| Image.putpalette | CPU | 732.90× |
| Image.putpixel | CPU | 3668.02× |
| Image.putpixel | WASM | 89.58× |
| Image.seek | CPU | 102.69× |
| Image.tell | CPU | 44.18× |
| Image.tobitmap | CPU | 0.04× |
| Image.tobytes | CPU | 14.73× |
| ImageDraw.bitmap | CPU | 0.00× |
| ... | ... | +12 more |

## PIL Parity Tests

**202 passed, 0 failed** (Pillow 12.2.0)
