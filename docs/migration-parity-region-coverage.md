# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: dd171a9c7823b663abcfb46c9953694a67e888448de5027682b84b963347517f
coverage_run_id: migration-coverage-e398c63d25874110a37f7fc636f3682a
coverage_target_profile: python-cpu
metric: region
threshold: below 90%
```

Each operation's coverage is the region coverage of the files declared
by its coverage component(s); operations inside one component share the
component's measured coverage by design.

## PIL.Image.Image.getbbox

`PIL.Image.Image.getbbox -> region coverage 4034/6940 (58.1%)`

## Operations below 90% region coverage

165 of 204 coverage-required operations are below 90%.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageOps.autocontrast` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.colorize` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.contain` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.cover` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.crop` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.deform` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.equalize` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.exif_transpose` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.expand` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.fit` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.flip` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.grayscale` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.invert` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.mirror` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.pad` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.posterize` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.scale` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageOps.solarize` | `image-ops` | 134/323 | 41.5% |
| `PIL.ImageFont.FreeTypeFont` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.font_variant` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.get_variation_axes` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.get_variation_names` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.getbbox` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.getlength` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.getmask` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.getmask2` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.getmetrics` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.getname` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_axes` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_name` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.ImageFont` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.ImageFont.getbbox` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.ImageFont.getlength` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.ImageFont.getmask` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.MAX_STRING_LENGTH` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.TransposedFont` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.TransposedFont.getbbox` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.TransposedFont.getlength` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.TransposedFont.getmask` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.load` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.load_default` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.load_default_imagefont` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.load_path` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageFont.truetype` | `image-font` | 1147/2486 | 46.1% |
| `PIL.ImageSequence.Iterator` | `image-sequence` | 1856/3319 | 55.9% |
| `PIL.ImageDraw.Draw` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.arc` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.bitmap` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.chord` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.circle` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.ellipse` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.getfont` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.line` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.multiline_text` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.multiline_textbbox` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.pieslice` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.point` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.polygon` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.rectangle` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.regular_polygon` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.rounded_rectangle` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.shape` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.text` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.textbbox` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.ImageDraw.textlength` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.ImageDraw.Outline` | `image-draw` | 1184/2101 | 56.4% |
| `PIL.Image.Image.alpha_composite` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.apply_transparency` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.close` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.convert` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.copy` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.crop` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.draft` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.effect_spread` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.entropy` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.filter` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.format` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.frombytes` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.get_child_images` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.get_flattened_data` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getbands` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getbbox` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getchannel` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getcolors` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getdata` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getexif` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getextrema` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getim` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getpalette` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getpixel` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getprojection` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.getxmp` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.height` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.histogram` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.info` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.load` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.mode` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.paste` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.point` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.putalpha` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.putdata` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.putpalette` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.putpixel` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.quantize` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.reduce` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.remap_palette` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.resize` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.rotate` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.save` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.seek` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.show` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.size` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.split` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.tell` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.thumbnail` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.tobitmap` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.tobytes` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.toqimage` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.toqpixmap` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.transform` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.transpose` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.verify` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.Image.width` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.alpha_composite` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.blend` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.composite` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.effect_mandelbrot` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.effect_noise` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.eval` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.fromarray` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.frombuffer` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.frombytes` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.linear_gradient` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.merge` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.new` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.open` | `image-core` | 4034/6940 | 58.1% |
| `PIL.Image.radial_gradient` | `image-core` | 4034/6940 | 58.1% |
| `PIL.ImageFilter.BLUR` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.BoxBlur` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.CONTOUR` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.Color3DLUT` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.DETAIL` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.EDGE_ENHANCE` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.EDGE_ENHANCE_MORE` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.EMBOSS` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.FIND_EDGES` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.GaussianBlur` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.Kernel` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.MaxFilter` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.MedianFilter` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.MinFilter` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.ModeFilter` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.RankFilter` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.SHARPEN` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.SMOOTH` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.SMOOTH_MORE` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageFilter.UnsharpMask` | `image-filter` | 59/96 | 61.5% |
| `PIL.ImageColor.getcolor` | `image-color` | 614/929 | 66.1% |
| `PIL.ImageColor.getrgb` | `image-color` | 614/929 | 66.1% |
| `PIL.ImagePalette.ImagePalette` | `image-palette` | 614/929 | 66.1% |
| `PIL.ImagePalette.ImagePalette.copy` | `image-palette` | 614/929 | 66.1% |
| `PIL.ImagePalette.ImagePalette.getcolor` | `image-palette` | 614/929 | 66.1% |
| `PIL.ImagePalette.ImagePalette.getdata` | `image-palette` | 614/929 | 66.1% |
| `PIL.ImagePalette.ImagePalette.save` | `image-palette` | 614/929 | 66.1% |
| `PIL.ImagePalette.ImagePalette.tobytes` | `image-palette` | 614/929 | 66.1% |

## Per-file region coverage for involved components

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-color` | `pillow-rs/src/color.rs` | 614/929 | 66.1% |
| `image-color` | `pillow-rs-py/python/pillow_rs/imagecolor.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/ops/transpose.rs` | 0/26 | 0.0% |
| `image-core` | `pillow-rs/src/ops/transform.rs` | 14/58 | 24.1% |
| `image-core` | `pillow-rs/src/lib.rs` | 60/217 | 27.6% |
| `image-core` | `pillow-rs/src/pipeline.rs` | 15/38 | 39.5% |
| `image-core` | `pillow-rs/src/ops/paste.rs` | 168/362 | 46.4% |
| `image-core` | `pillow-rs/src/ops/module_fns.rs` | 204/408 | 50.0% |
| `image-core` | `pillow-rs/src/ops/crop.rs` | 18/36 | 50.0% |
| `image-core` | `pillow-rs/src/image.rs` | 1856/3319 | 55.9% |
| `image-core` | `pillow-rs/src/ops/convert.rs` | 325/487 | 66.7% |
| `image-core` | `pillow-rs/src/ops/quantize.rs` | 1282/1880 | 68.2% |
| `image-core` | `pillow-rs/src/ops/resize.rs` | 62/75 | 82.7% |
| `image-core` | `pillow-rs/src/ops/split.rs` | 22/26 | 84.6% |
| `image-core` | `pillow-rs-py/python/pillow_rs/image.py` | 0/0 | n/a |
| `image-core` | `pillow-rs-py/python/pillow_rs/operations.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/ops/rotate.rs` | 8/8 | 100.0% |
| `image-draw` | `pillow-rs/src/draw/mod.rs` | 1184/2101 | 56.4% |
| `image-draw` | `pillow-rs-py/python/pillow_rs/imagedraw.py` | 0/0 | n/a |
| `image-filter` | `pillow-rs/src/ops/filter.rs` | 59/96 | 61.5% |
| `image-filter` | `pillow-rs-py/python/pillow_rs/imagefilter.py` | 0/0 | n/a |
| `image-font` | `pillow-rs/src/font/mod.rs` | 70/271 | 25.8% |
| `image-font` | `pillow-rs/src/font/pilfont.rs` | 146/554 | 26.4% |
| `image-font` | `pillow-rs/src/lib.rs` | 60/217 | 27.6% |
| `image-font` | `pillow-rs/src/font/imagingft.rs` | 871/1444 | 60.3% |
| `image-font` | `pillow-rs-py/python/pillow_rs/imagefont.py` | 0/0 | n/a |
| `image-ops` | `pillow-rs/src/ops/imageops.rs` | 134/323 | 41.5% |
| `image-ops` | `pillow-rs-py/python/pillow_rs/imageops.py` | 0/0 | n/a |
| `image-palette` | `pillow-rs/src/color.rs` | 614/929 | 66.1% |
| `image-palette` | `pillow-rs-py/python/pillow_rs/imagepalette.py` | 0/0 | n/a |
| `image-sequence` | `pillow-rs/src/image.rs` | 1856/3319 | 55.9% |
| `image-sequence` | `pillow-rs-py/python/pillow_rs/imagesequence.py` | 0/0 | n/a |

