# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@3
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: bb3bdd6e610a40da174b6ca342b13a147758fdcdd8d298c9aecc3b50a8d156bf
coverage_run_id: migration-coverage-0fbd3f43a79149feb2f02ee75b5bde80
coverage_target_profile: python-cpu
metric: region
threshold: below 95%
```

The operation table is a component aggregate used only to order the
backlog. Several public operations share a component, so these rows are
not operation-level coverage.

## PIL.Image.Image.getbbox

Scoped input-only evidence covers `38` getbbox cases (run `migration-coverage-f91bc6f9c206468b8dddda4bfd6c2344`).
Rust implementation regions: `104/104` (100.0%).
Python facade statements: `1/1` (100.0%).
Component aggregate for backlog ordering: `11366/12262` (92.7%).

## Operations below 95% region coverage

97 of 209 coverage-required operations are below 95%.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageFont.FreeTypeFont` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.font_variant` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.get_variation_axes` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.get_variation_names` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.getbbox` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.getlength` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.getmask` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.getmask2` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.getmetrics` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.getname` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_axes` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.FreeTypeFont.set_variation_by_name` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.ImageFont` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.ImageFont.getbbox` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.ImageFont.getlength` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.ImageFont.getmask` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.MAX_STRING_LENGTH` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.TransposedFont` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.TransposedFont.getbbox` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.TransposedFont.getlength` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.TransposedFont.getmask` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.load` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.load_default` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.load_default_imagefont` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.load_path` | `image-font` | 3361/3782 | 88.9% |
| `PIL.ImageFont.truetype` | `image-font` | 3361/3782 | 88.9% |
| `PIL.Image.Image.alpha_composite` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.apply_transparency` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.close` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.convert` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.copy` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.crop` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.draft` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.effect_spread` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.entropy` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.filter` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.format` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.frombytes` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.get_child_images` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.get_flattened_data` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getbands` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getbbox` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getchannel` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getcolors` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getdata` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getexif` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getextrema` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getim` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getpalette` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getpixel` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getprojection` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.getxmp` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.has_transparency_data` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.height` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.histogram` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.info` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.load` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.mode` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.paste` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.point` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.putalpha` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.putdata` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.putpalette` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.putpixel` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.quantize` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.reduce` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.remap_palette` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.resize` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.rotate` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.save` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.seek` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.size` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.split` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.tell` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.thumbnail` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.tobitmap` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.tobytes` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.toqimage` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.toqpixmap` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.transform` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.transpose` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.verify` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.Image.width` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.alpha_composite` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.blend` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.composite` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.effect_mandelbrot` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.effect_noise` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.eval` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.fromarray` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.frombuffer` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.frombytes` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.linear_gradient` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.merge` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.new` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.open` | `image-core` | 11366/12262 | 92.7% |
| `PIL.Image.radial_gradient` | `image-core` | 11366/12262 | 92.7% |

## Per-file region coverage for involved components

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-core` | `pillow-rs/src/ops/crop.rs` | 161/183 | 88.0% |
| `image-core` | `pillow-rs/src/image.rs` | 4719/5312 | 88.8% |
| `image-core` | `pillow-rs/src/pipeline.rs` | 35/39 | 89.7% |
| `image-core` | `pillow-rs/src/ops/paste.rs` | 798/879 | 90.8% |
| `image-core` | `pillow-rs/src/ops/rotate.rs` | 148/161 | 91.9% |
| `image-core` | `pillow-rs/src/ops/module_fns.rs` | 488/513 | 95.1% |
| `image-core` | `pillow-rs/src/ops/convert.rs` | 800/837 | 95.6% |
| `image-core` | `pillow-rs/src/ops/transform.rs` | 633/659 | 96.1% |
| `image-core` | `pillow-rs/src/ops/split.rs` | 25/26 | 96.2% |
| `image-core` | `pillow-rs/src/ops/quantize.rs` | 2809/2898 | 96.9% |
| `image-core` | `pillow-rs/src/ops/transpose.rs` | 64/65 | 98.5% |
| `image-core` | `pillow-rs/src/ops/analysis.rs` | 374/378 | 98.9% |
| `image-core` | `pillow-rs-py/python/pillow_rs/image.py` | 0/0 | n/a |
| `image-core` | `pillow-rs-py/python/pillow_rs/operations.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/ops/array.rs` | 88/88 | 100.0% |
| `image-core` | `pillow-rs/src/ops/resize.rs` | 224/224 | 100.0% |
| `image-font` | `pillow-rs/src/lib.rs` | 172/382 | 45.0% |
| `image-font` | `pillow-rs/src/font/pilfont.rs` | 576/628 | 91.7% |
| `image-font` | `pillow-rs/src/font/imagingft.rs` | 1954/2087 | 93.6% |
| `image-font` | `pillow-rs/src/font/mod.rs` | 659/685 | 96.2% |
| `image-font` | `pillow-rs-py/python/pillow_rs/imagefont.py` | 0/0 | n/a |
