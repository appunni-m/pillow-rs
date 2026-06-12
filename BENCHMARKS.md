# pillow-rs Benchmarks

> Auto-generated: 2026-06-12 10:01:07 | commit `82d640d` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 81 |
| Outliers flagged ⚠️ | 12 |
| Average CPU speedup vs Pillow | 4.16× |
| Native CPU benchmarks run | 94 |
| Missing (no data yet) | 8 |

## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)

> Chaining 20 image operations end-to-end. Measures scheduling, coherence, and clone avoidance.

| Variant | Time (ms) | vs Pillow |
|---------|-----------|-----------|
| ST | 174.31ms | 0.34× |
| Pillow (reference) | 59.0ms | — |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.42× | 0.42× | 1.44× | 0.42× | 1.20× | 1.94× |
| Image.crop | 0.97× | 0.97× | 3.37× | 0.97× | 3.40× | 3.72× |
| Image.rotate | 5.21× | 5.21× | 20.01× | 5.21× | 12.79× | 13.29× |
| Image.transpose | 0.73× | 0.73× | 2.12× | 0.73× | 2.02× | 2.03× |
| Image.thumbnail | — | — | 1.08× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.20× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.convert | 0.68× | 0.68× | 2.23× | 0.68× | 1.74× | 1.66× |
| Image.filter | 1.04× | 1.04× | 2.54× | 1.04× | 2.59× | 2.69× |
| Image.open | 0.51× | 0.51× | 1.80× | 0.51× | 309.90× ⚠️ | 328.40× ⚠️ |
| Image.save | 3.04× | 3.04× | 2.97× | 3.04× | 3.48× | 3.48× |
| Image.tobytes | 15.80× | 15.80× | 15.80× | 15.80× | 15.80× | 15.80× |

## Performance-Critical Operations

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.42× | 0.42× | 1.44× | 0.42× | 1.20× | 1.94× |
| Image.crop | 0.97× | 0.97× | 3.37× | 0.97× | 3.40× | 3.72× |
| Image.rotate | 5.21× | 5.21× | 20.01× | 5.21× | 12.79× | 13.29× |
| Image.transpose | 0.73× | 0.73× | 2.12× | 0.73× | 2.02× | 2.03× |
| Image.thumbnail | — | — | 1.08× | — | 1.63× | 1.55× |
| Image.new | — | — | 0.20× | — | 0.25× | 0.25× |
| Image.paste | — | — | — | — | 0.35× | 0.37× |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | 0.24× | 0.24× | 0.24× | 0.24× | 0.24× | 0.24× |
| Image.convert | 0.68× | 0.68× | 2.23× | 0.68× | 1.74× | 1.66× |
| Image.draft | 48.39× | 48.39× | 48.39× | 48.39× | 48.39× | 48.39× |
| Image.effect_spread | 95.03× | 95.03× | 95.03× | 95.03× | 95.03× | 95.03× |
| Image.entropy | 0.38× | 0.38× | 0.38× | 0.38× | 0.38× | 0.38× |
| Image.filter | 1.04× | 1.04× | 2.54× | 1.04× | 2.59× | 2.69× |
| Image.frombytes | — | — | — | — | — | — |
| Image.open | 0.51× | 0.51× | 1.80× | 0.51× | 309.90× ⚠️ | 328.40× ⚠️ |
| Image.point | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× |
| Image.putalpha | 33.69× | 33.69× | 33.69× | 33.69× | 33.69× | 33.69× |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | 711.35× ⚠️ | 711.35× ⚠️ | 711.35× ⚠️ | 711.35× ⚠️ | 711.35× ⚠️ | 711.35× ⚠️ |
| Image.putpixel | 4808.09× ⚠️ | 4808.09× ⚠️ | 63.42× | 4808.09× ⚠️ | 3.86× | 3.85× |
| Image.quantize | 2.08× | 2.08× | 2.08× | 2.08× | 2.08× | 2.08× |
| Image.reduce | 0.42× | 0.42× | 1.30× | 0.42× | 2.32× | 1.97× |
| Image.remap_palette | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× |
| Image.save | 3.04× | 3.04× | 2.97× | 3.04× | 3.48× | 3.48× |
| Image.split | 0.31× | 0.31× | 0.91× | 0.31× | 9.45× | 9.38× |
| Image.tobitmap | 0.04× | 0.04× | 0.04× | 0.04× | 0.04× | 0.04× |
| Image.transform | — | — | — | — | — | — |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | 0.39× | 0.39× | 0.39× | 0.39× | 0.39× | 0.39× |
| ImageChops.add_modulo | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× |
| ImageChops.blend | — | — | — | — | — | — |
| ImageChops.composite | — | — | — | — | — | — |
| ImageChops.constant | 0.49× | 0.49× | 0.49× | 0.49× | 0.49× | 0.49× |
| ImageChops.darker | 0.33× | 0.33× | 0.33× | 0.33× | 0.33× | 0.33× |
| ImageChops.difference | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× |
| ImageChops.duplicate | — | — | — | — | — | — |
| ImageChops.hard_light | 0.38× | 0.38× | 0.38× | 0.38× | 0.38× | 0.38× |
| ImageChops.invert | 0.54× | 0.54× | 0.54× | 0.54× | 0.54× | 0.54× |
| ImageChops.lighter | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× |
| ImageChops.logical_and | 0.38× | 0.38× | 0.38× | 0.38× | 0.38× | 0.38× |
| ImageChops.logical_or | 0.41× | 0.41× | 0.41× | 0.41× | 0.41× | 0.41× |
| ImageChops.logical_xor | 0.37× | 0.37× | 0.37× | 0.37× | 0.37× | 0.37× |
| ImageChops.multiply | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× | 0.29× |
| ImageChops.offset | 0.72× | 0.72× | 0.72× | 0.72× | 0.72× | 0.72× |
| ImageChops.overlay | 0.37× | 0.37× | 0.37× | 0.37× | 0.37× | 0.37× |
| ImageChops.screen | 0.35× | 0.35× | 0.35× | 0.35× | 0.35× | 0.35× |
| ImageChops.soft_light | 0.44× | 0.44× | 0.44× | 0.44× | 0.44× | 0.44× |
| ImageChops.subtract | 0.39× | 0.39× | 0.39× | 0.39× | 0.39× | 0.39× |
| ImageChops.subtract_modulo | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× | 0.31× |

### ImageEnhance

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageEnhance.Brightness | 25242.35× ⚠️ | 25242.35× ⚠️ | 2.32× | 25242.35× ⚠️ | 5.00× | 5.26× |
| ImageEnhance.Color | 21854.57× ⚠️ | 21854.57× ⚠️ | 21854.57× ⚠️ | 21854.57× ⚠️ | 21854.57× ⚠️ | 21854.57× ⚠️ |
| ImageEnhance.Contrast | 27190.17× ⚠️ | 27190.17× ⚠️ | 27190.17× ⚠️ | 27190.17× ⚠️ | 27190.17× ⚠️ | 27190.17× ⚠️ |
| ImageEnhance.Sharpness | 53243.83× ⚠️ | 53243.83× ⚠️ | 53243.83× ⚠️ | 53243.83× ⚠️ | 53243.83× ⚠️ | 53243.83× ⚠️ |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | 2.02× | 2.02× | 2.02× | 2.02× | 2.02× | 2.02× |
| ImageFilter.BoxBlur | — | — | — | — | — | — |
| ImageFilter.CONTOUR | 1.16× | 1.16× | 1.16× | 1.16× | 1.16× | 1.16× |
| ImageFilter.Color3DLUT | — | — | — | — | — | — |
| ImageFilter.DETAIL | 1.11× | 1.11× | 1.11× | 1.11× | 1.11× | 1.11× |
| ImageFilter.EDGE_ENHANCE | 1.18× | 1.18× | 1.18× | 1.18× | 1.18× | 1.18× |
| ImageFilter.EDGE_ENHANCE_MORE | 1.26× | 1.26× | 1.26× | 1.26× | 1.26× | 1.26× |
| ImageFilter.EMBOSS | 1.16× | 1.16× | 1.16× | 1.16× | 1.16× | 1.16× |
| ImageFilter.FIND_EDGES | 1.20× | 1.20× | 1.20× | 1.20× | 1.20× | 1.20× |
| ImageFilter.GaussianBlur | — | — | 1.87× | — | 2.15× | 2.31× |
| ImageFilter.Kernel | — | — | — | — | — | — |
| ImageFilter.MaxFilter | — | — | — | — | — | — |
| ImageFilter.MedianFilter | — | — | — | — | — | — |
| ImageFilter.MinFilter | — | — | — | — | — | — |
| ImageFilter.ModeFilter | — | — | — | — | — | — |
| ImageFilter.RankFilter | — | — | — | — | — | — |
| ImageFilter.SHARPEN | 1.11× | 1.11× | 1.11× | 1.11× | 1.11× | 1.11× |
| ImageFilter.SMOOTH | 1.15× | 1.15× | 1.15× | 1.15× | 1.15× | 1.15× |
| ImageFilter.SMOOTH_MORE | 1.86× | 1.86× | 1.86× | 1.86× | 1.86× | 1.86× |
| ImageFilter.UnsharpMask | — | — | — | — | — | — |

### ImageModule

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageModule.new | — | — | — | — | — | — |
| ImageModule.alpha_composite | — | — | — | — | — | — |
| ImageModule.blend | 0.10× | 0.10× | 0.10× | 0.10× | 0.10× | 0.10× |
| ImageModule.composite | 0.06× | 0.06× | 0.06× | 0.06× | 0.06× | 0.06× |
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
| ImageOps.colorize | 0.93× | 0.93× | 0.93× | 0.93× | 0.93× | 0.93× |
| ImageOps.contain | 0.91× | 0.91× | 0.91× | 0.91× | 0.91× | 0.91× |
| ImageOps.cover | — | — | — | — | — | — |
| ImageOps.deform | — | — | — | — | — | — |
| ImageOps.equalize | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× |
| ImageOps.exif_transpose | — | — | — | — | — | — |
| ImageOps.expand | 0.59× | 0.59× | 0.59× | 0.59× | 0.59× | 0.59× |
| ImageOps.fit | — | — | — | — | — | — |
| ImageOps.flip | 4.13× | 4.13× | 4.13× | 4.13× | 4.13× | 4.13× |
| ImageOps.grayscale | 3.73× | 3.73× | 2.22× | 3.73× | 4.49× | 4.82× |
| ImageOps.invert | 0.52× | 0.52× | 0.31× | 0.52× | 0.94× | 0.96× |
| ImageOps.mirror | 3.96× | 3.96× | 3.96× | 3.96× | 3.96× | 3.96× |
| ImageOps.pad | 0.46× | 0.46× | 0.46× | 0.46× | 0.46× | 0.46× |
| ImageOps.posterize | 0.56× | 0.56× | 0.56× | 0.56× | 0.56× | 0.56× |
| ImageOps.scale | 0.91× | 0.91× | 0.91× | 0.91× | 0.91× | 0.91× |
| ImageOps.solarize | 0.36× | 0.36× | 0.36× | 0.36× | 0.36× | 0.36× |

## Non-Performance-Critical Operations

> Metadata, I/O, analysis, drawing, and font operations. Not benchmarked for speed — 
> use CPU path timing as reference.

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.close | 136.07× ⚠️ | 136.07× ⚠️ | 136.07× ⚠️ | 136.07× ⚠️ | 136.07× ⚠️ | 136.07× ⚠️ |
| Image.copy | 0.77× | 0.77× | 0.77× | 0.77× | 0.77× | 0.77× |
| Image.get_child_images | 47.66× | 47.66× | 47.66× | 47.66× | 47.66× | 47.66× |
| Image.get_flattened_data | 34.36× | 34.36× | 34.36× | 34.36× | 34.36× | 34.36× |
| Image.getbands | 0.38× | 0.38× | 2.65× | 0.38× | 71.37× | 62.76× |
| Image.getbbox | 0.72× | 0.72× | 2.33× | 0.72× | 15.74× | 14.82× |
| Image.getchannel | 0.65× | 0.65× | 0.65× | 0.65× | 0.65× | 0.65× |
| Image.getcolors | 0.19× | 0.19× | 0.19× | 0.19× | 0.19× | 0.19× |
| Image.getdata | 0.67× | 0.67× | 0.67× | 0.67× | 0.67× | 0.67× |
| Image.getexif | 197.76× ⚠️ | 197.76× ⚠️ | 197.76× ⚠️ | 197.76× ⚠️ | 197.76× ⚠️ | 197.76× ⚠️ |
| Image.getextrema | 0.69× | 0.69× | 2.39× | 0.69× | 8.06× | 7.96× |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | 2505.61× ⚠️ | 2505.61× ⚠️ | 2505.61× ⚠️ | 2505.61× ⚠️ | 2505.61× ⚠️ | 2505.61× ⚠️ |
| Image.getpixel | 0.20× | 0.20× | 2.35× | 0.20× | 25114.13× ⚠️ | 22159.53× ⚠️ |
| Image.getprojection | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× | 0.09× |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | 0.76× | 0.76× | 2.71× | 0.76× | 7.66× | 7.58× |
| Image.load | 0.71× | 0.71× | 0.71× | 0.71× | 0.71× | 0.71× |
| Image.seek | 114.10× ⚠️ | 114.10× ⚠️ | 114.10× ⚠️ | 114.10× ⚠️ | 114.10× ⚠️ | 114.10× ⚠️ |
| Image.show | — | — | — | — | — | — |
| Image.tell | 145.16× ⚠️ | 145.16× ⚠️ | 145.16× ⚠️ | 145.16× ⚠️ | 145.16× ⚠️ | 145.16× ⚠️ |
| Image.tobytes | 15.80× | 15.80× | 15.80× | 15.80× | 15.80× | 15.80× |
| Image.verify | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× |
| ImageColor.getcolor | 0.42× | 0.42× | 0.42× | 0.42× | 0.42× | 0.42× |
| ImageColor.getrgb | 0.34× | 0.34× | 0.34× | 0.34× | 0.34× | 0.34× |
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
| ImagePalette.copy | — | — | — | — | — | — |
| ImagePalette.getcolor | — | — | — | — | — | — |
| ImagePalette.getdata | — | — | — | — | — | — |
| ImagePalette.save | — | — | — | — | — | — |
| ImagePalette.tobytes | 0.00× ⚠️ | 0.00× ⚠️ | 0.00× ⚠️ | 0.00× ⚠️ | 0.00× ⚠️ | 0.00× ⚠️ |
| ImageSequence.Iterator | — | — | — | — | — | — |
| ImageStat.Stat | 0.71× | 0.71× | 0.71× | 0.71× | 0.71× | 0.71× |

## ⚠️ Suspicious Ratios (>5× or <0.1×)

| Function | Source | Ratio |
|----------|--------|-------|
| Image.rotate | CPU | 5.21× |
| Image.rotate | WASM | 20.01× |
| Image.close | CPU | 136.07× |
| Image.draft | CPU | 48.39× |
| Image.effect_spread | CPU | 95.03× |
| Image.get_child_images | CPU | 47.66× |
| Image.get_flattened_data | CPU | 34.36× |
| Image.getexif | CPU | 197.76× |
| Image.getpalette | CPU | 2505.61× |
| Image.getprojection | CPU | 0.09× |
| Image.point | CPU | 0.09× |
| Image.putalpha | CPU | 33.69× |
| Image.putpalette | CPU | 711.35× |
| Image.putpixel | CPU | 4808.09× |
| Image.putpixel | WASM | 63.42× |
| Image.seek | CPU | 114.10× |
| Image.tell | CPU | 145.16× |
| Image.tobitmap | CPU | 0.04× |
| Image.tobytes | CPU | 15.80× |
| ImageEnhance.Brightness | CPU | 25242.35× |
| ... | ... | +5 more |

## PIL Parity Tests

**202 passed, 0 failed** (Pillow 12.2.0)
