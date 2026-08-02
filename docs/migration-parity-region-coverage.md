# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@2
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 62e916cfee9bd3c231c2af4d63af289de44c0a04a744223e484db172aeb3d9eb
coverage_run_id: migration-coverage-f0f4b5b789fe4381a9922bfe71ac89f4
coverage_target_profile: python-cpu
metric: region
threshold: below 95%
```

Each operation's coverage is the region coverage of the files declared
by its coverage component(s); operations inside one component share the
component's measured coverage by design.

## PIL.Image.Image.getbbox

`PIL.Image.Image.getbbox -> region coverage 9226/10480 (88.0%)`

## Operations below 95% region coverage

167 of 206 coverage-required operations are below 95%.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageFilter.BLUR` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.BoxBlur` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.CONTOUR` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.Color3DLUT` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.Color3DLUT.__repr__` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.DETAIL` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.EDGE_ENHANCE` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.EDGE_ENHANCE_MORE` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.EMBOSS` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.FIND_EDGES` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.GaussianBlur` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.Kernel` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.MaxFilter` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.MedianFilter` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.MinFilter` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.ModeFilter` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.RankFilter` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.SHARPEN` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.SMOOTH` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.SMOOTH_MORE` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFilter.UnsharpMask` | `image-filter` | 472/659 | 71.6% |
| `PIL.ImageFont.FreeTypeFont` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.font_variant` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.get_variation_axes` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.get_variation_names` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.getbbox` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.getlength` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.getmask` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.getmask2` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.getmetrics` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.getname` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_axes` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_name` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.ImageFont` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.ImageFont.getbbox` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.ImageFont.getlength` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.ImageFont.getmask` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.MAX_STRING_LENGTH` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.TransposedFont` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.TransposedFont.getbbox` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.TransposedFont.getlength` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.TransposedFont.getmask` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.load` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.load_default` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.load_default_imagefont` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.load_path` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageFont.truetype` | `image-font` | 2242/2630 | 85.2% |
| `PIL.ImageSequence.Iterator` | `image-sequence` | 3978/4637 | 85.8% |
| `PIL.ImageSequence.Iterator.__iter__` | `image-sequence` | 3978/4637 | 85.8% |
| `PIL.ImageSequence.Iterator.__next__` | `image-sequence` | 3978/4637 | 85.8% |
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
| `PIL.Image.Image.alpha_composite` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.apply_transparency` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.close` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.convert` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.copy` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.crop` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.draft` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.effect_spread` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.entropy` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.filter` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.format` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.frombytes` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.get_child_images` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.get_flattened_data` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getbands` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getbbox` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getchannel` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getcolors` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getdata` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getexif` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getextrema` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getim` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getpalette` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getpixel` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getprojection` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.getxmp` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.height` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.histogram` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.info` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.load` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.mode` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.paste` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.point` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.putalpha` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.putdata` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.putpalette` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.putpixel` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.quantize` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.reduce` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.remap_palette` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.resize` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.rotate` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.save` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.seek` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.size` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.split` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.tell` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.thumbnail` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.tobitmap` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.tobytes` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.toqimage` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.toqpixmap` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.transform` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.transpose` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.verify` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.Image.width` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.alpha_composite` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.blend` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.composite` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.effect_mandelbrot` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.effect_noise` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.eval` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.fromarray` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.frombuffer` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.frombytes` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.linear_gradient` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.merge` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.new` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.open` | `image-core` | 9226/10480 | 88.0% |
| `PIL.Image.radial_gradient` | `image-core` | 9226/10480 | 88.0% |
| `PIL.ImageDraw.Draw` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.arc` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.bitmap` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.chord` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.circle` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.ellipse` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.getfont` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.line` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.multiline_text` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.multiline_textbbox` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.pieslice` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.point` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.polygon` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.rectangle` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.regular_polygon` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.rounded_rectangle` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.shape` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.text` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.textbbox` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.ImageDraw.textlength` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageDraw.Outline` | `image-draw` | 2322/2634 | 88.2% |
| `PIL.ImageColor.getcolor` | `image-color` | 1150/1226 | 93.8% |
| `PIL.ImageColor.getrgb` | `image-color` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.copy` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.getcolor` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.getdata` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.save` | `image-palette` | 1150/1226 | 93.8% |
| `PIL.ImagePalette.ImagePalette.tobytes` | `image-palette` | 1150/1226 | 93.8% |

## Per-file region coverage for involved components

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-color` | `pillow-rs/src/color.rs` | 1150/1226 | 93.8% |
| `image-color` | `pillow-rs-py/python/pillow_rs/imagecolor.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/ops/transpose.rs` | 33/49 | 67.3% |
| `image-core` | `pillow-rs/src/ops/transform.rs` | 195/261 | 74.7% |
| `image-core` | `pillow-rs/src/ops/crop.rs` | 125/165 | 75.8% |
| `image-core` | `pillow-rs/src/ops/paste.rs` | 602/748 | 80.5% |
| `image-core` | `pillow-rs/src/pipeline.rs` | 32/38 | 84.2% |
| `image-core` | `pillow-rs/src/image.rs` | 3978/4637 | 85.8% |
| `image-core` | `pillow-rs/src/ops/quantize.rs` | 2483/2697 | 92.1% |
| `image-core` | `pillow-rs/src/ops/resize.rs` | 215/233 | 92.3% |
| `image-core` | `pillow-rs/src/ops/convert.rs` | 763/812 | 94.0% |
| `image-core` | `pillow-rs/src/ops/module_fns.rs` | 440/467 | 94.2% |
| `image-core` | `pillow-rs/src/ops/split.rs` | 25/26 | 96.2% |
| `image-core` | `pillow-rs/src/ops/analysis.rs` | 306/317 | 96.5% |
| `image-core` | `pillow-rs/src/ops/rotate.rs` | 29/30 | 96.7% |
| `image-core` | `pillow-rs-py/python/pillow_rs/image.py` | 0/0 | n/a |
| `image-core` | `pillow-rs-py/python/pillow_rs/operations.py` | 0/0 | n/a |
| `image-draw` | `pillow-rs/src/draw/mod.rs` | 2322/2634 | 88.2% |
| `image-draw` | `pillow-rs-py/python/pillow_rs/imagedraw.py` | 0/0 | n/a |
| `image-filter` | `pillow-rs/src/ops/param_filters.rs` | 347/525 | 66.1% |
| `image-filter` | `pillow-rs/src/ops/filter.rs` | 125/134 | 93.3% |
| `image-filter` | `pillow-rs-py/python/pillow_rs/imagefilter.py` | 0/0 | n/a |
| `image-font` | `pillow-rs/src/font/mod.rs` | 206/381 | 54.1% |
| `image-font` | `pillow-rs/src/lib.rs` | 146/226 | 64.6% |
| `image-font` | `pillow-rs/src/font/pilfont.rs` | 510/554 | 92.1% |
| `image-font` | `pillow-rs/src/font/imagingft.rs` | 1380/1469 | 93.9% |
| `image-font` | `pillow-rs-py/python/pillow_rs/imagefont.py` | 0/0 | n/a |
| `image-ops` | `pillow-rs/src/ops/imageops.rs` | 570/661 | 86.2% |
| `image-ops` | `pillow-rs-py/python/pillow_rs/imageops.py` | 0/0 | n/a |
| `image-palette` | `pillow-rs/src/color.rs` | 1150/1226 | 93.8% |
| `image-palette` | `pillow-rs-py/python/pillow_rs/imagepalette.py` | 0/0 | n/a |
| `image-sequence` | `pillow-rs/src/image.rs` | 3978/4637 | 85.8% |
| `image-sequence` | `pillow-rs-py/python/pillow_rs/imagesequence.py` | 0/0 | n/a |

