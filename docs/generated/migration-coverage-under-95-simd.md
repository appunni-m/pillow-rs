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
base_commit: 0129e8cf23523e826102beae2503bd7b1f2c97bb
coverage_run_id: migration-coverage-aca07b270a5146919030281a6422eee3
parity_run_id: migration-parity-f726b2df3ab94344938d84546797e00a
source_dirty_at_collection: true
threshold: 95%
metric: regions
total_regions: 106085
covered_regions: 62633
region_coverage: 59.0404%
total_lines: 68094
covered_lines: 40284
line_coverage: 59.1594%
total_branches: 13955
covered_branches: 7008
branch_coverage: 50.2330%
total_functions: 5281
covered_functions: 3021
function_coverage: 57.2051%
simd_impl_regions: 6852/7480 (91.6043%)
simd_impl_lines: 3612/3922 (92.0959%)
simd_impl_branches: 731/960 (76.1458%)
simd_impl_functions: 185/204 (90.6863%)
in_repo_files_below_threshold: 33
external_dependency_files_below_threshold: 43
```

The in-repository list is ordered from lowest to highest region coverage. The
43 below-threshold files from the sibling `fontdone` dependency are excluded
from the actionable pillow-rs list; they are an external-library backlog.

The fresh SIMD parity audit selected all 2,889 cases: 2,886 passed, 3 had
ordinary parity mismatches, and 0 had infrastructure errors. The coverage
workflow executed all 24 plans and passed all 2,889 execution checks. The
mismatches remain visible in the parity result; they are not removed from the
coverage denominator. This refresh adds two input-only public
`PIL.ImageChops.offset` cases for zero-width and zero-height images, reaching
the SIMD offset early-return branch; both cases pass source/target parity. The
previous SIMD batch added 21 input-only
nearest-neighbor and alternate/aspect cases covering `L`, `LA`, `RGBA`, `P`,
and `PA` ImageOps paths. The preceding refresh added 10 input-only bilinear cases for
`LA` and `RGBA` across `contain`, `cover`, `fit`, `scale`, and `pad`; all ten
new cases pass source/target parity. Earlier work also added
`PIL.Image.Image.thumbnail.nuanced.rgb-nearest-simd-path` and fixed SIMD
`Image.merge` handling for a public palette-first band case: Pillow consumes
that first `P` band as raw one-byte samples, while the previous SIMD path
expanded the palette and then collapsed the multi-band result back to `P`.
The three remaining mismatches are the two known 16-bit
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
| `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 1983/2295 (86.41%) | 1103/1252 (88.10%) | 105/164 (64.02%) | 75/85 (88.24%) |
| `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 4772/5076 (94.00%) | 2447/2600 (94.12%) | 624/794 (78.59%) | 101/106 (95.28%) |
| `pillow-rs/src/compute/pool_simd/mod.rs` | 97/109 (88.99%) | 62/70 (88.57%) | 2/2 (100.00%) | 9/13 (69.23%) |

| Rank | File | Regions | Region coverage | Lines |
| ---: | --- | ---: | ---: | ---: |
| 1 | `pillow-rs/src/compute/pool_cpu/ops/chops.rs` | 0/561 | 0.0% | 0/319 |
| 2 | `pillow-rs/src/compute/pool_cpu/ops/enhance.rs` | 0/398 | 0.0% | 0/216 |
| 3 | `pillow-rs/src/ops/utils.rs` | 0/65 | 0.0% | 0/48 |
| 4 | `pillow-rs/src/raster/traits/primitive.rs` | 0/225 | 0.0% | 0/132 |
| 5 | `pillow-rs/src/raster/traits/view.rs` | 0/25 | 0.0% | 0/24 |
| 6 | `pillow-rs/src/compute/pool_gpu/mod.rs` | 6/1718 | 0.3% | 6/1263 |
| 7 | `pillow-rs/src/compute/pool_cpu/ops/filter.rs` | 183/959 | 19.1% | 82/568 |
| 8 | `pillow-rs/src/raster/color/from_primitive.rs` | 16/80 | 20.0% | 8/43 |
| 9 | `pillow-rs/src/raster/dynamic.rs` | 349/1519 | 23.0% | 210/787 |
| 10 | `pillow-rs/src/compute/pool_cpu/ops/color.rs` | 119/506 | 23.5% | 55/217 |
| 11 | `pillow-rs/src/compute/pool_cpu/ops/effects.rs` | 990/2649 | 37.4% | 532/1324 |
| 12 | `pillow-rs/src/compute/registry.rs` | 999/2235 | 44.7% | 704/1368 |
| 13 | `pillow-rs/src/raster/color/from_color.rs` | 85/190 | 44.7% | 49/102 |
| 14 | `pillow-rs/src/compute/pool_cpu/ops/imageops.rs` | 433/965 | 44.9% | 251/522 |
| 15 | `pillow-rs/src/lib.rs` | 172/382 | 45.0% | 176/392 |
| 16 | `pillow-rs/src/error.rs` | 3/6 | 50.0% | 3/6 |
| 17 | `pillow-rs/src/raster/color/pixel_rgb.rs` | 48/78 | 61.5% | 27/48 |
| 18 | `pillow-rs-py/src/lib.rs` | 4619/5868 | 78.7% | 3092/3626 |
| 19 | `pillow-rs/src/checked_dims.rs` | 45/56 | 80.4% | 37/63 |
| 20 | `pillow-rs/src/compute/mod.rs` | 129/159 | 81.1% | 88/109 |
| 21 | `pillow-rs/src/compute/pool_cpu/ops/geometry.rs` | 1394/1700 | 82.0% | 730/871 |
| 22 | `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 1983/2295 | 86.4% | 1103/1252 |
| 23 | `pillow-rs/src/compute/pool_simd/mod.rs` | 97/109 | 89.0% | 62/70 |
| 24 | `pillow-rs/src/raster/color/pixel_luma.rs` | 54/60 | 90.0% | 32/38 |
| 25 | `pillow-rs/src/image.rs` | 4791/5291 | 90.5% | 3001/3263 |
| 26 | `pillow-rs/src/ops/transform.rs` | 644/703 | 91.6% | 459/507 |
| 27 | `pillow-rs/src/font/pilfont.rs` | 576/628 | 91.7% | 402/418 |
| 28 | `pillow-rs/src/ops/crop.rs` | 287/312 | 92.0% | 200/206 |
| 29 | `pillow-rs/src/ops/pil_resize.rs` | 1073/1159 | 92.6% | 639/679 |
| 30 | `pillow-rs/src/ops/paste.rs` | 942/1010 | 93.3% | 474/487 |
| 31 | `pillow-rs/src/font/imagingft.rs` | 1954/2087 | 93.6% | 1274/1317 |
| 32 | `pillow-rs/src/raster/buffer.rs` | 272/290 | 93.8% | 184/202 |
| 33 | `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 4772/5076 | 94.0% | 2447/2600 |
