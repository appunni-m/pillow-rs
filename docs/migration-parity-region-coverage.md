# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@2
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 62e916cfee9bd3c231c2af4d63af289de44c0a04a744223e484db172aeb3d9eb
coverage_run_id: migration-coverage-5e9753ad64ef4c809c053aa45bb65d3d
coverage_target_profile: python-cpu
metric: region
threshold: below 95%
```

Each operation's coverage is the region coverage of the files declared
by its coverage component(s); operations inside one component share the
component's measured coverage by design.

## PIL.Image.Image.getbbox

`PIL.Image.Image.getbbox -> region coverage 7994/8878 (90.0%)`

## Operations below 95% region coverage

146 of 206 coverage-required operations are below 95%.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageSequence.Iterator` | `image-sequence` | 3562/4099 | 86.9% |
| `PIL.ImageSequence.Iterator.__iter__` | `image-sequence` | 3562/4099 | 86.9% |
| `PIL.ImageSequence.Iterator.__next__` | `image-sequence` | 3562/4099 | 86.9% |
| `PIL.Image.Image.alpha_composite` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.apply_transparency` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.close` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.convert` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.copy` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.crop` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.draft` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.effect_spread` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.entropy` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.filter` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.format` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.frombytes` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.get_child_images` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.get_flattened_data` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getbands` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getbbox` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getchannel` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getcolors` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getdata` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getexif` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getextrema` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getim` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getpalette` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getpixel` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getprojection` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.getxmp` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.height` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.histogram` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.info` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.load` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.mode` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.paste` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.point` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.putalpha` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.putdata` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.putpalette` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.putpixel` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.quantize` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.reduce` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.remap_palette` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.resize` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.rotate` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.save` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.seek` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.size` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.split` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.tell` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.thumbnail` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.tobitmap` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.tobytes` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.toqimage` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.toqpixmap` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.transform` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.transpose` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.verify` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.Image.width` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.alpha_composite` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.blend` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.composite` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.effect_mandelbrot` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.effect_noise` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.eval` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.fromarray` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.frombuffer` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.frombytes` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.linear_gradient` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.merge` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.new` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.open` | `image-core` | 7994/8878 | 90.0% |
| `PIL.Image.radial_gradient` | `image-core` | 7994/8878 | 90.0% |
| `PIL.ImageDraw.Draw` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.arc` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.bitmap` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.chord` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.circle` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.ellipse` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.getfont` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.line` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.multiline_text` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.multiline_textbbox` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.pieslice` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.point` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.polygon` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.rectangle` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.regular_polygon` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.rounded_rectangle` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.shape` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.text` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.textbbox` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.ImageDraw.textlength` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageDraw.Outline` | `image-draw` | 1991/2200 | 90.5% |
| `PIL.ImageOps.autocontrast` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.colorize` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.contain` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.cover` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.crop` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.deform` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.equalize` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.exif_transpose` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.expand` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.fit` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.flip` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.grayscale` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.invert` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.mirror` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.pad` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.posterize` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.scale` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageOps.solarize` | `image-ops` | 307/339 | 90.6% |
| `PIL.ImageColor.getcolor` | `image-color` | 1123/1207 | 93.0% |
| `PIL.ImageColor.getrgb` | `image-color` | 1123/1207 | 93.0% |
| `PIL.ImagePalette.ImagePalette` | `image-palette` | 1123/1207 | 93.0% |
| `PIL.ImagePalette.ImagePalette.copy` | `image-palette` | 1123/1207 | 93.0% |
| `PIL.ImagePalette.ImagePalette.getcolor` | `image-palette` | 1123/1207 | 93.0% |
| `PIL.ImagePalette.ImagePalette.getdata` | `image-palette` | 1123/1207 | 93.0% |
| `PIL.ImagePalette.ImagePalette.save` | `image-palette` | 1123/1207 | 93.0% |
| `PIL.ImagePalette.ImagePalette.tobytes` | `image-palette` | 1123/1207 | 93.0% |
| `PIL.ImageFont.FreeTypeFont` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.font_variant` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.get_variation_axes` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.get_variation_names` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.getbbox` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.getlength` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.getmask` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.getmask2` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.getmetrics` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.getname` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_axes` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_name` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.ImageFont` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.ImageFont.getbbox` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.ImageFont.getlength` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.ImageFont.getmask` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.MAX_STRING_LENGTH` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.TransposedFont` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.TransposedFont.getbbox` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.TransposedFont.getlength` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.TransposedFont.getmask` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.load` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.load_default` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.load_default_imagefont` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.load_path` | `image-font` | 2347/2511 | 93.5% |
| `PIL.ImageFont.truetype` | `image-font` | 2347/2511 | 93.5% |

## Per-file region coverage for involved components

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-color` | `pillow-rs/src/color.rs` | 1123/1207 | 93.0% |
| `image-color` | `pillow-rs-py/python/pillow_rs/imagecolor.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/pipeline.rs` | 32/38 | 84.2% |
| `image-core` | `pillow-rs/src/image.rs` | 3562/4099 | 86.9% |
| `image-core` | `pillow-rs/src/ops/paste.rs` | 390/433 | 90.1% |
| `image-core` | `pillow-rs/src/ops/quantize.rs` | 2446/2656 | 92.1% |
| `image-core` | `pillow-rs/src/ops/resize.rs` | 72/77 | 93.5% |
| `image-core` | `pillow-rs/src/ops/convert.rs` | 675/721 | 93.6% |
| `image-core` | `pillow-rs/src/ops/module_fns.rs` | 399/425 | 93.9% |
| `image-core` | `pillow-rs/src/ops/crop.rs` | 34/36 | 94.4% |
| `image-core` | `pillow-rs/src/ops/split.rs` | 25/26 | 96.2% |
| `image-core` | `pillow-rs/src/ops/transpose.rs` | 25/26 | 96.2% |
| `image-core` | `pillow-rs/src/ops/transform.rs` | 57/59 | 96.6% |
| `image-core` | `pillow-rs/src/ops/analysis.rs` | 269/274 | 98.2% |
| `image-core` | `pillow-rs-py/python/pillow_rs/image.py` | 0/0 | n/a |
| `image-core` | `pillow-rs-py/python/pillow_rs/operations.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/ops/rotate.rs` | 8/8 | 100.0% |
| `image-draw` | `pillow-rs/src/draw/mod.rs` | 1991/2200 | 90.5% |
| `image-draw` | `pillow-rs-py/python/pillow_rs/imagedraw.py` | 0/0 | n/a |
| `image-font` | `pillow-rs/src/lib.rs` | 189/217 | 87.1% |
| `image-font` | `pillow-rs/src/font/mod.rs` | 239/271 | 88.2% |
| `image-font` | `pillow-rs/src/font/pilfont.rs` | 510/554 | 92.1% |
| `image-font` | `pillow-rs/src/font/imagingft.rs` | 1409/1469 | 95.9% |
| `image-font` | `pillow-rs-py/python/pillow_rs/imagefont.py` | 0/0 | n/a |
| `image-ops` | `pillow-rs/src/ops/imageops.rs` | 307/339 | 90.6% |
| `image-ops` | `pillow-rs-py/python/pillow_rs/imageops.py` | 0/0 | n/a |
| `image-palette` | `pillow-rs/src/color.rs` | 1123/1207 | 93.0% |
| `image-palette` | `pillow-rs-py/python/pillow_rs/imagepalette.py` | 0/0 | n/a |
| `image-sequence` | `pillow-rs/src/image.rs` | 3562/4099 | 86.9% |
| `image-sequence` | `pillow-rs-py/python/pillow_rs/imagesequence.py` | 0/0 | n/a |

