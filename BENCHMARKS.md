# pillow-rs Benchmarks

> Auto-generated: 2026-06-12 07:46:01 | commit `e7f3e71` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 32 |
| Outliers flagged ⚠️ | 0 |
| Average CPU speedup vs Pillow | 1.95× |
| Native CPU benchmarks run | 32 |
| Missing (no data yet) | 9 |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.50× | 0.50× | 1.41× | 0.50× | 0.50× | 0.50× |
| Image.crop | 0.94× | 0.94× | 3.78× | 0.94× | 0.94× | 0.94× |
| Image.rotate | 4.97× | 4.97× | 18.86× | 4.97× | 4.97× | 4.97× |
| Image.transpose | 0.70× | 0.70× | 1.80× | 0.70× | 0.70× | 0.70× |
| Image.thumbnail | — | — | 1.05× | — | — | — |
| Image.new | — | — | 0.23× | — | — | — |
| Image.paste | — | — | — | — | — | — |
| Image.convert | 0.67× | 0.67× | 2.14× | 0.67× | 0.67× | 0.67× |
| Image.filter | 1.07× | 1.07× | 2.27× | 1.07× | 1.07× | 1.07× |
| Image.open | — | — | 1.86× | — | — | — |
| Image.save | — | — | 2.85× | — | — | — |
| Image.tobytes | 4.89× | 4.89× | 4.89× | 4.89× | 4.89× | 4.89× |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.50× | 0.50× | 1.41× | 0.50× | 0.50× | 0.50× |
| Image.crop | 0.94× | 0.94× | 3.78× | 0.94× | 0.94× | 0.94× |
| Image.rotate | 4.97× | 4.97× | 18.86× | 4.97× | 4.97× | 4.97× |
| Image.transpose | 0.70× | 0.70× | 1.80× | 0.70× | 0.70× | 0.70× |
| Image.thumbnail | — | — | 1.05× | — | — | — |
| Image.new | — | — | 0.23× | — | — | — |
| Image.paste | — | — | — | — | — | — |
| Image.alpha_composite | — | — | — | — | — | — |
| Image.apply_transparency | — | — | — | — | — | — |
| Image.close | — | — | — | — | — | — |
| Image.convert | 0.67× | 0.67× | 2.14× | 0.67× | 0.67× | 0.67× |
| Image.copy | 0.73× | 0.73× | 0.73× | 0.73× | 0.73× | 0.73× |
| Image.draft | — | — | — | — | — | — |
| Image.effect_spread | 23.73× | 23.73× | 23.73× | 23.73× | 23.73× | 23.73× |
| Image.entropy | 0.33× | 0.33× | 0.33× | 0.33× | 0.33× | 0.33× |
| Image.filter | 1.07× | 1.07× | 2.27× | 1.07× | 1.07× | 1.07× |
| Image.frombytes | — | — | — | — | — | — |
| Image.get_child_images | — | — | — | — | — | — |
| Image.get_flattened_data | — | — | — | — | — | — |
| Image.getbands | 0.38× | 0.38× | 2.46× | 0.38× | 0.38× | 0.38× |
| Image.getbbox | 0.74× | 0.74× | 2.65× | 0.74× | 0.74× | 0.74× |
| Image.getchannel | 0.63× | 0.63× | 0.63× | 0.63× | 0.63× | 0.63× |
| Image.getcolors | 0.18× | 0.18× | 0.18× | 0.18× | 0.18× | 0.18× |
| Image.getdata | — | — | — | — | — | — |
| Image.getexif | — | — | — | — | — | — |
| Image.getextrema | 0.67× | 0.67× | 2.05× | 0.67× | 0.67× | 0.67× |
| Image.getim | — | — | — | — | — | — |
| Image.getpalette | — | — | — | — | — | — |
| Image.getpixel | 0.20× | 0.20× | 2.75× | 0.20× | 0.20× | 0.20× |
| Image.getprojection | 0.08× | 0.08× | 0.08× | 0.08× | 0.08× | 0.08× |
| Image.getxmp | — | — | — | — | — | — |
| Image.histogram | 0.64× | 0.64× | 2.56× | 0.64× | 0.64× | 0.64× |
| Image.load | — | — | — | — | — | — |
| Image.open | — | — | 1.86× | — | — | — |
| Image.point | 0.08× | 0.08× | 0.08× | 0.08× | 0.08× | 0.08× |
| Image.putalpha | 3.32× | 3.32× | 3.32× | 3.32× | 3.32× | 3.32× |
| Image.putdata | — | — | — | — | — | — |
| Image.putpalette | — | — | — | — | — | — |
| Image.putpixel | 0.74× | 0.74× | 70.74× | 0.74× | 0.74× | 0.74× |
| Image.quantize | 2.25× | 2.25× | 2.25× | 2.25× | 2.25× | 2.25× |
| Image.reduce | 0.32× | 0.32× | 1.25× | 0.32× | 0.32× | 0.32× |
| Image.remap_palette | — | — | — | — | — | — |
| Image.save | — | — | 2.85× | — | — | — |
| Image.seek | — | — | — | — | — | — |
| Image.show | — | — | — | — | — | — |
| Image.split | 0.29× | 0.29× | 1.12× | 0.29× | 0.29× | 0.29× |
| Image.tell | — | — | — | — | — | — |
| Image.tobitmap | — | — | — | — | — | — |
| Image.tobytes | 4.89× | 4.89× | 4.89× | 4.89× | 4.89× | 4.89× |
| Image.transform | — | — | — | — | — | — |
| Image.verify | — | — | — | — | — | — |

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
| ImageEnhance.Brightness | — | — | 2.38× | — | — | — |
| ImageEnhance.Color | — | — | — | — | — | — |
| ImageEnhance.Contrast | — | — | — | — | — | — |
| ImageEnhance.Sharpness | — | — | — | — | — | — |

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
| ImageFilter.GaussianBlur | — | — | 1.94× | — | — | — |
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
| ImageOps.crop | — | — | — | — | — | — |
| ImageOps.autocontrast | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× | 0.11× |
| ImageOps.colorize | — | — | — | — | — | — |
| ImageOps.contain | — | — | — | — | — | — |
| ImageOps.cover | — | — | — | — | — | — |
| ImageOps.deform | — | — | — | — | — | — |
| ImageOps.equalize | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× | 0.12× |
| ImageOps.exif_transpose | — | — | — | — | — | — |
| ImageOps.expand | — | — | — | — | — | — |
| ImageOps.fit | — | — | — | — | — | — |
| ImageOps.flip | 4.27× | 4.27× | 4.27× | 4.27× | 4.27× | 4.27× |
| ImageOps.grayscale | 3.72× | 3.72× | 2.28× | 3.72× | 3.72× | 3.72× |
| ImageOps.invert | 0.57× | 0.57× | 0.31× | 0.57× | 0.57× | 0.57× |
| ImageOps.mirror | 3.74× | 3.74× | 3.74× | 3.74× | 3.74× | 3.74× |
| ImageOps.pad | — | — | — | — | — | — |
| ImageOps.posterize | 0.51× | 0.51× | 0.51× | 0.51× | 0.51× | 0.51× |
| ImageOps.scale | — | — | — | — | — | — |
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
