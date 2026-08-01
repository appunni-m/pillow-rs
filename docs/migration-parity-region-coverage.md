# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 870b317cc3c9f4231b16d8bf58a84ecc1207c7bee2ade211200528de6120074d
coverage_run_id: migration-coverage-d7838cf055ac42a4be723beee4cb932d
coverage_target_profile: python-cpu
metric: region
threshold: below 90%
```

Each operation's coverage is the region coverage of the files declared
by its coverage component(s); operations inside one component share the
component's measured coverage by design.

## PIL.Image.Image.getbbox

`PIL.Image.Image.getbbox -> region coverage 6567/8258 (79.5%)`

## Operations below 90% region coverage

137 of 204 coverage-required operations are below 90%.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageSequence.Iterator` | `image-sequence` | 2505/3599 | 69.6% |
| `PIL.Image.Image.alpha_composite` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.apply_transparency` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.close` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.convert` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.copy` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.crop` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.draft` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.effect_spread` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.entropy` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.filter` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.format` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.frombytes` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.get_child_images` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.get_flattened_data` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getbands` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getbbox` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getchannel` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getcolors` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getdata` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getexif` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getextrema` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getim` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getpalette` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getpixel` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getprojection` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.getxmp` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.height` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.histogram` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.info` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.load` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.mode` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.paste` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.point` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.putalpha` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.putdata` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.putpalette` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.putpixel` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.quantize` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.reduce` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.remap_palette` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.resize` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.rotate` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.save` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.seek` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.show` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.size` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.split` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.tell` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.thumbnail` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.tobitmap` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.tobytes` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.toqimage` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.toqpixmap` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.transform` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.transpose` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.verify` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.Image.width` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.alpha_composite` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.blend` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.composite` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.effect_mandelbrot` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.effect_noise` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.eval` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.fromarray` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.frombuffer` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.frombytes` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.linear_gradient` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.merge` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.new` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.open` | `image-core` | 6567/8258 | 79.5% |
| `PIL.Image.radial_gradient` | `image-core` | 6567/8258 | 79.5% |
| `PIL.ImageColor.getcolor` | `image-color` | 953/1138 | 83.7% |
| `PIL.ImageColor.getrgb` | `image-color` | 953/1138 | 83.7% |
| `PIL.ImagePalette.ImagePalette` | `image-palette` | 953/1138 | 83.7% |
| `PIL.ImagePalette.ImagePalette.copy` | `image-palette` | 953/1138 | 83.7% |
| `PIL.ImagePalette.ImagePalette.getcolor` | `image-palette` | 953/1138 | 83.7% |
| `PIL.ImagePalette.ImagePalette.getdata` | `image-palette` | 953/1138 | 83.7% |
| `PIL.ImagePalette.ImagePalette.save` | `image-palette` | 953/1138 | 83.7% |
| `PIL.ImagePalette.ImagePalette.tobytes` | `image-palette` | 953/1138 | 83.7% |
| `PIL.ImageDraw.Draw` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.arc` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.bitmap` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.chord` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.circle` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.ellipse` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.getfont` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.line` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.multiline_text` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.multiline_textbbox` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.pieslice` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.point` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.polygon` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.rectangle` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.regular_polygon` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.rounded_rectangle` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.shape` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.text` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.textbbox` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.ImageDraw.textlength` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageDraw.Outline` | `image-draw` | 1851/2164 | 85.5% |
| `PIL.ImageStat.Stat` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageStat.Stat.count` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageStat.Stat.extrema` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageStat.Stat.mean` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageStat.Stat.median` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageStat.Stat.rms` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageStat.Stat.stddev` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageStat.Stat.sum` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageStat.Stat.sum2` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageStat.Stat.var` | `image-stat` | 238/274 | 86.9% |
| `PIL.ImageFont.FreeTypeFont` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.font_variant` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.get_variation_axes` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.get_variation_names` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.getbbox` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.getlength` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.getmask` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.getmask2` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.getmetrics` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.getname` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_axes` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_name` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.ImageFont` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.ImageFont.getbbox` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.ImageFont.getlength` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.ImageFont.getmask` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.MAX_STRING_LENGTH` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.TransposedFont` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.TransposedFont.getbbox` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.TransposedFont.getlength` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.TransposedFont.getmask` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.load` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.load_default` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.load_default_imagefont` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.load_path` | `image-font` | 2215/2514 | 88.1% |
| `PIL.ImageFont.truetype` | `image-font` | 2215/2514 | 88.1% |

## Per-file region coverage for involved components

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-color` | `pillow-rs/src/color.rs` | 953/1138 | 83.7% |
| `image-color` | `pillow-rs-py/python/pillow_rs/imagecolor.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/lib.rs` | 99/217 | 45.6% |
| `image-core` | `pillow-rs/src/ops/crop.rs` | 18/36 | 50.0% |
| `image-core` | `pillow-rs/src/image.rs` | 2505/3599 | 69.6% |
| `image-core` | `pillow-rs/src/ops/paste.rs` | 312/377 | 82.8% |
| `image-core` | `pillow-rs/src/pipeline.rs` | 32/38 | 84.2% |
| `image-core` | `pillow-rs/src/ops/split.rs` | 22/26 | 84.6% |
| `image-core` | `pillow-rs/src/ops/analysis.rs` | 238/274 | 86.9% |
| `image-core` | `pillow-rs/src/ops/quantize.rs` | 2314/2576 | 89.8% |
| `image-core` | `pillow-rs/src/ops/resize.rs` | 68/75 | 90.7% |
| `image-core` | `pillow-rs/src/ops/convert.rs` | 480/528 | 90.9% |
| `image-core` | `pillow-rs/src/ops/module_fns.rs` | 389/419 | 92.8% |
| `image-core` | `pillow-rs/src/ops/transpose.rs` | 25/26 | 96.2% |
| `image-core` | `pillow-rs/src/ops/transform.rs` | 57/59 | 96.6% |
| `image-core` | `pillow-rs-py/python/pillow_rs/image.py` | 0/0 | n/a |
| `image-core` | `pillow-rs-py/python/pillow_rs/operations.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/ops/rotate.rs` | 8/8 | 100.0% |
| `image-draw` | `pillow-rs/src/draw/mod.rs` | 1851/2164 | 85.5% |
| `image-draw` | `pillow-rs-py/python/pillow_rs/imagedraw.py` | 0/0 | n/a |
| `image-font` | `pillow-rs/src/lib.rs` | 99/217 | 45.6% |
| `image-font` | `pillow-rs/src/font/mod.rs` | 169/271 | 62.4% |
| `image-font` | `pillow-rs/src/font/pilfont.rs` | 509/554 | 91.9% |
| `image-font` | `pillow-rs/src/font/imagingft.rs` | 1438/1472 | 97.7% |
| `image-font` | `pillow-rs-py/python/pillow_rs/imagefont.py` | 0/0 | n/a |
| `image-palette` | `pillow-rs/src/color.rs` | 953/1138 | 83.7% |
| `image-palette` | `pillow-rs-py/python/pillow_rs/imagepalette.py` | 0/0 | n/a |
| `image-sequence` | `pillow-rs/src/image.rs` | 2505/3599 | 69.6% |
| `image-sequence` | `pillow-rs-py/python/pillow_rs/imagesequence.py` | 0/0 | n/a |
| `image-stat` | `pillow-rs/src/ops/analysis.rs` | 238/274 | 86.9% |
| `image-stat` | `pillow-rs-py/python/pillow_rs/imagestat.py` | 0/0 | n/a |

