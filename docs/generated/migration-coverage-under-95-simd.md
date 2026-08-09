# SIMD coverage backlog below 95%

Generated from the local SIMD LLVM report at
`target/coverage/migration-parity-rust.json`; this is execution coverage, not
parity proof. Coverage MCP was unavailable (`Transport closed`) for this
refresh, so this report is local diagnostic evidence rather than a managed
snapshot.

```yaml
snapshot_id: null
provenance: local-llvm-report
suite: migration-parity-rust-simd
base_commit: 37a5609e012ad6aaa74075fc50af6abd92301181
threshold: 95%
metric: regions
total_regions: 105994
covered_regions: 61233
region_coverage: 57.7703%
total_lines: 67940
covered_lines: 39527
line_coverage: 58.1793%
total_branches: 13923
covered_branches: 6865
branch_coverage: 49.3069%
total_functions: 5276
covered_functions: 2995
function_coverage: 56.7665%
in_repo_files_below_threshold: 33
external_dependency_files_below_threshold: 43
```

The in-repository list is ordered from lowest to highest region coverage. The
43 below-threshold files from the sibling `fontdone` dependency are excluded
from the actionable pillow-rs list; they are an external-library backlog.

The fresh SIMD parity audit selected all 2,840 cases: 2,783 passed, 57 had
ordinary parity mismatches, and 0 had infrastructure errors. The mismatches
remain visible in the parity result; they are not removed from the coverage
denominator.

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
| 10 | `pillow-rs/src/raster/color/from_primitive.rs` | 16/80 | 20.0% | 8/43 |
| 11 | `pillow-rs/src/raster/dynamic.rs` | 337/1519 | 22.2% | 201/787 |
| 12 | `pillow-rs/src/compute/pool_cpu/ops/effects.rs` | 990/2649 | 37.4% | 532/1321 |
| 13 | `pillow-rs/src/compute/registry.rs` | 999/2235 | 44.7% | 704/1368 |
| 14 | `pillow-rs/src/raster/color/from_color.rs` | 85/190 | 44.7% | 49/102 |
| 15 | `pillow-rs/src/lib.rs` | 172/382 | 45.0% | 176/392 |
| 16 | `pillow-rs/src/error.rs` | 3/6 | 50.0% | 3/6 |
| 17 | `pillow-rs/src/raster/color/pixel_rgb.rs` | 48/78 | 61.5% | 27/48 |
| 18 | `pillow-rs-py/src/lib.rs` | 4619/5868 | 78.7% | 3092/3626 |
| 19 | `pillow-rs/src/checked_dims.rs` | 45/56 | 80.4% | 37/63 |
| 20 | `pillow-rs/src/compute/mod.rs` | 129/159 | 81.1% | 88/109 |
| 21 | `pillow-rs/src/compute/pool_cpu/ops/geometry.rs` | 1394/1700 | 82.0% | 730/871 |
| 22 | `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 4281/5113 | 83.7% | 2144/2529 |
| 23 | `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 1889/2220 | 85.1% | 1068/1222 |
| 24 | `pillow-rs/src/color.rs` | 1061/1235 | 85.9% | 601/687 |
| 25 | `pillow-rs/src/ops/pil_resize.rs` | 1020/1159 | 88.0% | 613/679 |
| 26 | `pillow-rs/src/compute/pool_simd/mod.rs` | 93/105 | 88.6% | 59/67 |
| 27 | `pillow-rs/src/raster/color/pixel_luma.rs` | 54/60 | 90.0% | 32/38 |
| 28 | `pillow-rs/src/image.rs` | 4797/5291 | 90.7% | 3007/3263 |
| 29 | `pillow-rs/src/font/pilfont.rs` | 576/628 | 91.7% | 402/418 |
| 30 | `pillow-rs/src/ops/crop.rs` | 287/312 | 92.0% | 200/206 |
| 31 | `pillow-rs/src/ops/paste.rs` | 942/1010 | 93.3% | 474/487 |
| 32 | `pillow-rs/src/font/imagingft.rs` | 1954/2087 | 93.6% | 1274/1317 |
| 33 | `pillow-rs/src/raster/buffer.rs` | 272/290 | 93.8% | 184/202 |
