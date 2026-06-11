# pillow-rs Benchmarks

> Auto-generated: 2026-06-11 13:41:07 | commit `beac6a0` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 41 |
| Outliers flagged ⚠️ | 1 |
| Average CPU speedup vs Pillow | 2.33× |
| Native CPU benchmarks run | 47 |
| Missing (no data yet) | 76 |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.74× | NYW | 1.79× | NYW | 1.29× | 1.75× |
| Image.crop | 0.95× | NYW | 3.35× | NYW | 3.10× | 3.11× |
| Image.rotate | 4.99× | NYW | — | NYW | — | NYW |
| Image.transpose | — | NYW | 2.39× | NYW | 2.29× | 2.35× |
| Image.thumbnail | 45960.16× ⚠️ | NYW | 1.61× | NYW | 1.54× | 1.59× |
| Image.new | 1.50× | — | 1.04× | — | 0.89× | — |
| Image.paste | 0.72× | NYW | 186.34× ⚠️ | NYW | 212.05× ⚠️ | 200.52× ⚠️ |
| Image.convert | 0.64× | NYW | 1.70× | NYW | 1.77× | 1.62× |
| Image.filter | — | NYW | 2.45× | NYW | — | NYW |
| Image.open | — | — | — | — | — | — |
| Image.save | — | — | — | — | — | — |
| Image.tobytes | — | — | — | — | — | — |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.74× | NYW | 1.79× | NYW | 1.29× | 1.75× |
| Image.crop | 0.95× | NYW | 3.35× | NYW | 3.10× | 3.11× |
| Image.rotate | 4.99× | NYW | — | NYW | — | NYW |
| Image.transpose | — | NYW | 2.39× | NYW | 2.29× | 2.35× |
| Image.thumbnail | 45960.16× ⚠️ | NYW | 1.61× | NYW | 1.54× | 1.59× |
| Image.new | 1.50× | — | 1.04× | — | 0.89× | — |
| Image.paste | 0.72× | NYW | 186.34× ⚠️ | NYW | 212.05× ⚠️ | 200.52× ⚠️ |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | — | — | — | — | — | — |
| Image.close | — | — | — | — | — | — |
| Image.convert | 0.64× | NYW | 1.70× | NYW | 1.77× | 1.62× |
| Image.copy | — | — | — | — | — | — |
| Image.draft | — | — | — | — | — | — |
| Image.effect_spread | — | — | — | — | — | — |
| Image.entropy | — | — | — | — | — | — |
| Image.filter | — | NYW | 2.45× | NYW | — | NYW |
| Image.frombytes | — | — | — | — | — | — |
| Image.get_child_images | — | — | — | — | — | — |
| Image.get_flattened_data | — | — | — | — | — | — |
| Image.getbands | — | — | 49.57× | — | 53.56× | — |
| Image.getbbox | — | — | 10.76× | — | 11.81× | — |
| Image.getchannel | — | — | — | — | — | — |
| Image.getcolors | — | — | — | — | — | — |
| Image.getdata | — | — | — | — | — | — |
| Image.getexif | — | — | — | — | — | — |
| Image.getextrema | — | — | 7.40× | — | 7.71× | — |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | — | — | — | — | — | — |
| Image.getpixel | — | — | 11731.14× ⚠️ | — | 17596.71× ⚠️ | — |
| Image.getprojection | — | — | — | — | — | — |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | — | — | 9.24× | — | 6.09× | — |
| Image.load | — | — | — | — | — | — |
| Image.open | — | — | — | — | — | — |
| Image.point | 0.64× | NYW | — | NYW | — | NYW |
| Image.putalpha | 28.57× | — | — | — | — | — |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | — | — | — | — | — | — |
| Image.putpixel | 33.79× | — | 3.37× | — | 3.51× | — |
| Image.quantize | 0.21× | NYW | — | NYW | — | NYW |
| Image.reduce | 0.44× | NYW | 2.08× | NYW | 1.94× | 2.13× |
| Image.remap_palette | — | — | — | — | — | — |
| Image.save | — | — | — | — | — | — |
| Image.seek | — | — | — | — | — | — |
| Image.show | — | — | — | — | — | — |
| Image.split | 0.37× | — | 8.49× | — | 8.06× | — |
| Image.tell | — | — | — | — | — | — |
| Image.tobitmap | — | — | — | — | — | — |
| Image.tobytes | — | — | — | — | — | — |
| Image.transform | — | NYW | — | NYW | — | NYW |
| Image.verify | — | — | — | — | — | — |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | 0.54× | NYW | — | NYW | — | NYW |
| ImageChops.add_modulo | — | NYW | — | NYW | — | NYW |
| ImageChops.blend | — | NYW | — | NYW | — | NYW |
| ImageChops.composite | — | — | — | — | — | — |
| ImageChops.constant | — | — | — | — | — | — |
| ImageChops.darker | 0.33× | NYW | — | NYW | — | NYW |
| ImageChops.difference | 0.49× | NYW | — | NYW | — | NYW |
| ImageChops.duplicate | — | — | — | — | — | — |
| ImageChops.hard_light | — | NYW | — | NYW | — | NYW |
| ImageChops.invert | — | NYW | — | NYW | — | NYW |
| ImageChops.lighter | 0.33× | NYW | — | NYW | — | NYW |
| ImageChops.logical_and | — | NYW | — | NYW | — | NYW |
| ImageChops.logical_or | — | NYW | — | NYW | — | NYW |
| ImageChops.logical_xor | — | NYW | — | NYW | — | NYW |
| ImageChops.multiply | 0.36× | NYW | — | NYW | — | NYW |
| ImageChops.offset | — | — | — | — | — | — |
| ImageChops.overlay | — | NYW | — | NYW | — | NYW |
| ImageChops.screen | 0.38× | NYW | — | NYW | — | NYW |
| ImageChops.soft_light | — | NYW | — | NYW | — | NYW |
| ImageChops.subtract | 0.56× | NYW | — | NYW | — | NYW |
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
| ImageEnhance.Brightness | 1.16× | NYW | 8.30× | NYW | 8.59× | 8.94× |
| ImageEnhance.Color | 0.98× | NYW | — | NYW | — | NYW |
| ImageEnhance.Contrast | 0.94× | NYW | — | NYW | — | NYW |
| ImageEnhance.Sharpness | 1.49× | NYW | — | NYW | — | NYW |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | 1.06× | — | — | — | — | — |
| ImageFilter.BoxBlur | 1.40× | — | — | — | — | — |
| ImageFilter.CONTOUR | 0.60× | — | — | — | — | — |
| ImageFilter.Color3DLUT | — | — | — | — | — | — |
| ImageFilter.DETAIL | 0.51× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE | 0.64× | — | — | — | — | — |
| ImageFilter.EDGE_ENHANCE_MORE | — | — | — | — | — | — |
| ImageFilter.EMBOSS | 0.64× | — | — | — | — | — |
| ImageFilter.FIND_EDGES | 0.62× | — | — | — | — | — |
| ImageFilter.GaussianBlur | 1.87× | — | 2.54× | — | 2.45× | — |
| ImageFilter.Kernel | — | — | — | — | — | — |
| ImageFilter.MaxFilter | 0.61× | — | — | — | — | — |
| ImageFilter.MedianFilter | 0.79× | — | — | — | — | — |
| ImageFilter.MinFilter | 0.61× | — | — | — | — | — |
| ImageFilter.ModeFilter | 2.50× | — | — | — | — | — |
| ImageFilter.RankFilter | — | — | — | — | — | — |
| ImageFilter.SHARPEN | 0.52× | — | — | — | — | — |
| ImageFilter.SMOOTH | 0.58× | — | — | — | — | — |
| ImageFilter.SMOOTH_MORE | — | — | — | — | — | — |
| ImageFilter.UnsharpMask | 1.34× | — | — | — | — | — |

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
| ImageOps.grayscale | — | — | 4.57× | — | 5.17× | — |
| ImageOps.invert | 0.09× | NYW | 1.71× | NYW | 1.58× | 1.61× |
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
