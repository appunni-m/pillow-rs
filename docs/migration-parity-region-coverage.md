# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@2
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 5f6d393d5c454d810f0a506192f85ef762ed055fda2017f306bfcb24ca597e2b
coverage_run_id: migration-coverage-6fa5b8dfa0764c208a3f2b072afa8d7b
coverage_target_profile: python-cpu
metric: region
threshold: below 95%
```

Each operation's coverage is the region coverage of the files declared
by its coverage component(s); operations inside one component share the
component's measured coverage by design.

## PIL.Image.Image.getbbox

`PIL.Image.Image.getbbox -> region coverage 9782/10953 (89.3%)`

## Operations below 95% region coverage

169 of 208 coverage-required operations are below 95%.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageFont.FreeTypeFont` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.font_variant` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.get_variation_axes` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.get_variation_names` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.getbbox` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.getlength` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.getmask` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.getmask2` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.getmetrics` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.getname` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_axes` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_name` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.ImageFont` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.ImageFont.getbbox` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.ImageFont.getlength` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.ImageFont.getmask` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.MAX_STRING_LENGTH` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.TransposedFont` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.TransposedFont.getbbox` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.TransposedFont.getlength` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.TransposedFont.getmask` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.load` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.load_default` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.load_default_imagefont` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.load_path` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageFont.truetype` | `image-font` | 2458/2908 | 84.5% |
| `PIL.ImageOps.autocontrast` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.colorize` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.contain` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.cover` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.crop` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.deform` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.equalize` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.exif_transpose` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.expand` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.fit` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.flip` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.grayscale` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.invert` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.mirror` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.pad` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.posterize` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.scale` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageOps.solarize` | `image-ops` | 570/661 | 86.2% |
| `PIL.ImageSequence.Iterator` | `image-sequence` | 4041/4648 | 86.9% |
| `PIL.ImageSequence.Iterator.__iter__` | `image-sequence` | 4041/4648 | 86.9% |
| `PIL.ImageSequence.Iterator.__next__` | `image-sequence` | 4041/4648 | 86.9% |
| `PIL.ImageDraw.Draw` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.arc` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.bitmap` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.chord` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.circle` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.ellipse` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.getfont` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.line` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.multiline_text` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.multiline_textbbox` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.pieslice` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.point` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.polygon` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.rectangle` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.regular_polygon` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.rounded_rectangle` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.shape` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.text` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.textbbox` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.ImageDraw.textlength` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.ImageDraw.Outline` | `image-draw` | 2407/2707 | 88.9% |
| `PIL.Image.Image.alpha_composite` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.apply_transparency` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.close` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.convert` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.copy` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.crop` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.draft` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.effect_spread` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.entropy` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.filter` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.format` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.frombytes` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.get_child_images` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.get_flattened_data` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getbands` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getbbox` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getchannel` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getcolors` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getdata` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getexif` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getextrema` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getim` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getpalette` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getpixel` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getprojection` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.getxmp` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.height` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.histogram` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.info` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.load` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.mode` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.paste` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.point` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.putalpha` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.putdata` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.putpalette` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.putpixel` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.quantize` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.reduce` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.remap_palette` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.resize` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.rotate` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.save` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.seek` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.size` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.split` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.tell` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.thumbnail` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.tobitmap` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.tobytes` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.toqimage` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.toqpixmap` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.transform` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.transpose` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.verify` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.Image.width` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.alpha_composite` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.blend` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.composite` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.effect_mandelbrot` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.effect_noise` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.eval` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.fromarray` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.frombuffer` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.frombytes` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.linear_gradient` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.merge` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.new` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.open` | `image-core` | 9782/10953 | 89.3% |
| `PIL.Image.radial_gradient` | `image-core` | 9782/10953 | 89.3% |
| `PIL.ImageColor.getcolor` | `image-color` | 1150/1226 | 93.8% |
| `PIL.ImageColor.getrgb` | `image-color` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.copy` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.getcolor` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.getdata` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.save` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.tobytes` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImageFilter.BLUR` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.BoxBlur` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.CONTOUR` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.Color3DLUT` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.Color3DLUT.__repr__` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.Color3DLUT.generate` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.Color3DLUT.transform` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.DETAIL` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.EDGE_ENHANCE` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.EDGE_ENHANCE_MORE` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.EMBOSS` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.FIND_EDGES` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.GaussianBlur` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.Kernel` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.MaxFilter` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.MedianFilter` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.MinFilter` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.ModeFilter` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.RankFilter` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.SHARPEN` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.SMOOTH` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.SMOOTH_MORE` | `image-filter` | 630/667 | 94.5% |
| `PIL.ImageFilter.UnsharpMask` | `image-filter` | 630/667 | 94.5% |

## Per-file region coverage for involved components

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-color` | `pillow-rs/src/color.rs` | 1150/1226 | 93.8% |
| `image-color` | `pillow-rs-py/python/pillow_rs/imagecolor.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/ops/paste.rs` | 700/848 | 82.5% |
| `image-core` | `pillow-rs/src/pipeline.rs` | 32/38 | 84.2% |
| `image-core` | `pillow-rs/src/image.rs` | 4041/4648 | 86.9% |
| `image-core` | `pillow-rs/src/ops/crop.rs` | 160/183 | 87.4% |
| `image-core` | `pillow-rs/src/ops/resize.rs` | 215/243 | 88.5% |
| `image-core` | `pillow-rs/src/ops/transform.rs` | 516/572 | 90.2% |
| `image-core` | `pillow-rs/src/ops/quantize.rs` | 2483/2697 | 92.1% |
| `image-core` | `pillow-rs/src/ops/convert.rs` | 763/812 | 94.0% |
| `image-core` | `pillow-rs/src/ops/module_fns.rs` | 451/479 | 94.2% |
| `image-core` | `pillow-rs/src/ops/split.rs` | 25/26 | 96.2% |
| `image-core` | `pillow-rs/src/ops/rotate.rs` | 29/30 | 96.7% |
| `image-core` | `pillow-rs/src/ops/transpose.rs` | 58/60 | 96.7% |
| `image-core` | `pillow-rs/src/ops/analysis.rs` | 309/317 | 97.5% |
| `image-core` | `pillow-rs-py/python/pillow_rs/image.py` | 0/0 | n/a |
| `image-core` | `pillow-rs-py/python/pillow_rs/operations.py` | 0/0 | n/a |
| `image-draw` | `pillow-rs/src/draw/mod.rs` | 2407/2707 | 88.9% |
| `image-draw` | `pillow-rs-py/python/pillow_rs/imagedraw.py` | 0/0 | n/a |
| `image-filter` | `pillow-rs/src/ops/filter.rs` | 125/134 | 93.3% |
| `image-filter` | `pillow-rs/src/ops/param_filters.rs` | 505/533 | 94.7% |
| `image-filter` | `pillow-rs-py/python/pillow_rs/imagefilter.py` | 0/0 | n/a |
| `image-font` | `pillow-rs/src/lib.rs` | 177/323 | 54.8% |
| `image-font` | `pillow-rs/src/font/mod.rs` | 347/476 | 72.9% |
| `image-font` | `pillow-rs/src/font/pilfont.rs` | 556/614 | 90.6% |
| `image-font` | `pillow-rs/src/font/imagingft.rs` | 1378/1495 | 92.2% |
| `image-font` | `pillow-rs-py/python/pillow_rs/imagefont.py` | 0/0 | n/a |
| `image-ops` | `pillow-rs/src/ops/imageops.rs` | 570/661 | 86.2% |
| `image-ops` | `pillow-rs-py/python/pillow_rs/imageops.py` | 0/0 | n/a |
| `image-palette` | `pillow-rs/src/color.rs` | 1150/1226 | 93.8% |
| `image-palette` | `pillow-rs-py/python/pillow_rs/imagepalette.py` | 0/0 | n/a |
| `image-sequence` | `pillow-rs/src/image.rs` | 4041/4648 | 86.9% |
| `image-sequence` | `pillow-rs-py/python/pillow_rs/imagesequence.py` | 0/0 | n/a |

