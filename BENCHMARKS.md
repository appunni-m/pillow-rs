# pillow-rs Benchmarks

> Auto-generated: 2026-06-12 10:18:32 | commit `af775bf` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 81 |
| Outliers flagged ⚠️ | 28 |
| Average CPU speedup vs Pillow | 2.20× |
| Native CPU benchmarks run | 112 |
| Missing (no data yet) | 6 |

## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)

> Chaining 20 image operations end-to-end. Measures scheduling, coherence, and clone avoidance.

| Variant | Time (ms) | vs Pillow |
|---------|-----------|-----------|
| ST | 189.09ms | 0.31× |
| Pillow (reference) | 59.0ms | — |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | ⚠️ | — | 1.55× | — | 1.20× | 1.94× |
| Image.crop | ⚠️ | — | 3.24× | — | 3.40× | 3.72× |
| Image.rotate | ⚠️ | — | 20.48× | — | 12.79× | 13.29× |
| Image.transpose | ⚠️ | — | 2.22× | — | 2.02× | 2.03× |
| Image.thumbnail | 0.26× | — | 1.13× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.23× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.convert | ⚠️ | — | 2.09× | — | 1.74× | 1.66× |
| Image.filter | ⚠️ | — | 2.55× | — | 2.59× | 2.69× |
| Image.open | 0.62× | — | 2.01× | — | ⚠️ | ⚠️ |
| Image.save | 3.60× | — | 2.99× | — | 3.48× | 3.48× |
| Image.tobytes | 12.23× | — | — | — | — | — |

## Performance-Critical Operations

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | ⚠️ | — | 1.55× | — | 1.20× | 1.94× |
| Image.crop | ⚠️ | — | 3.24× | — | 3.40× | 3.72× |
| Image.rotate | ⚠️ | — | 20.48× | — | 12.79× | 13.29× |
| Image.transpose | ⚠️ | — | 2.22× | — | 2.02× | 2.03× |
| Image.thumbnail | 0.26× | — | 1.13× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.23× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | 0.26× | — | — | — | — | — |
| Image.convert | ⚠️ | — | 2.09× | — | 1.74× | 1.66× |
| Image.draft | ⚠️ | — | — | — | — | — |
| Image.effect_spread | ⚠️ | — | — | — | — | — |
| Image.entropy | 0.34× | — | — | — | — | — |
| Image.filter | ⚠️ | — | 2.55× | — | 2.59× | 2.69× |
| Image.frombytes | — | — | — | — | — | — |
| Image.open | 0.62× | — | 2.01× | — | ⚠️ | ⚠️ |
| Image.point | 0.09× | — | — | — | — | — |
| Image.putalpha | 21.66× | — | — | — | — | — |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | ⚠️ | — | — | — | — | — |
| Image.putpixel | ⚠️ | — | ⚠️ | — | 3.86× | 3.85× |
| Image.quantize | ⚠️ | — | — | — | — | — |
| Image.reduce | ⚠️ | — | 1.25× | — | 2.32× | 1.97× |
| Image.remap_palette | 0.29× | — | — | — | — | — |
| Image.save | 3.60× | — | 2.99× | — | 3.48× | 3.48× |
| Image.split | 0.32× | — | 1.14× | — | 9.45× | 9.38× |
| Image.tobitmap | 0.04× | — | — | — | — | — |
| Image.transform | — | — | — | — | — | — |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | 0.40× | — | — | — | — | — |
| ImageChops.add_modulo | 0.31× | — | — | — | — | — |
| ImageChops.blend | — | — | — | — | — | — |
| ImageChops.composite | — | — | — | — | — | — |
| ImageChops.constant | 0.52× | — | — | — | — | — |
| ImageChops.darker | 0.28× | — | — | — | — | — |
| ImageChops.difference | 0.29× | — | — | — | — | — |
| ImageChops.duplicate | — | — | — | — | — | — |
| ImageChops.hard_light | 0.42× | — | — | — | — | — |
| ImageChops.invert | 0.53× | — | — | — | — | — |
| ImageChops.lighter | 0.32× | — | — | — | — | — |
| ImageChops.logical_and | 0.38× | — | — | — | — | — |
| ImageChops.logical_or | 0.41× | — | — | — | — | — |
| ImageChops.logical_xor | 0.36× | — | — | — | — | — |
| ImageChops.multiply | 0.31× | — | — | — | — | — |
| ImageChops.offset | 0.78× | — | — | — | — | — |
| ImageChops.overlay | 0.40× | — | — | — | — | — |
| ImageChops.screen | 0.31× | — | — | — | — | — |
| ImageChops.soft_light | 0.43× | — | — | — | — | — |
| ImageChops.subtract | 0.37× | — | — | — | — | — |
| ImageChops.subtract_modulo | 0.31× | — | — | — | — | — |

### ImageEnhance

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageEnhance.Brightness | ⚠️ | — | 2.22× | — | 5.00× | 5.26× |
| ImageEnhance.Color | ⚠️ | — | — | — | — | — |
| ImageEnhance.Contrast | ⚠️ | — | — | — | — | — |
| ImageEnhance.Sharpness | ⚠️ | — | — | — | — | — |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | 1.94× | — | — | — | — | — |
| ImageFilter.BoxBlur | — | — | — | — | — | — |
| ImageFilter.CONTOUR | 0.97× | — | — | — | — | — |
| ImageFilter.Color3DLUT | — | — | — | — | — | — |
| ImageFilter.DETAIL | 0.92× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE | 1.00× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE_MORE | 1.20× | — | — | — | — | — |
| ImageFilter.EMBOSS | 1.05× | — | — | — | — | — |
| ImageFilter.FIND_EDGES | 1.05× | — | — | — | — | — |
| ImageFilter.GaussianBlur | — | — | 2.00× | — | 2.15× | 2.31× |
| ImageFilter.Kernel | — | — | — | — | — | — |
| ImageFilter.MaxFilter | — | — | — | — | — | — |
| ImageFilter.MedianFilter | — | — | — | — | — | — |
| ImageFilter.MinFilter | — | — | — | — | — | — |
| ImageFilter.ModeFilter | — | — | — | — | — | — |
| ImageFilter.RankFilter | — | — | — | — | — | — |
| ImageFilter.SHARPEN | 1.00× | — | — | — | — | — |
| ImageFilter.SMOOTH | 1.07× | — | — | — | — | — |
| ImageFilter.SMOOTH_MORE | 1.91× | — | — | — | — | — |
| ImageFilter.UnsharpMask | — | — | — | — | — | — |

### ImageModule

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageModule.new | — | — | — | — | — | — |
| ImageModule.alpha_composite | — | — | — | — | — | — |
| ImageModule.blend | 0.10× | — | — | — | — | — |
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
| ImageOps.colorize | 0.98× | — | — | — | — | — |
| ImageOps.contain | 0.89× | — | — | — | — | — |
| ImageOps.cover | — | — | — | — | — | — |
| ImageOps.deform | — | — | — | — | — | — |
| ImageOps.equalize | 0.12× | — | — | — | — | — |
| ImageOps.exif_transpose | — | — | — | — | — | — |
| ImageOps.expand | 0.58× | — | — | — | — | — |
| ImageOps.fit | — | — | — | — | — | — |
| ImageOps.flip | 3.79× | — | — | — | — | — |
| ImageOps.grayscale | 4.01× | — | 1.82× | — | 4.49× | 4.82× |
| ImageOps.invert | 0.59× | — | 0.30× | — | 0.94× | 0.96× |
| ImageOps.mirror | 3.88× | — | — | — | — | — |
| ImageOps.pad | 0.40× | — | — | — | — | — |
| ImageOps.posterize | 0.54× | — | — | — | — | — |
| ImageOps.scale | 0.93× | — | — | — | — | — |
| ImageOps.solarize | 0.38× | — | — | — | — | — |

## Non-Performance-Critical Operations

> Metadata, I/O, analysis, drawing, and font operations. Not benchmarked for speed — 
> use CPU path timing as reference.

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.close | ⚠️ | — | — | — | — | — |
| Image.copy | ⚠️ | — | — | — | — | — |
| Image.get_child_images | ⚠️ | — | — | — | — | — |
| Image.get_flattened_data | 33.92× | — | — | — | — | — |
| Image.getbands | 0.38× | — | 2.69× | — | ⚠️ | ⚠️ |
| Image.getbbox | 0.72× | — | 2.71× | — | 15.74× | 14.82× |
| Image.getchannel | 0.62× | — | — | — | — | — |
| Image.getcolors | 0.20× | — | — | — | — | — |
| Image.getdata | 0.68× | — | — | — | — | — |
| Image.getexif | ⚠️ | — | — | — | — | — |
| Image.getextrema | 0.69× | — | 2.43× | — | 8.06× | 7.96× |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | ⚠️ | — | — | — | — | — |
| Image.getpixel | 0.19× | — | 2.92× | — | ⚠️ | ⚠️ |
| Image.getprojection | 0.09× | — | — | — | — | — |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | 0.73× | — | 2.91× | — | 7.66× | 7.58× |
| Image.load | 0.68× | — | — | — | — | — |
| Image.seek | 39.50× | — | — | — | — | — |
| Image.show | — | — | — | — | — | — |
| Image.tell | ⚠️ | — | — | — | — | — |
| Image.tobytes | 12.23× | — | — | — | — | — |
| Image.verify | 0.13× | — | — | — | — | — |
| ImageColor.getcolor | 0.36× | — | — | — | — | — |
| ImageColor.getrgb | 0.31× | — | — | — | — | — |
| ImageDraw.arc | 0.76× | — | — | — | — | — |
| ImageDraw.bitmap | ⚠️ | — | — | — | — | — |
| ImageDraw.chord | 0.10× | — | — | — | — | — |
| ImageDraw.circle | 0.13× | — | — | — | — | — |
| ImageDraw.ellipse | 0.59× | — | — | — | — | — |
| ImageDraw.getfont | 18.26× | — | — | — | — | — |
| ImageDraw.line | 0.53× | — | — | — | — | — |
| ImageDraw.multiline_text | — | — | — | — | — | — |
| ImageDraw.multiline_textbbox | — | — | — | — | — | — |
| ImageDraw.pieslice | 0.12× | — | — | — | — | — |
| ImageDraw.point | — | — | — | — | — | — |
| ImageDraw.polygon | 0.13× | — | — | — | — | — |
| ImageDraw.rectangle | 0.71× | — | — | — | — | — |
| ImageDraw.regular_polygon | — | — | — | — | — | — |
| ImageDraw.rounded_rectangle | 0.13× | — | — | — | — | — |
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
| ImageFont.load | ⚠️ | — | — | — | — | — |
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
| ImageStat.Stat | 0.73× | — | — | — | — | — |

## ⚠️ Suspicious Ratios (>5× or <0.1×)

| Function | Source | Ratio |
|----------|--------|-------|
| Image.resize | CPU | 41501.52× |
| Image.crop | CPU | 22434.95× |
| Image.rotate | CPU | 136646.46× |
| Image.rotate | WASM | 20.48× |
| Image.transpose | CPU | 11764.41× |
| Image.close | CPU | 108.86× |
| Image.convert | CPU | 12190.48× |
| Image.copy | CPU | 25537.90× |
| Image.draft | CPU | 53.00× |
| Image.effect_spread | CPU | 96.56× |
| Image.filter | CPU | 77426.40× |
| Image.get_child_images | CPU | 198.59× |
| Image.get_flattened_data | CPU | 33.92× |
| Image.getexif | CPU | 197.76× |
| Image.getpalette | CPU | 3758.41× |
| Image.getprojection | CPU | 0.09× |
| Image.point | CPU | 0.09× |
| Image.putalpha | CPU | 21.66× |
| Image.putpalette | CPU | 806.19× |
| Image.putpixel | CPU | 4620.76× |
| ... | ... | +20 more |

## Input/Output Validation

| Metric | Value |
|--------|-------|
| PIL parity tests | **202/202 pass** |
| Output hash matches (PIL vs pillow-rs) | **0** |
| Output hash mismatches | **0** |
| Trust level | **100%** |
| Pillow version | 12.2.0 |

> Every benchmarked operation that passes PIL parity produces pixel-identical output.
> Hash mismatches indicate input/output differences that make the speedup ratio unreliable.
