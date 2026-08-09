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
base_commit: 48cff96276dc39e58b4ac75a1053acd6b8a12965
coverage_run_id: migration-coverage-b0fcbf35a2ba45c99b1b879e63a27c6f
parity_run_id: migration-parity-50e5ed41bbaa4fdfa1d973b5e0f092fe
source_dirty_at_collection: true
threshold: 95%
metric: regions
total_regions: 106090
covered_regions: 62595
region_coverage: 59.0018%
total_lines: 68112
covered_lines: 40280
line_coverage: 59.1379%
total_branches: 13955
covered_branches: 6990
branch_coverage: 50.0896%
total_functions: 5281
covered_functions: 3021
function_coverage: 57.2051%
simd_impl_regions: 6812/7480 (91.0695%)
simd_impl_lines: 3608/3939 (91.5969%)
simd_impl_branches: 712/960 (74.1667%)
simd_impl_functions: 185/204 (90.6863%)
in_repo_files_below_threshold: 33
external_dependency_files_below_threshold: 43
```

The in-repository list is ordered from lowest to highest region coverage. The
43 below-threshold files from the sibling `fontdone` dependency are excluded
from the actionable pillow-rs list; they are an external-library backlog.

The fresh SIMD parity audit selected all 2,856 cases: 2,853 passed, 3 had
ordinary parity mismatches, and 0 had infrastructure errors. The coverage
workflow executed all 24 plans and passed all 2,856 execution checks. The
mismatches remain visible in the parity result; they are not removed from the
coverage denominator. The added public case
`PIL.Image.Image.thumbnail.nuanced.rgb-nearest-simd-path` passes source/target
parity and exercises the RGB nearest-neighbor SIMD thumbnail path. Earlier
work also fixed SIMD `Image.merge` handling for a public palette-first band
case: Pillow consumes that first `P` band as raw one-byte samples, while the
previous SIMD path expanded the palette and then collapsed the multi-band
result back to `P`. The three remaining mismatches are the two known 16-bit
`PIL.Image.Image.paste` inputs (`opened-i16-scalar` and `opened-i16n-scalar`),
which remain pending for the TIFF/16-bit lane, and
`PIL.ImageFont.FreeTypeFont.set_variation_by_axes`, which is tracked in the
separate fontdone parity lane.

The higher-order ImageOps resize filters and Gaussian blur now use the shared
exact pure-Rust CPU implementation because the packed SIMD kernels only
implement nearest/bilinear resize and an approximate integer-radius blur.
This removes false parity from the SIMD lane while keeping those operations
covered and the unsupported SIMD approximation out of production results.

SIMD implementation-file coverage is:

| File | Regions | Lines | Branches | Functions |
| --- | ---: | ---: | ---: | ---: |
| `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 1974/2295 (86.01%) | 1118/1269 (88.10%) | 103/164 (62.80%) | 75/85 (88.24%) |
| `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 4741/5076 (93.40%) | 2428/2600 (93.38%) | 607/794 (76.45%) | 101/106 (95.28%) |
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
| 22 | `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 1974/2295 | 86.0% | 1118/1269 |
| 23 | `pillow-rs/src/ops/pil_resize.rs` | 1020/1159 | 88.0% | 613/679 |
| 24 | `pillow-rs/src/compute/pool_simd/mod.rs` | 97/109 | 89.0% | 62/70 |
| 25 | `pillow-rs/src/raster/color/pixel_luma.rs` | 54/60 | 90.0% | 32/38 |
| 26 | `pillow-rs/src/image.rs` | 4797/5291 | 90.7% | 3007/3263 |
| 27 | `pillow-rs/src/ops/transform.rs` | 644/703 | 91.6% | 459/507 |
| 28 | `pillow-rs/src/font/pilfont.rs` | 576/628 | 91.7% | 402/418 |
| 29 | `pillow-rs/src/ops/crop.rs` | 287/312 | 92.0% | 200/206 |
| 30 | `pillow-rs/src/ops/paste.rs` | 942/1010 | 93.3% | 474/487 |
| 31 | `pillow-rs/src/font/imagingft.rs` | 1954/2087 | 93.6% | 1274/1317 |
| 32 | `pillow-rs/src/raster/buffer.rs` | 272/290 | 93.8% | 184/202 |
| 33 | `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 4741/5076 | 93.4% | 2428/2600 |
