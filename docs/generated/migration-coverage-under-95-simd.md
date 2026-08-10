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
base_commit: 8a92e62fa5578aae4cbaaf538994bbfe51dee6a9
coverage_run_id: migration-coverage-90e8d80de8014a248bc4e8182c84214b
parity_run_id: migration-parity-83795cc623fc4f3fb6e1f7518bead352
source_dirty_at_collection: true
threshold: 95%
metric: regions
total_regions: 106458
covered_regions: 63235
region_coverage: 59.3990%
total_lines: 68472
covered_lines: 40664
line_coverage: 59.3878%
total_branches: 14056
covered_branches: 7193
branch_coverage: 51.1739%
total_functions: 5313
covered_functions: 3041
function_coverage: 57.2370%
simd_impl_regions: 7246/7448 (97.2879%)
simd_impl_lines: 3853/3943 (97.7175%)
simd_impl_branches: 874/1004 (87.0518%)
simd_impl_functions: 194/203 (95.5665%)
in_repo_files_below_threshold: 32
external_dependency_files_below_threshold: 43
```

The in-repository list is ordered from lowest to highest region coverage. The
43 below-threshold files from the sibling `fontdone` dependency are excluded
from the actionable pillow-rs list; they are an external-library backlog.

The latest full SIMD refresh selected all 3,049 cases: 3,046 passed, 3 had
ordinary parity mismatches, and 0 had infrastructure errors or not-run cases.
The coverage workflow executed all 24 plans and passed all 3,049 execution
checks. The mismatches remain visible in the parity result; they are not
removed from the coverage denominator. Four legacy zero-execution SIMD adapters
(`simd_quantize`, `simd_blend`, `simd_composite`, and `simd_point_op`) and
their orphan scalar helpers were removed from the SIMD registry/source. Their
`PipelineOp` variants remain available to the core/GPU pipeline; public paths
use the exact quantizer, module-based blend/composite, and `Eval` paths. The
SIMD implementation files now total 7,246/7,448 regions (97.2879%); the
remaining SIMD-specific backlog is concentrated in `ops/adapters.rs` and
`pool_simd/mod.rs`, while `ops/scalar.rs` is at 5,104/5,164 regions (98.8381%).
This refresh added a core-owned integer `putdata` bulk path: exact built-in
lists and tuples now preserve Pillow's numeric-versus-packed mode distinction
in Rust and reach one SIMD `PutData` operation instead of binding-side
per-pixel writes. Four reachable packed-mode cases (PA, CMYK, YCbCr, and HSV)
pass on SIMD and add 14 regions, 17 lines, and 2 branches to the SIMD scalar
implementation. The binding remains on the existing re-entrant per-item path
for subclasses, nested values, and arbitrary sequences.
This refresh added one input-only public RGB paste case whose box is entirely
outside the destination, reaching the SIMD no-overlap guard, plus empty-source
`ImageOps.cover` and `ImageOps.fit` cases. The latter two exposed a public
parity divergence: Pillow raises the exact `ZeroDivisionError` at call time,
while Rust previously deferred the operation and returned an empty image.
Core validation now preserves those messages before dispatch; all three cases
pass on CPU and SIMD. The new cases raise the merged SIMD implementation by
one region, one line, and one branch.
A prior refresh added one input-only public RGB nearest-neighbour thumbnail case
with a destination larger than the source, reaching the SIMD scalar
thumbnail no-shrink copy/alpha-clamp path; it passes source/target parity on
both safe CPU and SIMD lanes and raises the scalar implementation by 19
regions, 11 lines, and 3 branches. That case also exposed a shared-core
divergence: Pillow never enlarges an in-place thumbnail, while Rust previously
queued the larger bound and the CPU path returned 8x8 for a 4x4 source. The
public Rust thumbnail dimension validation now caps positive bounds at the
loaded source dimensions. The current refresh replaced six rejected
`ImageOps` alpha-mode cases with six reachable input-only SIMD cases: LA and
RGBA no-shrink thumbnails, LA and RGBA no-reduce operations, and LA and RGBA
same-mode conversions. All six pass source/target parity. They add 2 SIMD
regions and 4 SIMD branches; lines and functions were already covered.
Ten new
input-only indexed `P`/`PA` nearest-neighbour cases cover `thumbnail`,
`contain`, `cover`, `fit`, and `scale` on CPU and SIMD. The first divergence
was `ImageOps.scale`: Rust expanded indexed samples to RGB/RGBA before
dispatch, while Pillow preserves the indexed result; the core palette-safety
gate now keeps `P`/`PA` on the raw nearest-neighbour resize path. All ten
cases pass both backend lanes. Four input-only CMYK ImageEnhance cases now exercise the
public enhancement constructors on both safe CPU and SIMD runs. Two shared CPU
divergences were also fixed and rechecked through SIMD: RGBA-to-LA now carries
source alpha, and ImageOps.contain preserves a computed zero-width result.
The zero-width `fit` case exposed the first divergence: Pillow allocates the
requested destination and leaves it zero-filled, while Rust previously
returned the empty source dimensions. The SIMD scalar fit path now preserves
Pillow's destination shape and zero-filled output. All six new cases pass
source/target parity. The previous refresh added six input-only public
`PIL.Image.Image.resize`, `rotate`, and `transform` `L`-mode zero-dimension
source cases, reaching the SIMD geometry guards. The rotate cases use explicit
bilinear resampling so they select the SIMD lane; all six pass source/target
parity. The rotate mismatch exposed the first divergence: Pillow computes the
expanded empty-source canvas and fill path, while Rust previously returned the
original dimensions before sampling. The SIMD scalar path now computes the
rotated dimensions and fill before its empty-source return. The previous
refresh added two input-only public
`PIL.Image.Image.filter` `L`-mode cases with images smaller than the 3x3 and
5x5 kernels, reaching both SIMD scalar size guards; both pass source/target
parity. This refresh added eight input-only rectangular `L`/`I` filter cases
for the 3x3 and 5x5 kernels, reaching both width and height guards; all eight
pass source/target parity. It also added four public `LA` filter cases for
`MaxFilter`, `MinFilter`, `MedianFilter`, and `RankFilter`. The first SIMD
divergence was that the packed scalar filters preserved LA alpha from the
destination pixel, while Pillow filters alpha independently with the selected
window operation. The SIMD scalar implementation now filters LA/RGBA alpha
alongside color channels; all four cases pass on CPU and SIMD. The previous
refresh added four input-only public `ImageOps` `F`-mode
cases for `contain`, `cover`, `fit`, and `scale`, reaching the native-scalar
fallback branches; all four pass source/target parity. The
previous refresh added two input-only public
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
The latest refresh added six valid input-only odd-dimension cases for SIMD
`flip`, `mirror`, and `transpose`. They use in-bounds public `putpixel`
coordinates so the odd-height, odd-width, and odd-total branches execute
instead of stopping in the existing out-of-bounds setup path. All six pass
source/target parity on both safe CPU and SIMD lanes, adding 23 SIMD regions,
10 lines, and 11 branches.
The final refresh added one public nearest-neighbour `ImageOps.fit` case for a
tall target and one empty `ImageOps.equalize` case. Both pass on CPU and SIMD;
they add 2 SIMD regions, 1 line, and 1 branch in the scalar implementation.
The latest refresh added five valid input-only RGB bilinear workflows for
`ImageOps.contain`, `cover`, `fit`, `pad`, and `scale`. They exercise the
non-alpha side of the shared SIMD resize kernels and add one region and one
branch to the SIMD implementation aggregate. All five pass source/target
parity on both safe CPU and SIMD lanes.
The current refresh added one valid input-only `RGBa` nearest-neighbour
`Image.Image.resize` workflow. The existing premultiplied-alpha bilinear case
short-circuited before the explicit `RGBa` guard in the SIMD adapter; the
nearest case reaches that public mode branch and passes source/target parity on
both safe CPU and SIMD lanes, adding one SIMD branch.

The final refresh added one valid input-only `RGBa` affine transform workflow.
The SIMD adapter now evaluates the explicit premultiplied-mode guard before
the shared `Rgba8` storage-color guard, preserving Pillow's exact CPU fallback
for premultiplied samples while making the public branch reachable. The case
passes source/target parity on both safe CPU and SIMD lanes and raises SIMD
branch coverage from 805/952 to 806/952; the region, line, and function
numerators are unchanged.

The final refresh added input-only public `F` and `I` resize cases with a
zero-width source. Rust's scalar-storage validator previously rejected these
valid empty images through the nonzero allocation guard before the SIMD
adapter could reach its intentional native-scalar CPU fallback. The validator
now accepts an already-existing empty scalar buffer, and both cases pass on
CPU and SIMD. They increase merged Rust coverage to 63,039/106,276 regions;
the SIMD implementation aggregate remains 7,058/7,266 because the native
`F`/`I` path intentionally executes in the shared CPU geometry code.

The latest refresh added two reachable public SIMD workflows: an RGB affine
bilinear transform with an out-of-bounds fill boundary and a solid RGB paste
whose box is vertically outside the destination. Both materialize bytes and
pass source/target parity on CPU and SIMD. They add three SIMD branches in the
scalar implementation, raising the SIMD aggregate from 806/952 to 809/952
(84.9790%); regions, lines, and functions remain unchanged.

The current refresh added two valid input-only `L`-mode rotation workflows:
fractional bilinear rotation of an empty source and a non-empty source. Both
pass source/target parity on CPU and SIMD. The non-empty mode-path case reaches
four previously uncovered SIMD scalar regions and branches, raising the
implementation aggregate to 7,058/7,266 regions (97.1374%) and 813/952
branches (85.3992%); lines and functions remain unchanged.

The current refresh added four valid input-only RGBA `ImageEnhance` workflows
for `Brightness`, `Color`, `Contrast`, and `Sharpness`. They pass source/target
parity on both safe CPU and SIMD lanes and exercise the public alpha-preserving
enhancement contract. The merged SIMD LLVM numerators remain unchanged because
the corresponding scalar alpha branches were already reached by other valid
public workflows; these cases close the missing manifest coverage requirements
without inflating implementation coverage.

The latest refresh also adds a reviewed encoded-source `PIL.Image.Image.format`
workflow to the generated public input corpus. It passes on CPU and SIMD and
keeps the encoded metadata path represented in operation coverage; it does not
change the SIMD implementation numerator because `format` is outside the
compute lane.

The latest refresh added five valid input-only SIMD workflows: LA and RGBA
`ImageOps.autocontrast`, LA and RGBA `ImageOps.equalize`, and an `L` affine
bilinear `Image.transform`. All five pass source/target parity on both safe CPU
and SIMD lanes. The alpha workflows reach two previously uncovered SIMD scalar
regions and branches, raising the implementation aggregate to 7,060/7,266
regions (97.1643%) and 815/952 branches (85.6092%); lines and functions remain
unchanged.

The latest refresh added two reachable input-only `ImageOps` workflows: an
odd-width RGBA mirror case that preserves the middle pixel's alpha and a
fractional-centering pad case that rounds the positive offset upward. Both
pass source/target parity on CPU and SIMD, adding one SIMD region and two
SIMD branches in the scalar implementation.

The three remaining mismatches are the two known 16-bit
`PIL.Image.Image.paste` inputs (`opened-i16-scalar` and `opened-i16n-scalar`),
which remain pending for the TIFF/16-bit lane, and
`PIL.ImageFont.FreeTypeFont.set_variation_by_axes`, which is tracked in the
separate fontdone parity lane. There is no GPU/crash-lane result in this
report; that lane remains intentionally excluded after the prior device-hang
incident.

This refresh added four reachable public RGB workflows: affine bilinear
transform, direct bilinear resize, nearest-neighbour resize, and arbitrary
bilinear rotate. The first resize divergence was Pillow's coefficient-table
resampler versus the packed SIMD kernel's centered four-neighbour
approximation, so direct public bilinear resize now uses the shared exact
pure-Rust resampler. The rotate divergence was Pillow's half-pixel bilinear
support and edge clamping versus Rust's earlier fill-at-image-boundary rule;
the ordinary byte path now matches that support while preserving PA's legacy
index/alpha convention. All four workflows pass on the safe CPU and SIMD
lanes. The higher-order ImageOps resize filters and Gaussian blur continue to
use the shared exact pure-Rust CPU implementation because the packed SIMD
kernels only implement nearest/bilinear resize and an approximate
integer-radius blur. GPU execution remains intentionally excluded after the
prior device-hang incident.

The current refresh added six generator-backed public ImageOps alpha-mode
cases that preserve exact Pillow rejection behavior; they keep the public
coverage corpus honest but do not inflate SIMD kernel coverage. It also added
reachable LA and RGBA affine bilinear workflows. The first divergence was
Pillow's premultiplied `La`/`RGBa` sampling and straight-alpha conversion,
while Rust interpolated straight channels and the SIMD adapter previously
hid those modes behind a storage-color CPU fallback. The shared CPU and SIMD
paths now premultiply channels before interpolation, truncate weighted bytes,
and unpremultiply the result; nearest fills and non-alpha HSV/YCbCr/CMYK
storage retain their native semantics. The latest refresh routes valid LA/RGBA
bilinear rotations through the SIMD kernel, interpolates alpha, and applies
Pillow's truncated premultiply/unpremultiply round trip at transparent edges.
The new LA input and existing RGBA input pass on CPU and SIMD. The current
SIMD implementation aggregate is 7,246/7,448 regions (97.2879%), 3,853/3,943
lines (97.7175%), 874/1,004 branches (87.0518%), and 194/203 functions
(95.5665%).

The final SIMD coverage refresh added two generator-backed, observed public
`Image.Image.crop` workflows for LA and RGBA. Both pass on CPU and SIMD and
exercise the packed crop alpha-preservation branch; the scalar implementation
branch coverage increased from 761/846 to 762/846. That previous 3,048-case
lane had 3,045 passes, the same three ordinary mismatches, and no
infrastructure errors or not-run cases.

The current refresh adds one observed public RGB `ImageOps.fit` workflow with
an out-of-range `bleed=1.0` and a nearest-neighbour method. The first
divergence was that the Rust deferred pipeline passed the invalid bleed into
backend-specific geometry, while Pillow normalizes it to `0.0` at the public
call boundary. Core now owns that normalization for CPU, SIMD, and GPU; the
case passes on the SIMD lane. The full 3,049-case lane has 3,046 passes and
three ordinary mismatches, with zero infrastructure errors or not-run cases.
The merged report is 63,235/106,458 regions (59.3990%) overall; the SIMD
implementation aggregate remains 7,246/7,448 regions (97.2879%), because the
remaining zero-execution SIMD regions are defensive or invalid internal-shape
paths rather than reachable public workflows.

SIMD implementation-file coverage is:

| File | Regions | Lines | Branches | Functions |
| --- | ---: | ---: | ---: | ---: |
| `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 2045/2175 (94.02%) | 1125/1182 (95.18%) | 110/156 (70.51%) | 77/82 (93.90%) |
| `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 5104/5164 (98.84%) | 2666/2691 (99.07%) | 762/846 (90.07%) | 108/108 (100.00%) |
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
| 9 | `pillow-rs/src/compute/pool_cpu/ops/color.rs` | 119/531 | 22.4% | 55/229 |
| 10 | `pillow-rs/src/raster/dynamic.rs` | 344/1519 | 22.6% | 206/787 |
| 11 | `pillow-rs/src/compute/pool_cpu/ops/effects.rs` | 995/2680 | 37.1% | 537/1342 |
| 12 | `pillow-rs/src/compute/registry.rs` | 988/2219 | 44.5% | 701/1364 |
| 13 | `pillow-rs/src/raster/color/from_color.rs` | 85/190 | 44.7% | 49/102 |
| 14 | `pillow-rs/src/compute/pool_cpu/ops/imageops.rs` | 431/963 | 44.8% | 251/522 |
| 15 | `pillow-rs/src/lib.rs` | 172/382 | 45.0% | 176/392 |
| 16 | `pillow-rs/src/error.rs` | 3/6 | 50.0% | 3/6 |
| 17 | `pillow-rs/src/raster/color/pixel_rgb.rs` | 48/78 | 61.5% | 27/48 |
| 18 | `pillow-rs-py/src/lib.rs` | 4645/5907 | 78.6% | 3109/3647 |
| 19 | `pillow-rs/src/checked_dims.rs` | 45/56 | 80.4% | 37/63 |
| 20 | `pillow-rs/src/compute/mod.rs` | 129/159 | 81.1% | 88/109 |
| 21 | `pillow-rs/src/compute/pool_cpu/ops/geometry.rs` | 1422/1728 | 82.3% | 738/879 |
| 22 | `pillow-rs/src/compute/pool_simd/mod.rs` | 97/109 | 89.0% | 62/70 |
| 23 | `pillow-rs/src/raster/color/pixel_luma.rs` | 54/60 | 90.0% | 32/38 |
| 24 | `pillow-rs/src/image.rs` | 4820/5326 | 90.5% | 3016/3287 |
| 25 | `pillow-rs/src/ops/transform.rs` | 644/703 | 91.6% | 459/507 |
| 26 | `pillow-rs/src/font/pilfont.rs` | 576/628 | 91.7% | 402/418 |
| 27 | `pillow-rs/src/ops/crop.rs` | 287/312 | 92.0% | 200/206 |
| 28 | `pillow-rs/src/ops/paste.rs` | 942/1010 | 93.3% | 474/487 |
| 29 | `pillow-rs/src/ops/pil_resize.rs` | 1083/1159 | 93.4% | 642/679 |
| 30 | `pillow-rs/src/font/imagingft.rs` | 1954/2087 | 93.6% | 1274/1317 |
| 31 | `pillow-rs/src/raster/buffer.rs` | 272/290 | 93.8% | 184/202 |
| 32 | `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 2045/2175 | 94.0% | 1125/1182 |
