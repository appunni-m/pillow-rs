# pillow-rs Benchmarks

> Auto-generated: 2026-06-11 16:00:48 | commit `cbcb9a7` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 44 |
| Outliers flagged ⚠️ | 2 |
| Average CPU speedup vs Pillow | 3.36× |
| Native CPU benchmarks run | 47 |
| Missing (no data yet) | 6 |

## Pipeline Benchmark — 20 Operations (Single- vs Multi-Thread)

> Chaining 20 image operations end-to-end. Measures scheduling, coherence, and clone avoidance.

| Variant | Time (ms) | vs Pillow |
|---------|-----------|-----------|
| MT | 154.17ms | 0.38× |
| ST | 289.51ms | 0.20× |
| **MT Speedup** | **1.88×** | |
| Pillow (reference) | 59.0ms | — |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.73× | NYW | 2.01× | NYW | 1.29× | 1.75× |
| Image.crop | 0.91× | NYW | 3.39× | NYW | 3.10× | 3.11× |
| Image.rotate | 4.77× | NYW | 15.21× | NYW | — | NYW |
| Image.transpose | — | NYW | 2.61× | NYW | 2.29× | 2.35× |
| Image.thumbnail | 41364.15× ⚠️ | NYW | 1.82× | NYW | 1.54× | 1.59× |
| Image.new | 1.26× | — | 1.35× | — | 0.89× | — |
| Image.paste | 0.69× | NYW | 0.36× | NYW | 212.05× ⚠️ | 200.52× ⚠️ |
| Image.convert | 0.62× | NYW | 1.84× | NYW | 1.77× | 1.62× |
| Image.filter | — | NYW | 2.80× | NYW | — | NYW |
| Image.open | 353322.82× ⚠️ | — | 397.89× ⚠️ | — | 349.82× ⚠️ | — |
| Image.save | 4.82× | — | 3.41× | — | 3.26× | — |
| Image.tobytes | 48.55× | — | — | — | — | — |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.73× | NYW | 2.01× | NYW | 1.29× | 1.75× |
| Image.crop | 0.91× | NYW | 3.39× | NYW | 3.10× | 3.11× |
| Image.rotate | 4.77× | NYW | 15.21× | NYW | — | NYW |
| Image.transpose | — | NYW | 2.61× | NYW | 2.29× | 2.35× |
| Image.thumbnail | 41364.15× ⚠️ | NYW | 1.82× | NYW | 1.54× | 1.59× |
| Image.new | 1.26× | — | 1.35× | — | 0.89× | — |
| Image.paste | 0.69× | NYW | 0.36× | NYW | 212.05× ⚠️ | 200.52× ⚠️ |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | — | — | — | — | — | — |
| Image.close | — | — | — | — | — | — |
| Image.convert | 0.62× | NYW | 1.84× | NYW | 1.77× | 1.62× |
| Image.copy | — | — | — | — | — | — |
| Image.draft | — | — | — | — | — | — |
| Image.effect_spread | — | — | — | — | — | — |
| Image.entropy | — | — | — | — | — | — |
| Image.filter | — | NYW | 2.80× | NYW | — | NYW |
| Image.frombytes | 0.17× | — | — | — | — | — |
| Image.get_child_images | — | — | — | — | — | — |
| Image.get_flattened_data | — | — | — | — | — | — |
| Image.getbands | — | — | 71.88× | — | 53.56× | — |
| Image.getbbox | — | — | 13.29× | — | 11.81× | — |
| Image.getchannel | — | — | — | — | — | — |
| Image.getcolors | — | — | — | — | — | — |
| Image.getdata | — | — | — | — | — | — |
| Image.getexif | — | — | — | — | — | — |
| Image.getextrema | — | — | 8.29× | — | 7.71× | — |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | — | — | — | — | — | — |
| Image.getpixel | — | — | 16758.77× ⚠️ | — | 17596.71× ⚠️ | — |
| Image.getprojection | — | — | — | — | — | — |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | — | — | 7.19× | — | 6.09× | — |
| Image.load | — | — | — | — | — | — |
| Image.open | 353322.82× ⚠️ | — | 397.89× ⚠️ | — | 349.82× ⚠️ | — |
| Image.point | 0.57× | NYW | — | NYW | — | NYW |
| Image.putalpha | 25.24× | — | — | — | — | — |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | — | — | — | — | — | — |
| Image.putpixel | 31.76× | — | 3.88× | — | 3.51× | — |
| Image.quantize | 0.20× | NYW | — | NYW | — | NYW |
| Image.reduce | 0.41× | NYW | 2.28× | NYW | 1.94× | 2.13× |
| Image.remap_palette | — | — | — | — | — | — |
| Image.save | 4.82× | — | 3.41× | — | 3.26× | — |
| Image.seek | — | — | — | — | — | — |
| Image.show | — | — | — | — | — | — |
| Image.split | 0.30× | — | 9.11× | — | 8.06× | — |
| Image.tell | — | — | — | — | — | — |
| Image.tobitmap | — | — | — | — | — | — |
| Image.tobytes | 48.55× | — | — | — | — | — |
| Image.transform | — | NYW | — | NYW | — | NYW |
| Image.verify | — | — | — | — | — | — |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | 0.64× | NYW | — | NYW | — | NYW |
| ImageChops.add_modulo | — | NYW | — | NYW | — | NYW |
| ImageChops.blend | — | NYW | — | NYW | — | NYW |
| ImageChops.composite | — | — | — | — | — | — |
| ImageChops.constant | — | — | — | — | — | — |
| ImageChops.darker | 0.33× | NYW | — | NYW | — | NYW |
| ImageChops.difference | 0.48× | NYW | — | NYW | — | NYW |
| ImageChops.duplicate | — | — | — | — | — | — |
| ImageChops.hard_light | — | NYW | — | NYW | — | NYW |
| ImageChops.invert | — | NYW | — | NYW | — | NYW |
| ImageChops.lighter | 0.34× | NYW | — | NYW | — | NYW |
| ImageChops.logical_and | — | NYW | — | NYW | — | NYW |
| ImageChops.logical_or | — | NYW | — | NYW | — | NYW |
| ImageChops.logical_xor | — | NYW | — | NYW | — | NYW |
| ImageChops.multiply | 0.48× | NYW | — | NYW | — | NYW |
| ImageChops.offset | — | — | — | — | — | — |
| ImageChops.overlay | — | NYW | — | NYW | — | NYW |
| ImageChops.screen | 0.39× | NYW | — | NYW | — | NYW |
| ImageChops.soft_light | — | NYW | — | NYW | — | NYW |
| ImageChops.subtract | 0.63× | NYW | — | NYW | — | NYW |
| ImageChops.subtract_modulo | — | NYW | — | NYW | — | NYW |

### ImageColor

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageColor.getcolor | — | — | — | — | — | — |
| ImageColor.getrgb | — | — | — | — | — | — |

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
| ImageDraw.point | — | NYW | — | NYW | — | NYW |
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
| ImageEnhance.Brightness | 1.06× | NYW | 7.64× | NYW | 8.59× | 8.94× |
| ImageEnhance.Color | 1.07× | NYW | — | NYW | — | NYW |
| ImageEnhance.Contrast | 0.88× | NYW | — | NYW | — | NYW |
| ImageEnhance.Sharpness | 1.63× | NYW | — | NYW | — | NYW |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | 2.09× | — | — | — | — | — |
| ImageFilter.BoxBlur | 1.33× | — | — | — | — | — |
| ImageFilter.CONTOUR | 1.18× | — | — | — | — | — |
| ImageFilter.Color3DLUT | — | — | — | — | — | — |
| ImageFilter.DETAIL | 1.10× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE | 1.24× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE_MORE | — | — | — | — | — | — |
| ImageFilter.EMBOSS | 1.20× | — | — | — | — | — |
| ImageFilter.FIND_EDGES | 1.17× | — | — | — | — | — |
| ImageFilter.GaussianBlur | 1.74× | — | 2.65× | — | 2.45× | — |
| ImageFilter.Kernel | — | — | — | — | — | — |
| ImageFilter.MaxFilter | 0.59× | — | — | — | — | — |
| ImageFilter.MedianFilter | 0.77× | — | — | — | — | — |
| ImageFilter.MinFilter | 0.59× | — | — | — | — | — |
| ImageFilter.ModeFilter | 2.34× | — | — | — | — | — |
| ImageFilter.RankFilter | — | — | — | — | — | — |
| ImageFilter.SHARPEN | 1.11× | — | — | — | — | — |
| ImageFilter.SMOOTH | 1.12× | — | — | — | — | — |
| ImageFilter.SMOOTH_MORE | — | — | — | — | — | — |
| ImageFilter.UnsharpMask | 1.30× | — | — | — | — | — |

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
| ImageModule.blend | — | NYW | — | NYW | — | NYW |
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
| ImageOps.crop | — | NYW | — | NYW | — | NYW |
| ImageOps.autocontrast | 0.05× | NYW | — | NYW | — | NYW |
| ImageOps.colorize | — | NYW | — | NYW | — | NYW |
| ImageOps.contain | — | — | — | — | — | — |
| ImageOps.cover | — | — | — | — | — | — |
| ImageOps.deform | — | — | — | — | — | — |
| ImageOps.equalize | 0.06× | NYW | — | NYW | — | NYW |
| ImageOps.exif_transpose | — | — | — | — | — | — |
| ImageOps.expand | — | — | — | — | — | — |
| ImageOps.fit | — | — | — | — | — | — |
| ImageOps.flip | — | — | — | — | — | — |
| ImageOps.grayscale | — | — | 5.56× | — | 5.17× | — |
| ImageOps.invert | 0.08× | NYW | 2.04× | NYW | 1.58× | 1.61× |
| ImageOps.mirror | — | — | — | — | — | — |
| ImageOps.pad | — | — | — | — | — | — |
| ImageOps.posterize | — | NYW | — | NYW | — | NYW |
| ImageOps.scale | — | — | — | — | — | — |
| ImageOps.solarize | — | NYW | — | NYW | — | NYW |

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
