# pillow-rs Benchmarks

> Auto-generated: 2026-06-12 07:32:11 | commit `7308da1` | 166 functions | 6 targets

## Summary

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with GPU path | 42 |
| Valid CPU speedups (excl. outliers) | 32 |
| Outliers flagged ⚠️ | 0 |
| Average CPU speedup vs Pillow | 1.59× |
| Native CPU benchmarks run | 32 |
| Missing (no data yet) | 9 |

## Priority Operations (Tier 1)

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.42× | NYW | 9316.67× ⚠️ | NYW | TBD | NYW |
| Image.crop | 0.70× | NYW | 10501.46× ⚠️ | NYW | TBD | NYW |
| Image.rotate | 4.35× | NYW | 95238.44× ⚠️ | NYW | TBD | NYW |
| Image.transpose | 0.61× | NYW | 5626.46× ⚠️ | NYW | TBD | NYW |
| Image.thumbnail | TBD | NYW | 1797.17× ⚠️ | NYW | TBD | NYW |
| Image.new | TBD | N/A | 1.05× | N/A | TBD | N/A |
| Image.paste | TBD | NYW | 20.02× | NYW | TBD | NYW |
| Image.convert | 0.54× | NYW | 11082.26× ⚠️ | NYW | TBD | NYW |
| Image.filter | 0.73× | NYW | 54198.48× ⚠️ | NYW | TBD | NYW |
| Image.open | TBD | N/A | 324.79× ⚠️ | N/A | TBD | N/A |
| Image.save | TBD | N/A | 2.55× | N/A | TBD | N/A |
| Image.tobytes | 3.91× | N/A | TBD | N/A | TBD | N/A |

## All Functions

### Image

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| Image.resize | 0.42× | NYW | 9316.67× ⚠️ | NYW | TBD | NYW |
| Image.crop | 0.70× | NYW | 10501.46× ⚠️ | NYW | TBD | NYW |
| Image.rotate | 4.35× | NYW | 95238.44× ⚠️ | NYW | TBD | NYW |
| Image.transpose | 0.61× | NYW | 5626.46× ⚠️ | NYW | TBD | NYW |
| Image.thumbnail | TBD | NYW | 1797.17× ⚠️ | NYW | TBD | NYW |
| Image.new | TBD | N/A | 1.05× | N/A | TBD | N/A |
| Image.paste | TBD | NYW | 20.02× | NYW | TBD | NYW |
| Image.alpha_composite | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.apply_transparency | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.close | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.convert | 0.54× | NYW | 11082.26× ⚠️ | NYW | TBD | NYW |
| Image.copy | 0.69× | N/A | TBD | N/A | TBD | N/A |
| Image.draft | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.effect_spread | 17.28× | N/A | TBD | N/A | TBD | N/A |
| Image.entropy | 0.20× | N/A | TBD | N/A | TBD | N/A |
| Image.filter | 0.73× | NYW | 54198.48× ⚠️ | NYW | TBD | NYW |
| Image.frombytes | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.get_child_images | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.get_flattened_data | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.getbands | 0.35× | N/A | 2.72× | N/A | TBD | N/A |
| Image.getbbox | 0.66× | N/A | 2.56× | N/A | TBD | N/A |
| Image.getchannel | 0.60× | N/A | TBD | N/A | TBD | N/A |
| Image.getcolors | 0.16× | N/A | TBD | N/A | TBD | N/A |
| Image.getdata | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.getexif | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.getextrema | 0.64× | N/A | 2.37× | N/A | TBD | N/A |
| Image.getim | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.getpalette | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.getpixel | 0.19× | N/A | 2.83× | N/A | TBD | N/A |
| Image.getprojection | 0.08× | N/A | TBD | N/A | TBD | N/A |
| Image.getxmp | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.histogram | 0.66× | N/A | 2.83× | N/A | TBD | N/A |
| Image.load | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.open | TBD | N/A | 324.79× ⚠️ | N/A | TBD | N/A |
| Image.point | 0.08× | NYW | TBD | NYW | TBD | NYW |
| Image.putalpha | 3.52× | N/A | TBD | N/A | TBD | N/A |
| Image.putdata | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.putpalette | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.putpixel | 0.68× | N/A | 54.91× | N/A | TBD | N/A |
| Image.quantize | 2.19× | NYW | TBD | NYW | TBD | NYW |
| Image.reduce | 0.34× | NYW | 17574.16× ⚠️ | NYW | TBD | NYW |
| Image.remap_palette | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.save | TBD | N/A | 2.55× | N/A | TBD | N/A |
| Image.seek | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.show | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.split | 0.29× | N/A | 1.15× | N/A | TBD | N/A |
| Image.tell | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.tobitmap | TBD | N/A | TBD | N/A | TBD | N/A |
| Image.tobytes | 3.91× | N/A | TBD | N/A | TBD | N/A |
| Image.transform | TBD | NYW | TBD | NYW | TBD | NYW |
| Image.verify | TBD | N/A | TBD | N/A | TBD | N/A |

### ImageChops

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageChops.add | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.add_modulo | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.blend | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.composite | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageChops.constant | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageChops.darker | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.difference | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.duplicate | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageChops.hard_light | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.invert | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.lighter | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.logical_and | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.logical_or | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.logical_xor | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.multiply | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.offset | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageChops.overlay | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.screen | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.soft_light | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.subtract | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageChops.subtract_modulo | TBD | NYW | TBD | NYW | TBD | NYW |

### ImageColor

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageColor.getcolor | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageColor.getrgb | TBD | N/A | TBD | N/A | TBD | N/A |

### ImageDraw

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageDraw.arc | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.bitmap | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.chord | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.circle | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.ellipse | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.getfont | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.line | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.multiline_text | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.multiline_textbbox | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.pieslice | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.point | N/A | NYW | N/A | NYW | N/A | NYW |
| ImageDraw.polygon | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.rectangle | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.regular_polygon | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.rounded_rectangle | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.text | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.textbbox | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageDraw.textlength | TBD | N/A | TBD | N/A | TBD | N/A |

### ImageEnhance

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageEnhance.Brightness | TBD | NYW | 17669.65× ⚠️ | NYW | TBD | NYW |
| ImageEnhance.Color | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageEnhance.Contrast | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageEnhance.Sharpness | TBD | NYW | TBD | NYW | TBD | NYW |

### ImageFilter

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFilter.BLUR | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.BoxBlur | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.CONTOUR | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.Color3DLUT | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFilter.DETAIL | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.EDGE_ENHANCE | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.EDGE_ENHANCE_MORE | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.EMBOSS | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.FIND_EDGES | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.GaussianBlur | TBD | N/A | 28292.55× ⚠️ | N/A | TBD | N/A |
| ImageFilter.Kernel | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.MaxFilter | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.MedianFilter | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.MinFilter | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.ModeFilter | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.RankFilter | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.SHARPEN | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.SMOOTH | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.SMOOTH_MORE | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFilter.UnsharpMask | TBD | N/A | TBD | N/A | TBD | N/A |

### ImageFont

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageFont.FreeTypeFont | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.ImageFont | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.FreeTypeFont.getbbox | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.ImageFont.getbbox | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.FreeTypeFont.getlength | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.ImageFont.getlength | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.FreeTypeFont.getmask | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.ImageFont.getmask | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.FreeTypeFont.getmetrics | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.FreeTypeFont.getname | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.load | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.load_default | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.load_default_imagefont | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageFont.load_path | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageFont.truetype | TBD | N/A | TBD | N/A | TBD | N/A |

### ImageModule

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageModule.new | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.alpha_composite | N/A | N/A | N/A | N/A | N/A | N/A |
| ImageModule.blend | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageModule.composite | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageModule.effect_noise | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageModule.eval | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageModule.fromarray | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageModule.frombytes | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageModule.merge | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageModule.open | TBD | N/A | TBD | N/A | TBD | N/A |

### ImageOps

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageOps.crop | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageOps.autocontrast | 0.09× | NYW | TBD | NYW | TBD | NYW |
| ImageOps.colorize | TBD | NYW | TBD | NYW | TBD | NYW |
| ImageOps.contain | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageOps.cover | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageOps.deform | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageOps.equalize | 0.10× | NYW | TBD | NYW | TBD | NYW |
| ImageOps.exif_transpose | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageOps.expand | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageOps.fit | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageOps.flip | 3.75× | N/A | TBD | N/A | TBD | N/A |
| ImageOps.grayscale | 2.99× | N/A | 14721.49× ⚠️ | N/A | TBD | N/A |
| ImageOps.invert | 0.45× | NYW | 2254.49× ⚠️ | NYW | TBD | NYW |
| ImageOps.mirror | 2.77× | N/A | TBD | N/A | TBD | N/A |
| ImageOps.pad | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageOps.posterize | 0.42× | NYW | TBD | NYW | TBD | NYW |
| ImageOps.scale | TBD | N/A | TBD | N/A | TBD | N/A |
| ImageOps.solarize | 0.28× | NYW | TBD | NYW | TBD | NYW |

### ImagePalette

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImagePalette.copy | N/A | N/A | N/A | N/A | N/A | N/A |
| ImagePalette.getcolor | TBD | N/A | TBD | N/A | TBD | N/A |
| ImagePalette.getdata | TBD | N/A | TBD | N/A | TBD | N/A |
| ImagePalette.save | N/A | N/A | N/A | N/A | N/A | N/A |
| ImagePalette.tobytes | TBD | N/A | TBD | N/A | TBD | N/A |

### ImageSequence

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageSequence.Iterator | TBD | N/A | TBD | N/A | TBD | N/A |

### ImageStat

| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
| --- | --- | --- | --- | --- | --- | --- |
| ImageStat.Stat | TBD | N/A | TBD | N/A | TBD | N/A |
