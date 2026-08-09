# SIMD coverage backlog below 95%

Generated from the managed SIMD LLVM snapshot; this is execution coverage, not parity proof.

```yaml
snapshot_id: 784d5e4e-cbe3-47e8-b993-699187d70c09
suite: migration-parity-rust-simd
commit: ed50b7f987fa71486dd395b220cf3765af3e90b1
threshold: 95%
metric: regions
in_repo_files_below_threshold: 33
external_dependency_files_below_threshold: 43
```

The in-repository list is ordered from lowest to highest region coverage. 43 below-threshold files from the sibling `fontdone` dependency are intentionally not included in the actionable pillow-rs list.

| Rank | File | Regions | Region coverage | Lines |
| ---: | --- | ---: | ---: | ---: |
| 1 | `pillow-rs/src/compute/pool_cpu/ops/chops.rs` | 0/561 | 0.0% | 0/319 |
| 2 | `pillow-rs/src/compute/pool_cpu/ops/enhance.rs` | 0/398 | 0.0% | 0/216 |
| 3 | `pillow-rs/src/compute/pool_cpu/ops/filter.rs` | 0/959 | 0.0% | 0/568 |
| 4 | `pillow-rs/src/ops/utils.rs` | 0/65 | 0.0% | 0/48 |
| 5 | `pillow-rs/src/raster/traits/primitive.rs` | 0/225 | 0.0% | 0/132 |
| 6 | `pillow-rs/src/raster/traits/view.rs` | 0/25 | 0.0% | 0/24 |
| 7 | `pillow-rs/src/compute/pool_gpu/mod.rs` | 6/1718 | 0.3% | 6/1263 |
| 8 | `pillow-rs/src/compute/pool_cpu/ops/imageops.rs` | 69/961 | 7.2% | 42/519 |
| 9 | `pillow-rs/src/compute/pool_cpu/ops/color.rs` | 52/506 | 10.3% | 21/217 |
| 10 | `pillow-rs/src/ops/pil_resize.rs` | 149/1159 | 12.9% | 81/679 |
| 11 | `pillow-rs/src/raster/color/from_primitive.rs` | 16/80 | 20.0% | 8/43 |
| 12 | `pillow-rs/src/raster/dynamic.rs` | 337/1519 | 22.2% | 201/787 |
| 13 | `pillow-rs/src/compute/pool_cpu/ops/geometry.rs` | 444/1700 | 26.1% | 249/871 |
| 14 | `pillow-rs/src/compute/pool_cpu/ops/effects.rs` | 812/2649 | 30.7% | 441/1321 |
| 15 | `pillow-rs/src/compute/registry.rs` | 999/2235 | 44.7% | 704/1368 |
| 16 | `pillow-rs/src/raster/color/from_color.rs` | 85/190 | 44.7% | 49/102 |
| 17 | `pillow-rs/src/lib.rs` | 172/382 | 45.0% | 176/392 |
| 18 | `pillow-rs/src/error.rs` | 3/6 | 50.0% | 3/6 |
| 19 | `pillow-rs/src/raster/color/pixel_rgb.rs` | 48/78 | 61.5% | 27/48 |
| 20 | `pillow-rs-py/src/lib.rs` | 4620/5868 | 78.7% | 3092/3626 |
| 21 | `pillow-rs/src/checked_dims.rs` | 45/56 | 80.4% | 37/63 |
| 22 | `pillow-rs/src/compute/mod.rs` | 129/159 | 81.1% | 88/109 |
| 23 | `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 3966/4803 | 82.6% | 1964/2334 |
| 24 | `pillow-rs/src/color.rs` | 1061/1235 | 85.9% | 601/687 |
| 25 | `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 1649/1899 | 86.8% | 984/1121 |
| 26 | `pillow-rs/src/compute/pool_simd/mod.rs` | 93/105 | 88.6% | 59/67 |
| 27 | `pillow-rs/src/raster/color/pixel_luma.rs` | 54/60 | 90.0% | 32/38 |
| 28 | `pillow-rs/src/image.rs` | 4799/5291 | 90.7% | 3011/3263 |
| 29 | `pillow-rs/src/font/pilfont.rs` | 576/628 | 91.7% | 402/418 |
| 30 | `pillow-rs/src/ops/crop.rs` | 287/312 | 92.0% | 200/206 |
| 31 | `pillow-rs/src/ops/paste.rs` | 942/1010 | 93.3% | 474/487 |
| 32 | `pillow-rs/src/font/imagingft.rs` | 1954/2087 | 93.6% | 1274/1317 |
| 33 | `pillow-rs/src/raster/buffer.rs` | 272/290 | 93.8% | 184/202 |
