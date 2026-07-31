# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: abb25ed0681baf2d7404711e52bfadb954c2b449d7eec4e6facfce7b76f8201d
coverage_run_id: migration-coverage-1f94ccb1b19046369fbf7781128dd58f
coverage_target_profile: python-cpu
metric: region
threshold: below 90%
```

Each operation's coverage is the region coverage of the files declared
by its coverage component(s); operations inside one component share the
component's measured coverage by design.

## PIL.Image.Image.getbbox

`PIL.Image.Image.getbbox -> region coverage 5656/7052 (80.2%)`

## Operations below 90% region coverage

119 of 204 coverage-required operations are below 90%.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageSequence.Iterator` | `image-sequence` | 2416/3425 | 70.5% |
| `PIL.ImageDraw.Draw` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.arc` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.bitmap` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.chord` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.circle` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.ellipse` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.getfont` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.line` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.multiline_text` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.multiline_textbbox` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.pieslice` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.point` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.polygon` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.rectangle` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.regular_polygon` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.rounded_rectangle` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.shape` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.text` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.textbbox` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.ImageDraw.textlength` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.ImageDraw.Outline` | `image-draw` | 1718/2164 | 79.4% |
| `PIL.Image.Image.alpha_composite` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.apply_transparency` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.close` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.convert` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.copy` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.crop` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.draft` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.effect_spread` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.entropy` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.filter` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.format` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.frombytes` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.get_child_images` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.get_flattened_data` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getbands` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getbbox` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getchannel` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getcolors` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getdata` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getexif` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getextrema` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getim` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getpalette` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getpixel` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getprojection` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.getxmp` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.height` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.histogram` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.info` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.load` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.mode` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.paste` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.point` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.putalpha` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.putdata` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.putpalette` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.putpixel` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.quantize` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.reduce` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.remap_palette` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.resize` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.rotate` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.save` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.seek` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.show` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.size` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.split` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.tell` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.thumbnail` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.tobitmap` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.tobytes` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.toqimage` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.toqpixmap` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.transform` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.transpose` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.verify` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.Image.width` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.alpha_composite` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.blend` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.composite` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.effect_mandelbrot` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.effect_noise` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.eval` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.fromarray` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.frombuffer` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.frombytes` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.linear_gradient` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.merge` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.new` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.open` | `image-core` | 5656/7052 | 80.2% |
| `PIL.Image.radial_gradient` | `image-core` | 5656/7052 | 80.2% |
| `PIL.ImageFont.FreeTypeFont` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.font_variant` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.get_variation_axes` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.get_variation_names` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.getbbox` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.getlength` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.getmask` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.getmask2` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.getmetrics` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.getname` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_axes` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_name` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.ImageFont` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.ImageFont.getbbox` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.ImageFont.getlength` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.ImageFont.getmask` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.MAX_STRING_LENGTH` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.TransposedFont` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.TransposedFont.getbbox` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.TransposedFont.getlength` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.TransposedFont.getmask` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.load` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.load_default` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.load_default_imagefont` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.load_path` | `image-font` | 2212/2514 | 88.0% |
| `PIL.ImageFont.truetype` | `image-font` | 2212/2514 | 88.0% |

## Per-file region coverage for involved components

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-core` | `pillow-rs/src/lib.rs` | 99/217 | 45.6% |
| `image-core` | `pillow-rs/src/ops/crop.rs` | 18/36 | 50.0% |
| `image-core` | `pillow-rs/src/image.rs` | 2416/3425 | 70.5% |
| `image-core` | `pillow-rs/src/ops/paste.rs` | 280/362 | 77.3% |
| `image-core` | `pillow-rs/src/ops/resize.rs` | 62/75 | 82.7% |
| `image-core` | `pillow-rs/src/pipeline.rs` | 32/38 | 84.2% |
| `image-core` | `pillow-rs/src/ops/split.rs` | 22/26 | 84.6% |
| `image-core` | `pillow-rs/src/ops/module_fns.rs` | 365/411 | 88.8% |
| `image-core` | `pillow-rs/src/ops/convert.rs` | 444/490 | 90.6% |
| `image-core` | `pillow-rs/src/ops/transpose.rs` | 25/26 | 96.2% |
| `image-core` | `pillow-rs/src/ops/transform.rs` | 56/58 | 96.6% |
| `image-core` | `pillow-rs/src/ops/quantize.rs` | 1829/1880 | 97.3% |
| `image-core` | `pillow-rs-py/python/pillow_rs/image.py` | 0/0 | n/a |
| `image-core` | `pillow-rs-py/python/pillow_rs/operations.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/ops/rotate.rs` | 8/8 | 100.0% |
| `image-draw` | `pillow-rs/src/draw/mod.rs` | 1718/2164 | 79.4% |
| `image-draw` | `pillow-rs-py/python/pillow_rs/imagedraw.py` | 0/0 | n/a |
| `image-font` | `pillow-rs/src/lib.rs` | 99/217 | 45.6% |
| `image-font` | `pillow-rs/src/font/mod.rs` | 169/271 | 62.4% |
| `image-font` | `pillow-rs/src/font/pilfont.rs` | 509/554 | 91.9% |
| `image-font` | `pillow-rs/src/font/imagingft.rs` | 1435/1472 | 97.5% |
| `image-font` | `pillow-rs-py/python/pillow_rs/imagefont.py` | 0/0 | n/a |
| `image-sequence` | `pillow-rs/src/image.rs` | 2416/3425 | 70.5% |
| `image-sequence` | `pillow-rs-py/python/pillow_rs/imagesequence.py` | 0/0 | n/a |

