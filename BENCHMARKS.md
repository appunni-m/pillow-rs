# pillow-rs Benchmarks

> Auto-generated: 2026-06-11 22:31:16 | commit `9a5e4d7` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 0 |
| Outliers flagged ⚠️ | 0 |
| Average CPU speedup vs Pillow | 0.00× |
| Native CPU benchmarks run | 0 |
| Missing (no data yet) | 166 |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.crop | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.rotate | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.transpose | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.thumbnail | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.new | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.paste | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.convert | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.filter | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.open | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.save | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.tobytes | N/A | N/A | N/A | N/A | N/A | N/A |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.crop | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.rotate | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.transpose | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.thumbnail | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.new | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.paste | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.alpha_composite | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.apply_transparency | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.close | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.convert | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.copy | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.draft | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.effect_spread | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.entropy | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.filter | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.frombytes | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.get_child_images | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.get_flattened_data | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getbands | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getbbox | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getchannel | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getcolors | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getdata | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getexif | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getextrema | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getim | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getpalette | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getpixel | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getprojection | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.getxmp | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.histogram | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.load | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.open | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.point | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.putalpha | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.putdata | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.putpalette | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.putpixel | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.quantize | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.reduce | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.remap_palette | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.save | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.seek | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.show | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.split | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.tell | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.tobitmap | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.tobytes | N/A | N/A | N/A | N/A | N/A | N/A |
| Image.transform | N/A | NYW | N/A | NYW | N/A | NYW |
| Image.verify | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.add_modulo | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.blend | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.composite | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageChops.constant | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageChops.darker | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.difference | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.duplicate | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageChops.hard_light | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.invert | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.lighter | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.logical_and | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.logical_or | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.logical_xor | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.multiply | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.offset | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageChops.overlay | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.screen | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.soft_light | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.subtract | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageChops.subtract_modulo | N/A | NYW | N/A | NYW | N/A | NYW |

### ImageColor

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageColor.getcolor | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageColor.getrgb | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageDraw

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageDraw.arc | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.bitmap | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.chord | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.circle | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.ellipse | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.getfont | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.line | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.multiline_text | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.multiline_textbbox | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.pieslice | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.point | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageDraw.polygon | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.rectangle | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.regular_polygon | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.rounded_rectangle | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.text | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.textbbox | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageDraw.textlength | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageEnhance

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageEnhance.Brightness | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageEnhance.Color | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageEnhance.Contrast | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageEnhance.Sharpness | N/A | NYW | N/A | NYW | N/A | NYW |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.BoxBlur | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.CONTOUR | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.Color3DLUT | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.DETAIL | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.EDGE_ENHANCE | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.EDGE_ENHANCE_MORE | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.EMBOSS | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.FIND_EDGES | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.GaussianBlur | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.Kernel | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.MaxFilter | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.MedianFilter | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.MinFilter | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.ModeFilter | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.RankFilter | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.SHARPEN | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.SMOOTH | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.SMOOTH_MORE | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.UnsharpMask | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageFont

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFont.FreeTypeFont | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.ImageFont | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.FreeTypeFont.getbbox | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.ImageFont.getbbox | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.FreeTypeFont.getlength | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.ImageFont.getlength | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.FreeTypeFont.getmask | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.ImageFont.getmask | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.FreeTypeFont.getmetrics | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.FreeTypeFont.getname | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.load | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.load_default | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.load_default_imagefont | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.load_path | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.truetype | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageModule

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageModule.new | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.alpha_composite | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.blend | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageModule.composite | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.effect_noise | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.eval | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.fromarray | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.frombytes | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.merge | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.open | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageOps

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageOps.crop | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageOps.autocontrast | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageOps.colorize | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageOps.contain | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.cover | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.deform | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.equalize | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageOps.exif_transpose | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.expand | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.fit | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.flip | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.grayscale | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.invert | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageOps.mirror | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.pad | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.posterize | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageOps.scale | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageOps.solarize | N/A | NYW | N/A | NYW | N/A | NYW |

### ImagePalette

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImagePalette.copy | N/A | N/A | N/A | N/A | N/A | N/A |
| ImagePalette.getcolor | N/A | N/A | N/A | N/A | N/A | N/A |
| ImagePalette.getdata | N/A | N/A | N/A | N/A | N/A | N/A |
| ImagePalette.save | N/A | N/A | N/A | N/A | N/A | N/A |
| ImagePalette.tobytes | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageSequence

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageSequence.Iterator | N/A | N/A | N/A | N/A | N/A | N/A |

### ImageStat

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageStat.Stat | N/A | N/A | N/A | N/A | N/A | N/A |
