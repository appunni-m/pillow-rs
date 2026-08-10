# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@5
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 766c09c7df6556416ef5a9a3b4e51a2bf62704993c82fc40cb4a10b0b7530058
coverage_run_id: migration-coverage-0ff925f595fe40359be373ae99b6113b
coverage_target_profile: python-cpu
coverage_backend: simd
metric: region
threshold: below 95%
```

The operation table is a component aggregate used only to order the
backlog. Several public operations share a component, so these rows are
not operation-level coverage. The source-file table below is the
actionable file order and contains only files below the threshold.

## PIL.Image.Image.getbbox

Scoped input-only evidence covers `38` getbbox cases (run `migration-coverage-b085fa57e3c947d19116c91d24dcaa17`).
Rust implementation regions: `104/104` (100.0%).
Python facade statements: `1/1` (100.0%).
Component aggregate for backlog ordering: `13307/14425` (92.2%).

## Operations in below-95% components

120 of 209 coverage-required operations belong to a component below 95%; the percentages below are component aggregates.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageFilter.BLUR` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.BoxBlur` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.CONTOUR` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.Color3DLUT` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.Color3DLUT.__repr__` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.Color3DLUT.generate` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.Color3DLUT.transform` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.DETAIL` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.EDGE_ENHANCE` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.EDGE_ENHANCE_MORE` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.EMBOSS` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.FIND_EDGES` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.GaussianBlur` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.Kernel` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.MaxFilter` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.MedianFilter` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.MinFilter` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.ModeFilter` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.RankFilter` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.SHARPEN` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.SMOOTH` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.SMOOTH_MORE` | `image-filter` | 858/1665 | 51.5% |
| `PIL.ImageFilter.UnsharpMask` | `image-filter` | 858/1665 | 51.5% |
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
| `PIL.Image.Image.alpha_composite` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.apply_transparency` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.close` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.convert` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.copy` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.crop` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.draft` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.effect_spread` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.entropy` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.filter` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.format` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.frombytes` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.get_child_images` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.get_flattened_data` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getbands` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getbbox` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getchannel` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getcolors` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getdata` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getexif` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getextrema` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getim` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getpalette` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getpixel` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getprojection` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.getxmp` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.has_transparency_data` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.height` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.histogram` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.info` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.load` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.mode` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.paste` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.point` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.putalpha` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.putdata` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.putpalette` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.putpixel` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.quantize` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.reduce` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.remap_palette` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.resize` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.rotate` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.save` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.seek` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.size` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.split` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.tell` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.thumbnail` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.tobitmap` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.tobytes` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.toqimage` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.toqpixmap` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.transform` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.transpose` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.verify` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.Image.width` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.alpha_composite` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.blend` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.composite` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.effect_mandelbrot` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.effect_noise` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.eval` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.fromarray` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.frombuffer` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.frombytes` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.linear_gradient` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.merge` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.new` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.open` | `image-core` | 13307/14425 | 92.2% |
| `PIL.Image.radial_gradient` | `image-core` | 13307/14425 | 92.2% |

## Ordered below-95% source-file backlog

Sorted from lowest to highest region coverage, then by component and path.

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-filter` | `pillow-rs/src/compute/pool_cpu/ops/filter.rs` | 183/959 | 19.1% |
| `image-font` | `pillow-rs/src/lib.rs` | 172/382 | 45.0% |
| `image-core` | `pillow-rs/src/compute/pool_cpu/ops/geometry.rs` | 1422/1728 | 82.3% |
| `image-core` | `pillow-rs/src/image.rs` | 4797/5292 | 90.6% |
| `image-core` | `pillow-rs/src/ops/transform.rs` | 644/703 | 91.6% |
| `image-font` | `pillow-rs/src/font/pilfont.rs` | 576/628 | 91.7% |
| `image-core` | `pillow-rs/src/ops/crop.rs` | 287/312 | 92.0% |
| `image-core` | `pillow-rs/src/ops/paste.rs` | 942/1010 | 93.3% |
| `image-font` | `pillow-rs/src/font/imagingft.rs` | 1954/2087 | 93.6% |
