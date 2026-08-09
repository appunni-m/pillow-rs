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
base_commit: d5594a8b1fc1bf206ae5b3da9d8d2d2100ee0417
coverage_run_id: migration-coverage-b2437ed0b17046f99469af04b2110d40
parity_run_id: migration-parity-20bb7c610ca148978777fbacf1e780ca
threshold: 95%
metric: regions
total_regions: 106081
covered_regions: 62035
region_coverage: 58.4789%
total_lines: 68018
covered_lines: 39898
line_coverage: 58.6580%
total_branches: 13935
covered_branches: 6944
branch_coverage: 49.8314%
total_functions: 5280
covered_functions: 3008
function_coverage: 56.9697%
simd_impl_regions: 6858/7471 (91.7949%)
simd_impl_lines: 3546/3845 (92.2237%)
simd_impl_branches: 718/940 (76.3830%)
simd_impl_functions: 184/203 (90.6404%)
in_repo_files_below_threshold: 33
external_dependency_files_below_threshold: 43
```

The in-repository list is ordered from lowest to highest region coverage. The
43 below-threshold files from the sibling `fontdone` dependency are excluded
from the actionable pillow-rs list; they are an external-library backlog.

The fresh SIMD parity audit selected all 2,855 cases: 2,812 passed, 43 had
ordinary parity mismatches, and 0 had infrastructure errors. The coverage
workflow executed all 24 plans and passed all 2,855 execution checks. The
mismatches remain visible in the parity result; they are not removed from the
coverage denominator. This pass fixed SIMD `Image.merge` handling for a public
palette-first band case: Pillow consumes that first `P` band as raw one-byte
samples, while the previous SIMD path expanded the palette and then collapsed
the multi-band result back to `P`.

SIMD implementation-file coverage is:

| File | Regions | Lines | Branches | Functions |
| --- | ---: | ---: | ---: | ---: |
| `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 1938/2239 (86.56%) | 1083/1228 (88.19%) | 89/144 (61.81%) | 73/83 (87.95%) |
| `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 4823/5123 (94.14%) | 2401/2547 (94.27%) | 627/794 (78.97%) | 102/107 (95.33%) |
| `pillow-rs/src/compute/pool_simd/mod.rs` | 97/109 (88.99%) | 62/70 (88.57%) | 2/2 (100.00%) | 9/13 (69.23%) |

| Rank | File | Regions | Region coverage | Lines |
| ---: | --- | ---: | ---: | ---: |
| 1 | `pillow-rs/src/compute/pool_cpu/ops/chops.rs` | 0/561 | 0.0% | 0/319 |
| 2 | `pillow-rs/src/compute/pool_cpu/ops/enhance.rs` | 0/398 | 0.0% | 0/216 |
| 3 | `pillow-rs/src/compute/pool_cpu/ops/filter.rs` | 0/959 | 0.0% | 0/568 |
| 4 | `pillow-rs/src/ops/utils.rs` | 0/65 | 0.0% | 0/48 |
| 5 | `pillow-rs/src/raster/traits/primitive.rs` | 0/225 | 0.0% | 0/132 |
| 6 | `pillow-rs/src/raster/traits/view.rs` | 0/25 | 0.0% | 0/24 |
| 7 | `pillow-rs/src/compute/pool_gpu/mod.rs` | 6/1718 | 0.3% | 6/1263 |
| 8 | `pillow-rs/src/compute/pool_cpu/ops/imageops.rs` | 69/965 | 7.2% | 42/522 |
| 9 | `pillow-rs/src/raster/color/from_primitive.rs` | 16/80 | 20.0% | 8/43 |
| 10 | `pillow-rs/src/raster/dynamic.rs` | 337/1519 | 22.2% | 201/787 |
| 11 | `pillow-rs/src/compute/pool_cpu/ops/color.rs` | 119/506 | 23.5% | 55/217 |
| 12 | `pillow-rs/src/compute/pool_cpu/ops/effects.rs` | 990/2649 | 37.4% | 532/1324 |
| 13 | `pillow-rs/src/compute/registry.rs` | 999/2235 | 44.7% | 704/1368 |
| 14 | `pillow-rs/src/raster/color/from_color.rs` | 85/190 | 44.7% | 49/102 |
| 15 | `pillow-rs/src/lib.rs` | 172/382 | 45.0% | 176/392 |
| 16 | `pillow-rs/src/error.rs` | 3/6 | 50.0% | 3/6 |
| 17 | `pillow-rs/src/raster/color/pixel_rgb.rs` | 48/78 | 61.5% | 27/48 |
| 18 | `pillow-rs-py/src/lib.rs` | 4619/5868 | 78.7% | 3092/3626 |
| 19 | `pillow-rs/src/checked_dims.rs` | 45/56 | 80.4% | 37/63 |
| 20 | `pillow-rs/src/compute/mod.rs` | 129/159 | 81.1% | 88/109 |
| 21 | `pillow-rs/src/compute/pool_cpu/ops/geometry.rs` | 1394/1700 | 82.0% | 730/871 |
| 22 | `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 1935/2237 | 86.5% | 1087/1232 |
| 23 | `pillow-rs/src/ops/pil_resize.rs` | 1020/1159 | 88.0% | 613/679 |
| 24 | `pillow-rs/src/compute/pool_simd/mod.rs` | 93/105 | 88.6% | 59/67 |
| 25 | `pillow-rs/src/raster/color/pixel_luma.rs` | 54/60 | 90.0% | 32/38 |
| 26 | `pillow-rs/src/image.rs` | 4797/5291 | 90.7% | 3007/3263 |
| 27 | `pillow-rs/src/ops/transform.rs` | 644/703 | 91.6% | 459/507 |
| 28 | `pillow-rs/src/font/pilfont.rs` | 576/628 | 91.7% | 402/418 |
| 29 | `pillow-rs/src/ops/crop.rs` | 287/312 | 92.0% | 200/206 |
| 30 | `pillow-rs/src/ops/paste.rs` | 942/1010 | 93.3% | 474/487 |
| 31 | `pillow-rs/src/font/imagingft.rs` | 1954/2087 | 93.6% | 1274/1317 |
| 32 | `pillow-rs/src/raster/buffer.rs` | 272/290 | 93.8% | 184/202 |
| 33 | `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 4823/5123 | 94.1% | 2401/2547 |
