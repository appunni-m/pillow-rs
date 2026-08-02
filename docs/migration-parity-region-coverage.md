# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: e9793ed33f529f1a1eec6f858df78d545f461fd585a1548ad85e016e90cf3234
coverage_run_id: migration-coverage-88a1fd42f9f04cf79720f0d2872abe85
coverage_target_profile: python-cpu
metric: region
threshold: below 90%
```

Each operation's coverage is the region coverage of the files declared
by its coverage component(s); operations inside one component share the
component's measured coverage by design.

## PIL.Image.Image.getbbox

`PIL.Image.Image.getbbox -> region coverage 7822/8709 (89.8%)`

## Operations below 90% region coverage

73 of 205 coverage-required operations are below 90%.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageSequence.Iterator` | `image-sequence` | 3420/3939 | 86.8% |
| `PIL.ImageSequence.Iterator.__iter__` | `image-sequence` | 3420/3939 | 86.8% |
| `PIL.ImageSequence.Iterator.__next__` | `image-sequence` | 3420/3939 | 86.8% |
| `PIL.Image.Image.alpha_composite` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.apply_transparency` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.close` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.convert` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.copy` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.crop` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.draft` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.effect_spread` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.entropy` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.filter` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.format` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.frombytes` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.get_child_images` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.get_flattened_data` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getbands` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getbbox` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getchannel` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getcolors` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getdata` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getexif` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getextrema` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getim` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getpalette` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getpixel` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getprojection` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.getxmp` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.height` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.histogram` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.info` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.load` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.mode` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.paste` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.point` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.putalpha` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.putdata` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.putpalette` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.putpixel` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.quantize` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.reduce` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.remap_palette` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.resize` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.rotate` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.save` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.seek` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.size` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.split` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.tell` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.thumbnail` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.tobitmap` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.tobytes` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.toqimage` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.toqpixmap` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.transform` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.transpose` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.verify` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.Image.width` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.alpha_composite` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.blend` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.composite` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.effect_mandelbrot` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.effect_noise` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.eval` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.fromarray` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.frombuffer` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.frombytes` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.linear_gradient` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.merge` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.new` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.open` | `image-core` | 7822/8709 | 89.8% |
| `PIL.Image.radial_gradient` | `image-core` | 7822/8709 | 89.8% |

## Per-file region coverage for involved components

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-core` | `pillow-rs/src/pipeline.rs` | 32/38 | 84.2% |
| `image-core` | `pillow-rs/src/image.rs` | 3420/3939 | 86.8% |
| `image-core` | `pillow-rs/src/ops/paste.rs` | 386/430 | 89.8% |
| `image-core` | `pillow-rs/src/ops/convert.rs` | 652/717 | 90.9% |
| `image-core` | `pillow-rs/src/ops/resize.rs` | 69/75 | 92.0% |
| `image-core` | `pillow-rs/src/ops/quantize.rs` | 2446/2656 | 92.1% |
| `image-core` | `pillow-rs/src/ops/module_fns.rs` | 399/425 | 93.9% |
| `image-core` | `pillow-rs/src/ops/crop.rs` | 34/36 | 94.4% |
| `image-core` | `pillow-rs/src/ops/split.rs` | 25/26 | 96.2% |
| `image-core` | `pillow-rs/src/ops/transpose.rs` | 25/26 | 96.2% |
| `image-core` | `pillow-rs/src/ops/transform.rs` | 57/59 | 96.6% |
| `image-core` | `pillow-rs/src/ops/analysis.rs` | 269/274 | 98.2% |
| `image-core` | `pillow-rs-py/python/pillow_rs/image.py` | 0/0 | n/a |
| `image-core` | `pillow-rs-py/python/pillow_rs/operations.py` | 0/0 | n/a |
| `image-core` | `pillow-rs/src/ops/rotate.rs` | 8/8 | 100.0% |
| `image-sequence` | `pillow-rs/src/image.rs` | 3420/3939 | 86.8% |
| `image-sequence` | `pillow-rs-py/python/pillow_rs/imagesequence.py` | 0/0 | n/a |

