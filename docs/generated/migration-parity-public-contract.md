# Migration parity public contract

This is the generated specification view. It contains declared public
contract and indexed input mappings only; it contains no measured result.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 0bbb44ffa8992bb7a5772f65deabf1ebdee9611e59d672477088c25d6705c39e
statement_status: declared
```

## Scope

- Scope: `pillow-rs-selected-public-contract` (`full`)
- Oracle: `Pillow 12.2.0`
- Target profiles: `python-cpu`
- Public surfaces: 22
- Operations: 204
- Requirements: 1780
- Indexed parity cases: 1244
- Indexed coverage plans: 22
- Indexed benchmark workloads: 203

## Declared operations

| Surface | Operation | Kind | Source path | Target path | Requirements |
| --- | --- | --- | --- | --- | ---: |
| `PIL.Image` | `alpha_composite` | `function` | `PIL.Image.alpha_composite` | `pillow_rs.alpha_composite` | 7 |
| `PIL.Image` | `blend` | `function` | `PIL.Image.blend` | `pillow_rs.blend` | 11 |
| `PIL.Image` | `composite` | `function` | `PIL.Image.composite` | `pillow_rs.composite` | 13 |
| `PIL.Image` | `effect_mandelbrot` | `function` | `PIL.Image.effect_mandelbrot` | `pillow_rs.effect_mandelbrot` | 6 |
| `PIL.Image` | `effect_noise` | `function` | `PIL.Image.effect_noise` | `pillow_rs.effect_noise` | 6 |
| `PIL.Image` | `eval` | `function` | `PIL.Image.eval` | `pillow_rs.eval` | 12 |
| `PIL.Image` | `fromarray` | `function` | `PIL.Image.fromarray` | `pillow_rs.fromarray` | 8 |
| `PIL.Image` | `frombuffer` | `function` | `PIL.Image.frombuffer` | `pillow_rs.frombuffer` | 8 |
| `PIL.Image` | `frombytes` | `function` | `PIL.Image.frombytes` | `pillow_rs.frombytes` | 15 |
| `PIL.Image` | `linear_gradient` | `function` | `PIL.Image.linear_gradient` | `pillow_rs.linear_gradient` | 5 |
| `PIL.Image` | `merge` | `function` | `PIL.Image.merge` | `pillow_rs.merge` | 10 |
| `PIL.Image` | `new` | `function` | `PIL.Image.new` | `pillow_rs.new` | 28 |
| `PIL.Image` | `open` | `function` | `PIL.Image.open` | `pillow_rs.open` | 30 |
| `PIL.Image` | `radial_gradient` | `function` | `PIL.Image.radial_gradient` | `pillow_rs.radial_gradient` | 5 |
| `PIL.Image.Image` | `alpha_composite` | `method` | `PIL.Image.Image.alpha_composite` | `pillow_rs.Image.alpha_composite` | 9 |
| `PIL.Image.Image` | `apply_transparency` | `method` | `PIL.Image.Image.apply_transparency` | `pillow_rs.Image.apply_transparency` | 9 |
| `PIL.Image.Image` | `close` | `method` | `PIL.Image.Image.close` | `pillow_rs.Image.close` | 9 |
| `PIL.Image.Image` | `convert` | `method` | `PIL.Image.Image.convert` | `pillow_rs.Image.convert` | 33 |
| `PIL.Image.Image` | `copy` | `method` | `PIL.Image.Image.copy` | `pillow_rs.Image.copy` | 9 |
| `PIL.Image.Image` | `crop` | `method` | `PIL.Image.Image.crop` | `pillow_rs.Image.crop` | 20 |
| `PIL.Image.Image` | `draft` | `method` | `PIL.Image.Image.draft` | `pillow_rs.Image.draft` | 11 |
| `PIL.Image.Image` | `effect_spread` | `method` | `PIL.Image.Image.effect_spread` | `pillow_rs.Image.effect_spread` | 11 |
| `PIL.Image.Image` | `entropy` | `method` | `PIL.Image.Image.entropy` | `pillow_rs.Image.entropy` | 12 |
| `PIL.Image.Image` | `filter` | `method` | `PIL.Image.Image.filter` | `pillow_rs.Image.filter` | 27 |
| `PIL.Image.Image` | `format` | `property_get` | `PIL.Image.Image.format` | `pillow_rs.Image.format` | 2 |
| `PIL.Image.Image` | `frombytes` | `method` | `PIL.Image.Image.frombytes` | `pillow_rs.Image.frombytes` | 10 |
| `PIL.Image.Image` | `get_child_images` | `method` | `PIL.Image.Image.get_child_images` | `pillow_rs.Image.get_child_images` | 5 |
| `PIL.Image.Image` | `get_flattened_data` | `method` | `PIL.Image.Image.get_flattened_data` | `pillow_rs.Image.get_flattened_data` | 6 |
| `PIL.Image.Image` | `getbands` | `method` | `PIL.Image.Image.getbands` | `pillow_rs.Image.getbands` | 9 |
| `PIL.Image.Image` | `getbbox` | `method` | `PIL.Image.Image.getbbox` | `pillow_rs.Image.getbbox` | 13 |
| `PIL.Image.Image` | `getchannel` | `method` | `PIL.Image.Image.getchannel` | `pillow_rs.Image.getchannel` | 14 |
| `PIL.Image.Image` | `getcolors` | `method` | `PIL.Image.Image.getcolors` | `pillow_rs.Image.getcolors` | 12 |
| `PIL.Image.Image` | `getdata` | `method` | `PIL.Image.Image.getdata` | `pillow_rs.Image.getdata` | 10 |
| `PIL.Image.Image` | `getexif` | `method` | `PIL.Image.Image.getexif` | `pillow_rs.Image.getexif` | 5 |
| `PIL.Image.Image` | `getextrema` | `method` | `PIL.Image.Image.getextrema` | `pillow_rs.Image.getextrema` | 10 |
| `PIL.Image.Image` | `getim` | `method` | `PIL.Image.Image.getim` | `pillow_rs.Image.getim` | 3 |
| `PIL.Image.Image` | `getpalette` | `method` | `PIL.Image.Image.getpalette` | `pillow_rs.Image.getpalette` | 7 |
| `PIL.Image.Image` | `getpixel` | `method` | `PIL.Image.Image.getpixel` | `pillow_rs.Image.getpixel` | 11 |
| `PIL.Image.Image` | `getprojection` | `method` | `PIL.Image.Image.getprojection` | `pillow_rs.Image.getprojection` | 9 |
| `PIL.Image.Image` | `getxmp` | `method` | `PIL.Image.Image.getxmp` | `pillow_rs.Image.getxmp` | 5 |
| `PIL.Image.Image` | `height` | `property_get` | `PIL.Image.Image.height` | `pillow_rs.Image.height` | 2 |
| `PIL.Image.Image` | `histogram` | `method` | `PIL.Image.Image.histogram` | `pillow_rs.Image.histogram` | 12 |
| `PIL.Image.Image` | `info` | `property_get` | `PIL.Image.Image.info` | `pillow_rs.Image.info` | 2 |
| `PIL.Image.Image` | `load` | `method` | `PIL.Image.Image.load` | `pillow_rs.Image.load` | 9 |
| `PIL.Image.Image` | `mode` | `property_get` | `PIL.Image.Image.mode` | `pillow_rs.Image.mode` | 2 |
| `PIL.Image.Image` | `paste` | `method` | `PIL.Image.Image.paste` | `pillow_rs.Image.paste` | 22 |
| `PIL.Image.Image` | `point` | `method` | `PIL.Image.Image.point` | `pillow_rs.Image.point` | 11 |
| `PIL.Image.Image` | `putalpha` | `method` | `PIL.Image.Image.putalpha` | `pillow_rs.Image.putalpha` | 11 |
| `PIL.Image.Image` | `putdata` | `method` | `PIL.Image.Image.putdata` | `pillow_rs.Image.putdata` | 16 |
| `PIL.Image.Image` | `putpalette` | `method` | `PIL.Image.Image.putpalette` | `pillow_rs.Image.putpalette` | 7 |
| `PIL.Image.Image` | `putpixel` | `method` | `PIL.Image.Image.putpixel` | `pillow_rs.Image.putpixel` | 12 |
| `PIL.Image.Image` | `quantize` | `method` | `PIL.Image.Image.quantize` | `pillow_rs.Image.quantize` | 12 |
| `PIL.Image.Image` | `reduce` | `method` | `PIL.Image.Image.reduce` | `pillow_rs.Image.reduce` | 10 |
| `PIL.Image.Image` | `remap_palette` | `method` | `PIL.Image.Image.remap_palette` | `pillow_rs.Image.remap_palette` | 7 |
| `PIL.Image.Image` | `resize` | `method` | `PIL.Image.Image.resize` | `pillow_rs.Image.resize` | 25 |
| `PIL.Image.Image` | `rotate` | `method` | `PIL.Image.Image.rotate` | `pillow_rs.Image.rotate` | 23 |
| `PIL.Image.Image` | `save` | `method` | `PIL.Image.Image.save` | `pillow_rs.Image.save` | 22 |
| `PIL.Image.Image` | `seek` | `method` | `PIL.Image.Image.seek` | `pillow_rs.Image.seek` | 11 |
| `PIL.Image.Image` | `show` | `method` | `PIL.Image.Image.show` | `pillow_rs.Image.show` | 4 |
| `PIL.Image.Image` | `size` | `property_get` | `PIL.Image.Image.size` | `pillow_rs.Image.size` | 2 |
| `PIL.Image.Image` | `split` | `method` | `PIL.Image.Image.split` | `pillow_rs.Image.split` | 8 |
| `PIL.Image.Image` | `tell` | `method` | `PIL.Image.Image.tell` | `pillow_rs.Image.tell` | 10 |
| `PIL.Image.Image` | `thumbnail` | `method` | `PIL.Image.Image.thumbnail` | `pillow_rs.Image.thumbnail` | 15 |
| `PIL.Image.Image` | `tobitmap` | `method` | `PIL.Image.Image.tobitmap` | `pillow_rs.Image.tobitmap` | 6 |
| `PIL.Image.Image` | `tobytes` | `method` | `PIL.Image.Image.tobytes` | `pillow_rs.Image.tobytes` | 12 |
| `PIL.Image.Image` | `toqimage` | `method` | `PIL.Image.Image.toqimage` | `pillow_rs.Image.toqimage` | 3 |
| `PIL.Image.Image` | `toqpixmap` | `method` | `PIL.Image.Image.toqpixmap` | `pillow_rs.Image.toqpixmap` | 3 |
| `PIL.Image.Image` | `transform` | `method` | `PIL.Image.Image.transform` | `pillow_rs.Image.transform` | 15 |
| `PIL.Image.Image` | `transpose` | `method` | `PIL.Image.Image.transpose` | `pillow_rs.Image.transpose` | 16 |
| `PIL.Image.Image` | `verify` | `method` | `PIL.Image.Image.verify` | `pillow_rs.Image.verify` | 7 |
| `PIL.Image.Image` | `width` | `property_get` | `PIL.Image.Image.width` | `pillow_rs.Image.width` | 2 |
| `PIL.ImageChops` | `add` | `function` | `PIL.ImageChops.add` | `pillow_rs.ImageChops.add` | 10 |
| `PIL.ImageChops` | `add_modulo` | `function` | `PIL.ImageChops.add_modulo` | `pillow_rs.ImageChops.add_modulo` | 8 |
| `PIL.ImageChops` | `blend` | `function` | `PIL.ImageChops.blend` | `pillow_rs.ImageChops.blend` | 7 |
| `PIL.ImageChops` | `composite` | `function` | `PIL.ImageChops.composite` | `pillow_rs.ImageChops.composite` | 8 |
| `PIL.ImageChops` | `constant` | `function` | `PIL.ImageChops.constant` | `pillow_rs.ImageChops.constant` | 7 |
| `PIL.ImageChops` | `darker` | `function` | `PIL.ImageChops.darker` | `pillow_rs.ImageChops.darker` | 8 |
| `PIL.ImageChops` | `difference` | `function` | `PIL.ImageChops.difference` | `pillow_rs.ImageChops.difference` | 8 |
| `PIL.ImageChops` | `duplicate` | `function` | `PIL.ImageChops.duplicate` | `pillow_rs.ImageChops.duplicate` | 6 |
| `PIL.ImageChops` | `hard_light` | `function` | `PIL.ImageChops.hard_light` | `pillow_rs.ImageChops.hard_light` | 8 |
| `PIL.ImageChops` | `invert` | `function` | `PIL.ImageChops.invert` | `pillow_rs.ImageChops.invert` | 6 |
| `PIL.ImageChops` | `lighter` | `function` | `PIL.ImageChops.lighter` | `pillow_rs.ImageChops.lighter` | 8 |
| `PIL.ImageChops` | `logical_and` | `function` | `PIL.ImageChops.logical_and` | `pillow_rs.ImageChops.logical_and` | 5 |
| `PIL.ImageChops` | `logical_or` | `function` | `PIL.ImageChops.logical_or` | `pillow_rs.ImageChops.logical_or` | 5 |
| `PIL.ImageChops` | `logical_xor` | `function` | `PIL.ImageChops.logical_xor` | `pillow_rs.ImageChops.logical_xor` | 5 |
| `PIL.ImageChops` | `multiply` | `function` | `PIL.ImageChops.multiply` | `pillow_rs.ImageChops.multiply` | 8 |
| `PIL.ImageChops` | `offset` | `function` | `PIL.ImageChops.offset` | `pillow_rs.ImageChops.offset` | 8 |
| `PIL.ImageChops` | `overlay` | `function` | `PIL.ImageChops.overlay` | `pillow_rs.ImageChops.overlay` | 6 |
| `PIL.ImageChops` | `screen` | `function` | `PIL.ImageChops.screen` | `pillow_rs.ImageChops.screen` | 8 |
| `PIL.ImageChops` | `soft_light` | `function` | `PIL.ImageChops.soft_light` | `pillow_rs.ImageChops.soft_light` | 8 |
| `PIL.ImageChops` | `subtract` | `function` | `PIL.ImageChops.subtract` | `pillow_rs.ImageChops.subtract` | 10 |
| `PIL.ImageChops` | `subtract_modulo` | `function` | `PIL.ImageChops.subtract_modulo` | `pillow_rs.ImageChops.subtract_modulo` | 8 |
| `PIL.ImageColor` | `getcolor` | `function` | `PIL.ImageColor.getcolor` | `pillow_rs.ImageColor.getcolor` | 6 |
| `PIL.ImageColor` | `getrgb` | `function` | `PIL.ImageColor.getrgb` | `pillow_rs.ImageColor.getrgb` | 3 |
| `PIL.ImageDraw` | `Draw` | `function` | `PIL.ImageDraw.Draw` | `pillow_rs.ImageDraw.Draw` | 4 |
| `PIL.ImageDraw` | `Outline` | `function` | `PIL.ImageDraw.Outline` | `pillow_rs.ImageDraw.Outline` | 2 |
| `PIL.ImageDraw.ImageDraw` | `arc` | `method` | `PIL.ImageDraw.ImageDraw.arc` | `pillow_rs.ImageDraw.Draw.arc` | 16 |
| `PIL.ImageDraw.ImageDraw` | `bitmap` | `method` | `PIL.ImageDraw.ImageDraw.bitmap` | `pillow_rs.ImageDraw.Draw.bitmap` | 13 |
| `PIL.ImageDraw.ImageDraw` | `chord` | `method` | `PIL.ImageDraw.ImageDraw.chord` | `pillow_rs.ImageDraw.Draw.chord` | 17 |
| `PIL.ImageDraw.ImageDraw` | `circle` | `method` | `PIL.ImageDraw.ImageDraw.circle` | `pillow_rs.ImageDraw.Draw.circle` | 16 |
| `PIL.ImageDraw.ImageDraw` | `ellipse` | `method` | `PIL.ImageDraw.ImageDraw.ellipse` | `pillow_rs.ImageDraw.Draw.ellipse` | 15 |
| `PIL.ImageDraw.ImageDraw` | `getfont` | `method` | `PIL.ImageDraw.ImageDraw.getfont` | `pillow_rs.ImageDraw.Draw.getfont` | 3 |
| `PIL.ImageDraw.ImageDraw` | `line` | `method` | `PIL.ImageDraw.ImageDraw.line` | `pillow_rs.ImageDraw.Draw.line` | 16 |
| `PIL.ImageDraw.ImageDraw` | `multiline_text` | `method` | `PIL.ImageDraw.ImageDraw.multiline_text` | `pillow_rs.ImageDraw.Draw.multiline_text` | 24 |
| `PIL.ImageDraw.ImageDraw` | `multiline_textbbox` | `method` | `PIL.ImageDraw.ImageDraw.multiline_textbbox` | `pillow_rs.ImageDraw.Draw.multiline_textbbox` | 18 |
| `PIL.ImageDraw.ImageDraw` | `pieslice` | `method` | `PIL.ImageDraw.ImageDraw.pieslice` | `pillow_rs.ImageDraw.Draw.pieslice` | 17 |
| `PIL.ImageDraw.ImageDraw` | `point` | `method` | `PIL.ImageDraw.ImageDraw.point` | `pillow_rs.ImageDraw.Draw.point` | 13 |
| `PIL.ImageDraw.ImageDraw` | `polygon` | `method` | `PIL.ImageDraw.ImageDraw.polygon` | `pillow_rs.ImageDraw.Draw.polygon` | 15 |
| `PIL.ImageDraw.ImageDraw` | `rectangle` | `method` | `PIL.ImageDraw.ImageDraw.rectangle` | `pillow_rs.ImageDraw.Draw.rectangle` | 16 |
| `PIL.ImageDraw.ImageDraw` | `regular_polygon` | `method` | `PIL.ImageDraw.ImageDraw.regular_polygon` | `pillow_rs.ImageDraw.Draw.regular_polygon` | 15 |
| `PIL.ImageDraw.ImageDraw` | `rounded_rectangle` | `method` | `PIL.ImageDraw.ImageDraw.rounded_rectangle` | `pillow_rs.ImageDraw.Draw.rounded_rectangle` | 17 |
| `PIL.ImageDraw.ImageDraw` | `shape` | `method` | `PIL.ImageDraw.ImageDraw.shape` | `pillow_rs.ImageDraw.Draw.shape` | 6 |
| `PIL.ImageDraw.ImageDraw` | `text` | `method` | `PIL.ImageDraw.ImageDraw.text` | `pillow_rs.ImageDraw.Draw.text` | 25 |
| `PIL.ImageDraw.ImageDraw` | `textbbox` | `method` | `PIL.ImageDraw.ImageDraw.textbbox` | `pillow_rs.ImageDraw.Draw.textbbox` | 18 |
| `PIL.ImageDraw.ImageDraw` | `textlength` | `method` | `PIL.ImageDraw.ImageDraw.textlength` | `pillow_rs.ImageDraw.Draw.textlength` | 13 |
| `PIL.ImageEnhance` | `Brightness` | `type` | `PIL.ImageEnhance.Brightness` | `pillow_rs.ImageEnhance.Brightness` | 7 |
| `PIL.ImageEnhance` | `Color` | `type` | `PIL.ImageEnhance.Color` | `pillow_rs.ImageEnhance.Color` | 7 |
| `PIL.ImageEnhance` | `Contrast` | `type` | `PIL.ImageEnhance.Contrast` | `pillow_rs.ImageEnhance.Contrast` | 7 |
| `PIL.ImageEnhance` | `Sharpness` | `type` | `PIL.ImageEnhance.Sharpness` | `pillow_rs.ImageEnhance.Sharpness` | 7 |
| `PIL.ImageEnhance.Brightness` | `enhance` | `method` | `PIL.ImageEnhance.Brightness.enhance` | `pillow_rs.ImageEnhance.Brightness.enhance` | 3 |
| `PIL.ImageEnhance.Color` | `enhance` | `method` | `PIL.ImageEnhance.Color.enhance` | `pillow_rs.ImageEnhance.Color.enhance` | 3 |
| `PIL.ImageEnhance.Contrast` | `enhance` | `method` | `PIL.ImageEnhance.Contrast.enhance` | `pillow_rs.ImageEnhance.Contrast.enhance` | 3 |
| `PIL.ImageEnhance.Sharpness` | `enhance` | `method` | `PIL.ImageEnhance.Sharpness.enhance` | `pillow_rs.ImageEnhance.Sharpness.enhance` | 3 |
| `PIL.ImageFilter` | `BLUR` | `type` | `PIL.ImageFilter.BLUR` | `pillow_rs.ImageFilter.BLUR` | 8 |
| `PIL.ImageFilter` | `BoxBlur` | `type` | `PIL.ImageFilter.BoxBlur` | `pillow_rs.ImageFilter.BoxBlur` | 7 |
| `PIL.ImageFilter` | `CONTOUR` | `type` | `PIL.ImageFilter.CONTOUR` | `pillow_rs.ImageFilter.CONTOUR` | 6 |
| `PIL.ImageFilter` | `Color3DLUT` | `type` | `PIL.ImageFilter.Color3DLUT` | `pillow_rs.ImageFilter.Color3DLUT` | 10 |
| `PIL.ImageFilter` | `DETAIL` | `type` | `PIL.ImageFilter.DETAIL` | `pillow_rs.ImageFilter.DETAIL` | 6 |
| `PIL.ImageFilter` | `EDGE_ENHANCE` | `type` | `PIL.ImageFilter.EDGE_ENHANCE` | `pillow_rs.ImageFilter.EDGE_ENHANCE` | 7 |
| `PIL.ImageFilter` | `EDGE_ENHANCE_MORE` | `type` | `PIL.ImageFilter.EDGE_ENHANCE_MORE` | `pillow_rs.ImageFilter.EDGE_ENHANCE_MORE` | 6 |
| `PIL.ImageFilter` | `EMBOSS` | `type` | `PIL.ImageFilter.EMBOSS` | `pillow_rs.ImageFilter.EMBOSS` | 6 |
| `PIL.ImageFilter` | `FIND_EDGES` | `type` | `PIL.ImageFilter.FIND_EDGES` | `pillow_rs.ImageFilter.FIND_EDGES` | 6 |
| `PIL.ImageFilter` | `GaussianBlur` | `type` | `PIL.ImageFilter.GaussianBlur` | `pillow_rs.ImageFilter.GaussianBlur` | 7 |
| `PIL.ImageFilter` | `Kernel` | `type` | `PIL.ImageFilter.Kernel` | `pillow_rs.ImageFilter.Kernel` | 10 |
| `PIL.ImageFilter` | `MaxFilter` | `type` | `PIL.ImageFilter.MaxFilter` | `pillow_rs.ImageFilter.MaxFilter` | 7 |
| `PIL.ImageFilter` | `MedianFilter` | `type` | `PIL.ImageFilter.MedianFilter` | `pillow_rs.ImageFilter.MedianFilter` | 7 |
| `PIL.ImageFilter` | `MinFilter` | `type` | `PIL.ImageFilter.MinFilter` | `pillow_rs.ImageFilter.MinFilter` | 7 |
| `PIL.ImageFilter` | `ModeFilter` | `type` | `PIL.ImageFilter.ModeFilter` | `pillow_rs.ImageFilter.ModeFilter` | 7 |
| `PIL.ImageFilter` | `RankFilter` | `type` | `PIL.ImageFilter.RankFilter` | `pillow_rs.ImageFilter.RankFilter` | 8 |
| `PIL.ImageFilter` | `SHARPEN` | `type` | `PIL.ImageFilter.SHARPEN` | `pillow_rs.ImageFilter.SHARPEN` | 11 |
| `PIL.ImageFilter` | `SMOOTH` | `type` | `PIL.ImageFilter.SMOOTH` | `pillow_rs.ImageFilter.SMOOTH` | 6 |
| `PIL.ImageFilter` | `SMOOTH_MORE` | `type` | `PIL.ImageFilter.SMOOTH_MORE` | `pillow_rs.ImageFilter.SMOOTH_MORE` | 6 |
| `PIL.ImageFilter` | `UnsharpMask` | `type` | `PIL.ImageFilter.UnsharpMask` | `pillow_rs.ImageFilter.UnsharpMask` | 9 |
| `PIL.ImageFont` | `FreeTypeFont` | `type` | `PIL.ImageFont.FreeTypeFont` | `pillow_rs.ImageFont.FreeTypeFont` | 7 |
| `PIL.ImageFont` | `ImageFont` | `type` | `PIL.ImageFont.ImageFont` | `pillow_rs.ImageFont.ImageFont` | 2 |
| `PIL.ImageFont` | `MAX_STRING_LENGTH` | `constant` | `PIL.ImageFont.MAX_STRING_LENGTH` | `pillow_rs.ImageFont.MAX_STRING_LENGTH` | 1 |
| `PIL.ImageFont` | `TransposedFont` | `type` | `PIL.ImageFont.TransposedFont` | `pillow_rs.ImageFont.TransposedFont` | 4 |
| `PIL.ImageFont` | `load` | `function` | `PIL.ImageFont.load` | `pillow_rs.ImageFont.load` | 3 |
| `PIL.ImageFont` | `load_default` | `function` | `PIL.ImageFont.load_default` | `pillow_rs.ImageFont.load_default` | 3 |
| `PIL.ImageFont` | `load_default_imagefont` | `function` | `PIL.ImageFont.load_default_imagefont` | `pillow_rs.ImageFont.load_default_imagefont` | 3 |
| `PIL.ImageFont` | `load_path` | `function` | `PIL.ImageFont.load_path` | `pillow_rs.ImageFont.load_path` | 4 |
| `PIL.ImageFont` | `truetype` | `function` | `PIL.ImageFont.truetype` | `pillow_rs.ImageFont.truetype` | 7 |
| `PIL.ImageFont.FreeTypeFont` | `font_variant` | `method` | `PIL.ImageFont.FreeTypeFont.font_variant` | `pillow_rs.ImageFont.FreeTypeFont.font_variant` | 7 |
| `PIL.ImageFont.FreeTypeFont` | `get_variation_axes` | `method` | `PIL.ImageFont.FreeTypeFont.get_variation_axes` | `pillow_rs.ImageFont.FreeTypeFont.get_variation_axes` | 2 |
| `PIL.ImageFont.FreeTypeFont` | `get_variation_names` | `method` | `PIL.ImageFont.FreeTypeFont.get_variation_names` | `pillow_rs.ImageFont.FreeTypeFont.get_variation_names` | 2 |
| `PIL.ImageFont.FreeTypeFont` | `getbbox` | `method` | `PIL.ImageFont.FreeTypeFont.getbbox` | `pillow_rs.ImageFont.FreeTypeFont.getbbox` | 9 |
| `PIL.ImageFont.FreeTypeFont` | `getlength` | `method` | `PIL.ImageFont.FreeTypeFont.getlength` | `pillow_rs.ImageFont.FreeTypeFont.getlength` | 7 |
| `PIL.ImageFont.FreeTypeFont` | `getmask` | `method` | `PIL.ImageFont.FreeTypeFont.getmask` | `pillow_rs.ImageFont.FreeTypeFont.getmask` | 11 |
| `PIL.ImageFont.FreeTypeFont` | `getmask2` | `method` | `PIL.ImageFont.FreeTypeFont.getmask2` | `pillow_rs.ImageFont.FreeTypeFont.getmask2` | 13 |
| `PIL.ImageFont.FreeTypeFont` | `getmetrics` | `method` | `PIL.ImageFont.FreeTypeFont.getmetrics` | `pillow_rs.ImageFont.FreeTypeFont.getmetrics` | 2 |
| `PIL.ImageFont.FreeTypeFont` | `getname` | `method` | `PIL.ImageFont.FreeTypeFont.getname` | `pillow_rs.ImageFont.FreeTypeFont.getname` | 2 |
| `PIL.ImageFont.FreeTypeFont` | `set_variation_by_axes` | `method` | `PIL.ImageFont.FreeTypeFont.set_variation_by_axes` | `pillow_rs.ImageFont.FreeTypeFont.set_variation_by_axes` | 3 |
| `PIL.ImageFont.FreeTypeFont` | `set_variation_by_name` | `method` | `PIL.ImageFont.FreeTypeFont.set_variation_by_name` | `pillow_rs.ImageFont.FreeTypeFont.set_variation_by_name` | 3 |
| `PIL.ImageFont.ImageFont` | `getbbox` | `method` | `PIL.ImageFont.ImageFont.getbbox` | `pillow_rs.ImageFont.ImageFont.getbbox` | 5 |
| `PIL.ImageFont.ImageFont` | `getlength` | `method` | `PIL.ImageFont.ImageFont.getlength` | `pillow_rs.ImageFont.ImageFont.getlength` | 5 |
| `PIL.ImageFont.ImageFont` | `getmask` | `method` | `PIL.ImageFont.ImageFont.getmask` | `pillow_rs.ImageFont.ImageFont.getmask` | 6 |
| `PIL.ImageFont.TransposedFont` | `getbbox` | `method` | `PIL.ImageFont.TransposedFont.getbbox` | `pillow_rs.ImageFont.TransposedFont.getbbox` | 5 |
| `PIL.ImageFont.TransposedFont` | `getlength` | `method` | `PIL.ImageFont.TransposedFont.getlength` | `pillow_rs.ImageFont.TransposedFont.getlength` | 5 |
| `PIL.ImageFont.TransposedFont` | `getmask` | `method` | `PIL.ImageFont.TransposedFont.getmask` | `pillow_rs.ImageFont.TransposedFont.getmask` | 6 |
| `PIL.ImageOps` | `autocontrast` | `function` | `PIL.ImageOps.autocontrast` | `pillow_rs.ImageOps.autocontrast` | 11 |
| `PIL.ImageOps` | `colorize` | `function` | `PIL.ImageOps.colorize` | `pillow_rs.ImageOps.colorize` | 11 |
| `PIL.ImageOps` | `contain` | `function` | `PIL.ImageOps.contain` | `pillow_rs.ImageOps.contain` | 9 |
| `PIL.ImageOps` | `cover` | `function` | `PIL.ImageOps.cover` | `pillow_rs.ImageOps.cover` | 9 |
| `PIL.ImageOps` | `crop` | `function` | `PIL.ImageOps.crop` | `pillow_rs.ImageOps.crop` | 8 |
| `PIL.ImageOps` | `deform` | `function` | `PIL.ImageOps.deform` | `pillow_rs.ImageOps.deform` | 5 |
| `PIL.ImageOps` | `equalize` | `function` | `PIL.ImageOps.equalize` | `pillow_rs.ImageOps.equalize` | 8 |
| `PIL.ImageOps` | `exif_transpose` | `function` | `PIL.ImageOps.exif_transpose` | `pillow_rs.ImageOps.exif_transpose` | 4 |
| `PIL.ImageOps` | `expand` | `function` | `PIL.ImageOps.expand` | `pillow_rs.ImageOps.expand` | 9 |
| `PIL.ImageOps` | `fit` | `function` | `PIL.ImageOps.fit` | `pillow_rs.ImageOps.fit` | 11 |
| `PIL.ImageOps` | `flip` | `function` | `PIL.ImageOps.flip` | `pillow_rs.ImageOps.flip` | 7 |
| `PIL.ImageOps` | `grayscale` | `function` | `PIL.ImageOps.grayscale` | `pillow_rs.ImageOps.grayscale` | 8 |
| `PIL.ImageOps` | `invert` | `function` | `PIL.ImageOps.invert` | `pillow_rs.ImageOps.invert` | 7 |
| `PIL.ImageOps` | `mirror` | `function` | `PIL.ImageOps.mirror` | `pillow_rs.ImageOps.mirror` | 7 |
| `PIL.ImageOps` | `pad` | `function` | `PIL.ImageOps.pad` | `pillow_rs.ImageOps.pad` | 11 |
| `PIL.ImageOps` | `posterize` | `function` | `PIL.ImageOps.posterize` | `pillow_rs.ImageOps.posterize` | 8 |
| `PIL.ImageOps` | `scale` | `function` | `PIL.ImageOps.scale` | `pillow_rs.ImageOps.scale` | 9 |
| `PIL.ImageOps` | `solarize` | `function` | `PIL.ImageOps.solarize` | `pillow_rs.ImageOps.solarize` | 8 |
| `PIL.ImagePalette` | `ImagePalette` | `type` | `PIL.ImagePalette.ImagePalette` | `pillow_rs.ImagePalette.ImagePalette` | 4 |
| `PIL.ImagePalette.ImagePalette` | `copy` | `method` | `PIL.ImagePalette.ImagePalette.copy` | `pillow_rs.ImagePalette.ImagePalette.copy` | 5 |
| `PIL.ImagePalette.ImagePalette` | `getcolor` | `method` | `PIL.ImagePalette.ImagePalette.getcolor` | `pillow_rs.ImagePalette.ImagePalette.getcolor` | 7 |
| `PIL.ImagePalette.ImagePalette` | `getdata` | `method` | `PIL.ImagePalette.ImagePalette.getdata` | `pillow_rs.ImagePalette.ImagePalette.getdata` | 5 |
| `PIL.ImagePalette.ImagePalette` | `save` | `method` | `PIL.ImagePalette.ImagePalette.save` | `pillow_rs.ImagePalette.ImagePalette.save` | 6 |
| `PIL.ImagePalette.ImagePalette` | `tobytes` | `method` | `PIL.ImagePalette.ImagePalette.tobytes` | `pillow_rs.ImagePalette.ImagePalette.tobytes` | 5 |
| `PIL.ImageSequence` | `Iterator` | `type` | `PIL.ImageSequence.Iterator` | `pillow_rs.ImageSequence.Iterator` | 5 |
| `PIL.ImageStat` | `Stat` | `type` | `PIL.ImageStat.Stat` | `pillow_rs.ImageStat.Stat` | 7 |
| `PIL.ImageStat.Stat` | `count` | `property_get` | `PIL.ImageStat.Stat.count` | `pillow_rs.ImageStat.Stat.count` | 5 |
| `PIL.ImageStat.Stat` | `extrema` | `property_get` | `PIL.ImageStat.Stat.extrema` | `pillow_rs.ImageStat.Stat.extrema` | 5 |
| `PIL.ImageStat.Stat` | `mean` | `property_get` | `PIL.ImageStat.Stat.mean` | `pillow_rs.ImageStat.Stat.mean` | 5 |
| `PIL.ImageStat.Stat` | `median` | `property_get` | `PIL.ImageStat.Stat.median` | `pillow_rs.ImageStat.Stat.median` | 5 |
| `PIL.ImageStat.Stat` | `rms` | `property_get` | `PIL.ImageStat.Stat.rms` | `pillow_rs.ImageStat.Stat.rms` | 5 |
| `PIL.ImageStat.Stat` | `stddev` | `property_get` | `PIL.ImageStat.Stat.stddev` | `pillow_rs.ImageStat.Stat.stddev` | 5 |
| `PIL.ImageStat.Stat` | `sum` | `property_get` | `PIL.ImageStat.Stat.sum` | `pillow_rs.ImageStat.Stat.sum` | 5 |
| `PIL.ImageStat.Stat` | `sum2` | `property_get` | `PIL.ImageStat.Stat.sum2` | `pillow_rs.ImageStat.Stat.sum2` | 5 |
| `PIL.ImageStat.Stat` | `var` | `property_get` | `PIL.ImageStat.Stat.var` | `pillow_rs.ImageStat.Stat.var` | 5 |

## Lane inputs

The manifest index is closed: only the following indexed documents are
inputs to the corresponding lane. Results and documentation are not
accepted as input truth.

| Lane | Documents |
| --- | ---: |
| parity | 22 |
| coverage | 22 |
| benchmark | 22 |
