# Coverage backlog after the Python binding target

This is the remaining initial queue after selecting pillow-rs-py/src/lib.rs
as the first target. It is ordered by missing LLVM regions, highest first.

Evidence source: Coverage MCP snapshot
35129b99-796a-47c3-9f19-05db4f6510e9, suite
pillow-rs-current-cpu-simd-gpu-coverage-20260815, commit
78d361194e272a574ae89ceb13be3b3be1114377.

The current target is intentionally omitted. Zero-gap files are omitted. This
report is planning evidence only; it does not modify fixtures, outputs, hashes,
thresholds, filters, coverage counts, or denominators. Font entries remain
deferred, and GPU work must remain bounded and memory-safe.

## Rank-1 follow-up: `pillow-rs/src/compute/pool_simd/ops/scalar.rs`

The reachability audit used the baseline snapshot above and the read-only Sol
strategy packet. The retained input batch adds 100 valid public
`PIL.ImageEnhance.Brightness.enhance` cases in explicit `YCbCr` and `HSV`
modes, using generator families 31-40. These modes bypass the native byte LUT
while remaining supported RGB8-backed public images, so they reach the packed
scalar Brightness fallback.

The follow-up Coverage MCP snapshot is
`5b4abec1-a801-4720-a4fb-29ca9d3c23ba`. It covers `scalar.rs:129` and leaves
`scalar.rs:133` red. The remaining red branch is the unsupported
non-alpha/non-CMYK fallback: supported public L/RGB posterize and Brightness
inputs use native paths, while the public typed/palette alternatives reject or
divert before this branch.

The audit also classifies the large scalar transpose body as native-fast-path
or CPU-diverted for all supported public representations; the F-mode rank
fallback as unreachable through public filter dispatch; the packed median,
min/max/rank and BoxBlur bodies as already covered; and scalar rotate as
already covered. This rank is therefore complete as a reachability target, not
as a license to force private probes. The next reachable file-level audit is
`pillow-rs/src/image.rs`; `pillow-rs/src/raster/dynamic.rs` remains an
internal type-matrix target unless a supported public path is demonstrated.

## Rank-3 follow-up: `pillow-rs/src/image.rs` `putdata` bytes fallback

Coverage MCP source context identified `image.rs:4875-4876` as the public
multiband `bytes` handoff: `putdata_bulk` recognizes a Python `bytes` object,
the core fast path consumes only `1`, `L`, `P`, and `I;16*`, and all other
supported modes deliberately return `Ok(false)` for generic per-item
coercion. The retained input batch adds exactly 100 valid public
`PIL.Image.Image.putdata` cases in ten families, covering RGB, RGBA, LA, PA,
CMYK, YCbCr, HSV, and multiband scale/offset variants. Representative RGB,
LA, CMYK, PA, YCbCr, HSV, and scaled cases passed focused parity validation.

The Coverage MCP run is `ee78289b-7b7b-43a4-b8d3-2b8f95b85d1c`, using the
registered bounded all-GPU command. It passed all 24 plans, 9,881 tests, and
468 parity cases with zero failures or skips; the ingested snapshot is
`9f4cc724-9d07-48bd-a2a2-429fe341a56f`. Compared with the prior snapshot
`5b4abec1-a801-4720-a4fb-29ca9d3c23ba`, aggregate coverage gained 4 regions,
3 lines, and 1 branch with unchanged denominators. The file-level result is
5,677/6,528 regions (86.964%), 3,517/3,988 lines (88.190%), 611/690
branches (88.551%), and 289/368 functions (78.533%). MCP line evidence shows
300 hits on lines 4875-4876, confirming the new inputs reached the intended
fallback rather than a private probe.

The remaining `PutPixelValue::Invalid` branch at lines 2813-2819 is not
retained as an input target: the manifest declares `putpixel.value` as
integer, number, or sequence, and non-integral sequence members reach
source/target error-message differences for multiband modes. The next
`image.rs` audit should therefore move to another declared public operation,
not broaden the input contract or alter comparison policy.

## Reachable public input: `putdata` generic `P` sequence

A parity-validated generic sequence was added for `PIL.Image.Image.putdata` on
mode `P`. Unlike exact lists, tuples, and bytes, the public sequence protocol
uses the per-item `putdata_value_at` path; the case reaches the
`Image::Paletted` mutation arm at `image.rs:4993-4996`.

The bounded all-GPU Coverage MCP run
`a0cda9c8-da31-4a9a-9954-32824403a0bc` passed all 24 plans, 9,883 tests, and
468 parity cases with zero failures/skips in 193.239 seconds. Snapshot
`b03eb6ff-68e8-471c-9c0a-4adc70b7afa1` compared with
`34936b0b-f469-49d2-b5d8-e1442c4b82b7` gained 7 regions, 5 lines, and 1
branch with unchanged denominators. `image.rs` is now at 5,685/6,528 regions,
3,522/3,988 lines, 612/690 branches, and 289/368 functions; aggregate
coverage is 55,103/64,241 regions, 34,277/39,438 lines, 5,556/6,968
branches, and 2,655/3,259 functions. The generic P case is retained.

## Zero-gain audit: typed `ImageStat.Stat` fallback

The supported public 16-bit RGB/RGBA PNG streams were tested as a candidate
entry point for the `ImageStat.Stat` fallback at `image.rs:3030-3206`. One
hundred valid `ImageStat.Stat` workflows were temporarily added, with both
RGB16 and RGBA16 PNG inputs and all public Stat properties observed. The two
representative RGB16/RGBA16 cases passed focused parity validation, but the
bounded all-GPU Coverage MCP run `7fe55f11-1f5c-48b8-96c9-0e4e8f2b5b78`
passed all 24 plans, 9,981 tests, and 468 parity cases with zero failures or
skips while producing no coverage delta: snapshot
`db224a1b-af3b-4458-a25f-4a064470b7b8` remains 55,095/64,241 regions,
34,272/39,438 lines, 5,555/6,968 branches, and 2,655/3,259 functions,
identical to snapshot `9f4cc724-9d07-48bd-a2a2-429fe341a56f`. The temporary
batch was pruned as zero-gain. The fallback remains a reachability finding,
not an honest input target, until a supported public source is shown to retain
the typed DynamicImage representation through the maintained decode path.

## Zero-gain audit: `Image::materialize_for_ops` palette-pipeline path

Coverage MCP source context at `image.rs:4270-4298` identifies the remaining
palette expansion branches in `Image::materialize_for_ops` and
`paletted_to_rgb`. A temporary batch of exactly 100 public
`PIL.ImageChops.blend` workflows used deferred `P` operands with nontrivial
alpha values; all four representative cases passed focused parity validation.
The bounded all-GPU Coverage MCP run
`53bb1c5a-5aed-4883-810f-b9303d0640d7` passed all 24 plans, 9,981 tests, and
468 parity cases with zero failures or skips; snapshot
`610d34d7-852f-4425-92c4-0871ea883f95` is identical to the prior snapshot
`db224a1b-af3b-4458-a25f-4a064470b7b8` at 55,095/64,241 regions,
34,272/39,438 lines, 5,555/6,968 branches, and 2,655/3,259 functions.
MCP line evidence leaves lines 4273, 4277-4282, and 4290-4291 red.

The zero gain is explained by the supported public contract, not by missing
input variety: `pillow-rs/src/ops/module_fns.rs::blend` rejects `P` images
before queuing the operation, while the public indexed paste/composite paths
deliberately use `materialize_indices` to preserve raw samples. The temporary
batch was pruned, and these branches remain classified as internal fallback
reachability rather than an honest input target.

## Reachable public input: `Image.load` malformed PNG without `IDAT`

Coverage MCP source context at `image.rs:4428-4436` showed the public lazy
decode path. A valid encoded PNG header with an `IHDR` and `IEND` but no
`IDAT` was added as one direct `PIL.Image.Image.load` input. Focused parity
passed exactly: Pillow and Rust both accepted the header at `open` and
returned `OSError("cannot load this image")` from `load`.

The bounded all-GPU Coverage MCP run was
`84dcaa74-8762-4b98-8fda-1b38b7308d77`, with 24 plans, 9,882 tests, and 468
parity cases passing, zero failures/skips, and a 197.077-second duration. The
ingested snapshot is `34936b0b-f469-49d2-b5d8-e1442c4b82b7`. Compared with
`aa99e411-abd1-4b0a-987a-ae1f0c5af83d`, it gained one region and no lines,
branches, functions, or denominator changes: aggregate coverage is now
55,096/64,241 regions, 34,272/39,438 lines, 5,555/6,968 branches, and
2,655/3,259 functions. The final `image_from_materialized(...)?` region at
line 4436 remains red because this malformed stream fails during deferred
codec materialization before that conversion call. The input is retained as
an honest one-region gain.

## Zero-gain audit: palette-less `P` pipeline `load`

A focused public case chained `Image.new("P")`, `ImageChops.invert`, and
`Image.load` to probe the `Image::Pipeline` no-retained-palette error arm at
`image.rs:4458`. Source and target both returned a `PixelAccess`, so focused
parity passed. The bounded all-GPU Coverage MCP run
`2865ce1f-6bd8-4f97-b149-44b5df119b73` passed all 24 plans, 9,883 tests, and
468 parity cases with zero failures/skips in 198.545 seconds; snapshot
`455a88f4-f820-44eb-9841-123ff5b60293` had no coverage delta from
`34936b0b-f469-49d2-b5d8-e1442c4b82b7`.

The branch is not reached because `Image::push_op` deliberately represents
an indexed ImageChops result with `Some(Vec::new())`, distinguishing an empty
public palette from an absent retained-palette invariant. The temporary case
was pruned; line 4458 remains classified as an internal defensive branch.

## Zero-gain audit: `Image.getdata(band=...)` non-byte fallback

A valid public `Image.frombytes("I;16", ...)` followed by
`Image.getdata(band=0)` was tested against the red fallback at
`image.rs:4595-4598`. Focused parity passed, but the bounded all-GPU Coverage
MCP run `2d02c2ef-cbd7-4caa-bcc2-1ce86705d9a7` passed all 24 plans, 9,883
tests, and 468 parity cases with zero failures/skips in 193.964 seconds; its
snapshot `b9762199-3372-44e4-a4a4-0a81b78b8375` had no delta from
`34936b0b-f469-49d2-b5d8-e1442c4b82b7`.

The public Python binding handles `I;16` in `getdata_formatted` and rejects an
explicit band with `ValueError("image has wrong mode")` before calling core
`Image::getdata`. The temporary case was therefore pruned; the fallback is
binding-inaccessible through supported Python inputs.

## Zero-gain audit: `Image.getcolors` non-native raster fallback

A valid 16-bit RGB PNG was tested against the conversion fallbacks at
`image.rs:5250-5277`. Focused parity passed, but the bounded all-GPU Coverage
MCP run `ee5f7581-ed6c-4edb-bf89-cfae6ad3f826` passed all 24 plans, 9,884
tests, and 468 parity cases with zero failures/skips in 192.955 seconds;
snapshot `2526c366-d132-4c02-a6a8-b26cb33c4961` had no coverage delta from
`b03eb6ff-68e8-471c-9c0a-4adc70b7afa1`.

The maintained encoded-input path normalizes this public PNG into the ordinary
RGB representation before `getcolors`; it does not expose a `DynamicImage::Rgb16`
to the operation. The temporary case was pruned, and the non-native fallback
remains an internal decoder/type-matrix path.

## Rejected input: synthetic 16-bit RGB TIFF

To distinguish the PNG normalization result from a genuinely retained typed
raster, a temporary valid little-endian 16-bit RGB TIFF was tried against
`ImageStat.Stat`, `Image.getcolors`, and `Image.getprojection`. Pillow decoded
the source case, but the supported target decoder returned
`UnidentifiedImageError("cannot identify image file ...")` during the public
`Image.open` setup step. Focused parity therefore failed with a
`not_run_mismatch`, and no Coverage MCP run was submitted because the parity
gate rejected the input before the operation under test.

The helper and all three temporary cases were pruned. This is evidence that
the TIFF route cannot establish honest target reachability for the RGB16
fallbacks; it does not justify changing `image-slash-star`, the target decoder
surface, or parity policy.

## Zero-gain audit: `Image.getexif` JPEG scanner boundaries

Coverage MCP source context at `image.rs:4127-4176` leaves only the defensive
JPEG scanner returns at lines 4129, 4149, 4164, and 4175. Existing supported
JPEG inputs cover the valid SOI, standalone-marker, short-length, SOS, and
APP1-payload paths. Three temporary encoded inputs were then tried to place
EOI, a truncated APP1 segment, or end-of-input before SOS. Each focused parity
run reported `setup-image-1 failed`, leaving the `getexif` call `not_run` on
both source and target; the matching all-GPU Coverage MCP run
`eda3b511-dcab-4d87-be0d-a9d9323ca7e7` passed in 192.964 seconds and ingested
snapshot `75cb3bfc-a7f9-456c-a623-0285cfa119ee`, but comparison with
`b03eb6ff-68e8-471c-9c0a-4adc70b7afa1` showed zero coverage delta. The
pre-SOS cases were pruned. These returns remain unreachable through the
supported public JPEG opener; covering them would require a direct parser
probe or a decoder change, neither of which is an input-driven coverage gain.

## Zero-gain audit: `Image::shares_execution_source` non-loaded arms

Coverage MCP source context at `image.rs:1228-1258` identifies the remaining
paletted, encoded-byte, and deferred-pipeline identity arms at lines 1233-1254.
A temporary matrix supplied 100 valid public
`PIL.ImageChops.multiply(...).screen(...)` workflows that reused the same
secondary object, spanning concrete `P`, lazy encoded RGB/RGBA, and deferred
pipeline operands. Sixty-nine structurally distinct cases were retained after
input deduplication; representative P, pipeline, and lazy encoded cases all
passed focused parity validation.

The bounded all-backend Coverage MCP run
`f2d5f7a2-910c-4ea3-a38a-6eaef353837a` completed in 193.346 seconds with all
24 plans, 9,950 tests, and 468 parity cases passing, zero failures/skips, and
an ingested snapshot `8f15b258-7466-4b1a-90cf-cae85995d5ad`. Compared with
snapshot `610d34d7-852f-4425-92c4-0871ea883f95`, aggregate coverage was
unchanged at 55,095/64,241 regions, 34,272/39,438 lines, 5,555/6,968
branches, and 2,655/3,259 functions. MCP source evidence still marks
`image.rs:1233-1234`, `1238`, `1241`, `1247`, `1250`, and `1253-1254` red.

The public inputs reached the documented fusion-shaped workflows but not these
defensive identity arms: CPU and SIMD fusion return before the identity check
when an explicit mode is present, and the registered combined dispatch did not
enter the remaining Paletted/Bytes/Pipeline match arms. The temporary matrix
was pruned. The already-covered `Loaded` arm remains evidence that ordinary
same-source fusion is reachable; the other arms are currently classified as
internal reachability rather than retained input targets.

## First-target audit: `pillow-rs-py/src/lib.rs`

The first input-driven pass used Coverage MCP snapshot
`dbedfb50-985d-452f-b2db-df4ab018654c` after adding a public sequence
descriptor and six parity-validated `putdata`/`putpixel` stimuli. The file is
now at 4,966/6,311 regions (78.688%), 3,294/3,885 lines (84.788%),
303/360 branches (84.167%), and 452/575 functions (78.609%). Compared with
the initial snapshot, this recovered 60 regions, 35 lines, and 8 branches;
the aggregate suite recovered 60 regions and moved from 85.172% to 85.265%
region coverage.

The remaining `putdata_value_from_python` conversion arms are covered. The
remaining red conversion region is the final `Invalid` arm of
`putpixel_value_from_python`: an input containing a non-numeric member reaches
the target, but the source and target expose different public error messages,
so it cannot be retained as an honest parity case without changing behavior or
comparison policy.

Most of this file is correctly binding-owned: PyO3 extraction, Python
sequence/type-slot checks, Python callback invocation, filesystem/file-like
I/O, `PyErr` mapping, and conversion of Rust results into Python objects. The
actual image semantics and validation are delegated to `pillow-rs`. The one
clear algorithmic duplicate is the `_core.mesh_flatten` compatibility helper
around lines 2239-2256; public `Image.transform` already flattens real mesh
data in `pillow-rs/src/ops/transform.rs`, while no active public workflow calls
the standalone helper. It is therefore classified as an architectural cleanup,
not an input-driven coverage target for this pass.

Conversions do not automatically cover every conversion branch. Exact
`list`/`tuple`/`bytes` values intentionally take bulk fast paths before the
per-item converter; defaults and internal-only entry points bypass other arms;
and unsupported Python object shapes are not expressible as valid
parity-equivalent inputs. The new generic sequence cases cover the valid
non-fast-path contract without weakening expected results.

| Rank | File | Missing regions | Missing lines | Missing branches | Missing functions | Initial disposition |
|---:|---|---:|---:|---:|---:|---|
| 1 | [pillow-rs/src/compute/pool_simd/ops/scalar.rs](../pillow-rs/src/compute/pool_simd/ops/scalar.rs) | 1078 | 565 | 254 | 18 | reachability audit; public dispatch currently bypasses several scalar arms |
| 2 | [pillow-rs/src/raster/dynamic.rs](../pillow-rs/src/raster/dynamic.rs) | 852 | 460 | 2 | 57 | internal type matrix; not a supported public-input target |
| 3 | [pillow-rs/src/image.rs](../pillow-rs/src/image.rs) | 847 | 471 | 79 | 78 | targeted public-input audit; many lifecycle/invariant paths |
| 4 | [pillow-rs/src/compute/pool_simd/ops/adapters.rs](../pillow-rs/src/compute/pool_simd/ops/adapters.rs) | 742 | 417 | 196 | 14 | reachability audit; adapter guards and unsupported layouts |
| 5 | [pillow-rs/src/compute/pool_gpu/mod.rs](../pillow-rs/src/compute/pool_gpu/mod.rs) | 735 | 561 | 210 | 43 | do not force; cache/device/failure paths are safety-sensitive |
| 6 | [pillow-rs/src/compute/registry.rs](../pillow-rs/src/compute/registry.rs) | 530 | 281 | 111 | 11 | internal registry/backend permutations |
| 7 | [pillow-rs/src/compute/pool_cpu/ops/color.rs](../pillow-rs/src/compute/pool_cpu/ops/color.rs) | 286 | 109 | 22 | 2 | conditional public-input audit; verify route before adding cases |
| 8 | [pillow-rs/src/compute/mod.rs](../pillow-rs/src/compute/mod.rs) | 285 | 236 | 18 | 47 | internal backend control/telemetry helpers |
| 9 | [pillow-rs/src/raster/traits/primitive.rs](../pillow-rs/src/raster/traits/primitive.rs) | 225 | 132 | 48 | 14 | generic trait instantiations not reached by supported inputs |
| 10 | [pillow-rs/src/lib.rs](../pillow-rs/src/lib.rs) | 210 | 216 | 0 | 37 | public/core entry-point audit |
| 11 | [pillow-rs/src/compute/pool_cpu/ops/chops.rs](../pillow-rs/src/compute/pool_cpu/ops/chops.rs) | 208 | 175 | 10 | 18 | targeted public-input audit |
| 12 | [pillow-rs/src/compute/pool_cpu/ops/effects.rs](../pillow-rs/src/compute/pool_cpu/ops/effects.rs) | 197 | 127 | 41 | 34 | targeted public-input audit |
| 13 | [pillow-rs/src/compute/pool_simd/mod.rs](../pillow-rs/src/compute/pool_simd/mod.rs) | 150 | 108 | 17 | 11 | backend dispatch reachability audit |
| 14 | [pillow-rs/src/compute/pool_cpu/ops/imageops.rs](../pillow-rs/src/compute/pool_cpu/ops/imageops.rs) | 139 | 69 | 12 | 6 | targeted public-input audit |
| 15 | [pillow-rs/src/font/imagingft.rs](../pillow-rs/src/font/imagingft.rs) | 133 | 43 | 18 | 11 | defer; font lane excluded from this campaign |
| 16 | [pillow-rs/src/ops/analysis.rs](../pillow-rs/src/ops/analysis.rs) | 124 | 53 | 28 | 0 | targeted public-input audit |
| 17 | [pillow-rs/src/draw/mod.rs](../pillow-rs/src/draw/mod.rs) | 123 | 91 | 36 | 13 | targeted public-input audit |
| 18 | [pillow-rs/src/compute/pool_cpu/ops/draw.rs](../pillow-rs/src/compute/pool_cpu/ops/draw.rs) | 111 | 65 | 61 | 2 | targeted public-input audit |
| 19 | [pillow-rs/src/ops/convert.rs](../pillow-rs/src/ops/convert.rs) | 100 | 30 | 10 | 10 | targeted public-input audit |
| 20 | [pillow-rs/src/raster/color/from_color.rs](../pillow-rs/src/raster/color/from_color.rs) | 93 | 46 | 0 | 8 | reachability audit; generic color conversion matrix |
| 21 | [pillow-rs/src/color.rs](../pillow-rs/src/color.rs) | 90 | 39 | 1 | 7 | targeted public-input audit |
| 22 | [pillow-rs/src/ops/pil_resize.rs](../pillow-rs/src/ops/pil_resize.rs) | 78 | 49 | 43 | 4 | targeted public-input audit |
| 23 | [pillow-rs/src/raster/color/from_primitive.rs](../pillow-rs/src/raster/color/from_primitive.rs) | 77 | 40 | 0 | 8 | generic conversion matrix; likely unsupported public types |
| 24 | [pillow-rs/src/compute/pool_cpu/ops/filter.rs](../pillow-rs/src/compute/pool_cpu/ops/filter.rs) | 70 | 49 | 29 | 7 | targeted public-input audit |
| 25 | [pillow-rs/src/ops/paste.rs](../pillow-rs/src/ops/paste.rs) | 68 | 13 | 0 | 10 | targeted public-input audit |
| 26 | [pillow-rs/src/compute/pool_cpu/ops/geometry.rs](../pillow-rs/src/compute/pool_cpu/ops/geometry.rs) | 65 | 23 | 26 | 7 | targeted public-input audit |
| 27 | [pillow-rs/src/ops/utils.rs](../pillow-rs/src/ops/utils.rs) | 65 | 48 | 14 | 2 | zero-coverage utility; reachability audit |
| 28 | [pillow-rs/src/ops/quantize.rs](../pillow-rs/src/ops/quantize.rs) | 61 | 38 | 36 | 2 | targeted public-input audit |
| 29 | [pillow-rs/src/ops/imageops.rs](../pillow-rs/src/ops/imageops.rs) | 59 | 18 | 10 | 3 | targeted public-input audit |
| 30 | [pillow-rs/src/font/pilfont.rs](../pillow-rs/src/font/pilfont.rs) | 52 | 16 | 0 | 12 | defer; font lane excluded from this campaign |
| 31 | [pillow-rs/src/ops/transform.rs](../pillow-rs/src/ops/transform.rs) | 48 | 45 | 5 | 3 | targeted public-input audit |
| 32 | [pillow-rs/src/compute/pool_cpu/mod.rs](../pillow-rs/src/compute/pool_cpu/mod.rs) | 36 | 19 | 9 | 4 | backend dispatch reachability audit |
| 33 | [pillow-rs/src/ops/module_fns.rs](../pillow-rs/src/ops/module_fns.rs) | 36 | 17 | 10 | 0 | targeted public-input audit |
| 34 | [pillow-rs/src/raster/color/pixel_rgb.rs](../pillow-rs/src/raster/color/pixel_rgb.rs) | 30 | 21 | 0 | 5 | generic pixel conversion reachability audit |
| 35 | [pillow-rs/src/font/mod.rs](../pillow-rs/src/font/mod.rs) | 26 | 17 | 0 | 3 | defer; font lane excluded from this campaign |
| 36 | [pillow-rs/src/ops/array.rs](../pillow-rs/src/ops/array.rs) | 25 | 10 | 2 | 7 | targeted public-input audit |
| 37 | [pillow-rs/src/ops/crop.rs](../pillow-rs/src/ops/crop.rs) | 25 | 6 | 0 | 6 | targeted public-input audit |
| 38 | [pillow-rs/src/ops/param_filters.rs](../pillow-rs/src/ops/param_filters.rs) | 24 | 2 | 0 | 2 | targeted public-input audit |
| 39 | [pillow-rs/src/checked_dims.rs](../pillow-rs/src/checked_dims.rs) | 19 | 39 | 4 | 4 | bounded error cases; avoid unsafe allocation forcing |
| 40 | [pillow-rs/src/raster/buffer.rs](../pillow-rs/src/raster/buffer.rs) | 17 | 15 | 5 | 1 | targeted public-input audit |
| 41 | [pillow-rs/src/compute/pool_cpu/ops/enhance.rs](../pillow-rs/src/compute/pool_cpu/ops/enhance.rs) | 12 | 5 | 5 | 2 | targeted public-input audit |
| 42 | [pillow-rs/src/ops/filter.rs](../pillow-rs/src/ops/filter.rs) | 7 | 5 | 0 | 1 | targeted public-input audit |
| 43 | [pillow-rs/src/raster/color/pixel_luma.rs](../pillow-rs/src/raster/color/pixel_luma.rs) | 6 | 6 | 0 | 2 | generic pixel conversion reachability audit |
| 44 | [pillow-rs/src/ops/rotate.rs](../pillow-rs/src/ops/rotate.rs) | 3 | 0 | 0 | 0 | targeted public-input audit |
| 45 | [pillow-rs/src/ops/chops.rs](../pillow-rs/src/ops/chops.rs) | 2 | 0 | 0 | 0 | targeted public-input audit |
| 46 | [pillow-rs/src/ops/enhance.rs](../pillow-rs/src/ops/enhance.rs) | 2 | 1 | 1 | 0 | targeted public-input audit |
| 47 | [pillow-rs/src/ops/resize.rs](../pillow-rs/src/ops/resize.rs) | 2 | 1 | 0 | 1 | targeted public-input audit |
| 48 | [pillow-rs/src/ops/split.rs](../pillow-rs/src/ops/split.rs) | 1 | 0 | 0 | 0 | targeted public-input audit |
| 49 | [pillow-rs/src/ops/transpose.rs](../pillow-rs/src/ops/transpose.rs) | 1 | 1 | 0 | 0 | targeted public-input audit |
| 50 | [pillow-rs/src/pipeline.rs](../pillow-rs/src/pipeline.rs) | 1 | 1 | 0 | 0 | pipeline reachability audit |
| 51 | [pillow-rs/src/raster/traits/view.rs](../pillow-rs/src/raster/traits/view.rs) | 1 | 1 | 1 | 0 | generic view reachability audit |

## Accepted input: `Thumbnail` size metadata

Coverage MCP source context identified `image.rs:338` as the only uncovered
arm in `known_pipeline_op_dimensions` for the public `Thumbnail` operation. A
focused workflow using `Image.new("RGB", (46, 22), ...)`, the public
`Image.thumbnail((23, 11), 0)`, the public `size` property, and final
`tobytes` passed exact parity with every observation executed on both sides.
The proportional dimensions are intentional: a non-proportional probe also
exposed an existing target-vs-Pillow thumbnail aspect-ratio mismatch
(`31x19` bounded by `23x11`), so it was not retained as a coverage input.

The retained case was included in bounded all-GPU Coverage MCP run
`8c16b69a-4630-4fd8-ab74-02c088fbe79e`, which passed in 190.157 seconds and
ingested snapshot `ae6dc5b8-ff20-4523-b063-0f7fb957cc6b`. Compared with
`b03eb6ff-68e8-471c-9c0a-4adc70b7afa1`, it added one covered line and two
covered regions overall; `image.rs:338` recorded six hits. Aggregate coverage
is now 34,278/39,438 lines and 55,105/64,241 regions, with no denominator,
branch, or function changes.

## Reachability disposition: SIMD scalar `posterize`

Coverage MCP snapshot `ae6dc5b8-ff20-4523-b063-0f7fb957cc6b` still reports
`pillow-rs/src/compute/pool_simd/ops/scalar.rs:87-106` red. The public
`ImageOps.posterize` contract creates a pipeline only for logical modes `L`
and `RGB`; the adapter's `native_byte_layout` covers both corresponding
`DynamicImage` layouts before it can call `scalar::posterize`. Pillow's `P`
case is the named `NotImplementedError` path, and alpha, CMYK, `I`, `F`, and
16-bit modes fail the public mode check before a `Posterize` operation is
queued. Therefore no supported public input can currently reach this scalar
function without changing dispatch or public behavior; no synthetic or
invalid case was retained.

## Accepted input: GPU transpose composition planner

The public adjacent-transpose workflow was first tested with eight valid
`Image.transpose` pairs. That input was semantically correct but produced no
new GPU fuser coverage because `Image::push_op` installed a prefix cache
between mode-preserving operations. Disabling that cache was not retained:
explicit mode selection then skipped the fuser, and adding a grayscale prefix
caused the GPU preflight to route the mixed batch to CPU. The retained cases
therefore use a public `ImageOps.grayscale` prefix followed by the transpose
composition, which exercises the planner without a private probe or a GPU
device allocation.

The retained IDs are
`after-grayscale-compose`, `after-grayscale-identity`,
`after-grayscale-stop-at-invert`, `after-grayscale-rotate90`,
`after-grayscale-rotate270`, `after-grayscale-transpose`,
`after-grayscale-transverse`, and `after-grayscale-zero-dimensions`.
Focused exact parity passed for all eight cases. The final bounded all-GPU
Coverage MCP run was `3ad9dac8-8967-45ef-86b8-56f5e971624d`, completed in
204.467 seconds, and ingested snapshot
`6a0cad1b-41ae-44f5-a874-b401421d70cc`. Relative to the campaign baseline
`ae6dc5b8-ff20-4523-b063-0f7fb957cc6b`, the snapshot reached 34,440/39,438
lines, 55,322/64,241 regions, 5,579/6,968 branches, and 2,665/3,259
functions. All lines in the helper range `pool_gpu/mod.rs:2558-2628` are now
covered; one helper branch remains red. The explicit zero-dimension
`tobytes` observation was not retained because Pillow returns empty bytes while
the target rejects zero dimensions; the retained zero-dimension case observes
only the public transpose result.

## Accepted input: GPU safety fallback for negative Gaussian radius

Pillow accepts negative `GaussianBlur` radii as a no-op. One hundred valid
public pipeline cases, IDs
`pipeline-composition.gpu-safety-negative-gaussian-000` through `-099`,
cycle supported `L`, `RGB`, and `RGBA` images and use negative radii from
`-1.0` through `-2.75` before materialization. Four representative cases
passed focused CPU parity, and a focused GPU-selected case also passed.

The registered bounded all-GPU Coverage MCP run
`3fff204a-da5c-4344-a2be-0aa29e1327bd` passed in 205.677 seconds with no
hang, timeout, cancellation, GPU-memory change, or setting change. It
ingested snapshot `45f655ec-2e8c-44c3-adc6-f61cede3eb2e`. Compared with
baseline `ae6dc5b8-ff20-4523-b063-0f7fb957cc6b`, aggregate coverage increased
by one line, one region, and one branch, to 34,441/39,438 lines,
55,323/64,241 regions, 5,580/6,968 branches, and 2,665/3,259 functions.
Coverage MCP line evidence records 100 hits on
`pillow-rs/src/compute/pool_gpu/mod.rs:3300`, the safety rejection return in
`gpu_operation_is_safe`; the fallback occurs before GPU adapter/device
initialization.

A separate valid LUT-cache experiment was pruned after the focused
GPU-selected run reported that the local environment enumerated no GPU
adapter at `Image.blend`. It is not coverage evidence and no LUT cases remain
in the maintained inputs. The earlier invalid scalar fallback batch likewise
remains excluded from coverage claims.

## Zero-gain audit: palette expansion before `ImageOps.equalize`

One hundred public `P` workflows with an attached RGB palette were tested to
look for a new route through `image.rs:2226-2247`. Four representative CPU
cases and one GPU-selected case passed exact parity. The bounded all-GPU
Coverage MCP run `f8a0ea26-87ba-4112-9282-f9cd6af912fd` passed in 209.338
seconds and ingested snapshot `7e62fc40-0b77-4751-963f-84afb1e4e144`, but
Coverage MCP reported zero aggregate delta: the palette expansion lines were
already covered by existing inputs.

A second 100-case public `PA` probe batch without an RGB palette also passed
the oracle/target result comparison, but `ImageOps.equalize` rejects `PA` at
the public mode check, so it did not reach the pipeline fallback. Its bounded
all-GPU run `4f2693fe-e3b2-45b0-aa3b-6c8f93438582` passed in 195.314 seconds
and ingested snapshot `81b0d745-96ec-4d02-a0d3-349908459bad`; it likewise
produced zero aggregate delta. Narrow MCP evidence still shows
`image.rs:2244` (the `execute_prepared` error arm) and `image.rs:2246` (the
no-palette fallthrough) uncovered. Both batches were pruned from the
maintained inputs because neither added honest coverage.

## Zero-gain audit: typed SIMD `flip` fallback

One hundred valid public `ImageOps.flip` workflows were temporarily added
using `I` and `F` images with odd heights, targeting the packed scalar fallback
at `pool_simd/ops/scalar.rs:185-186`. Focused CPU parity for representative
cases and one GPU-selected parity case passed. The bounded all-GPU Coverage
MCP run `cce5a613-4791-4ecd-b5de-02646e59be43` passed in 217.800 seconds and
ingested snapshot `1014a5f5-4b71-4059-8fe6-24d354df485d`, but aggregate
coverage was unchanged from snapshot
`81b0d745-96ec-4d02-a0d3-349908459bad`.

Line-level MCP evidence at the resulting snapshot records zero hits on both
lines 185 and 186. The retained run log also contains no `typed-flip` case
execution, so the public workflows did not reach this implementation path
under the maintained coverage runner. The temporary batch was pruned; the
scalar middle-row cleanup remains unproven through supported public inputs.

## Accepted input: packed scalar fallback for public `RGBa` Chops

Twenty-eight valid public `PIL.ImageChops` workflows were added for the
premultiplied-alpha `RGBa` mode: four each for `multiply`, `screen`, `darker`,
`lighter`, `difference`, `subtract_modulo`, and `add_modulo`. `RGBa` shares four-byte
storage with `RGBA` but is intentionally excluded from the native straight-
alpha byte adapters, so these cases exercise the packed scalar dual-image
fallback. All seven representative operations passed focused exact parity.

The bounded all-GPU Coverage MCP run
`79b5c9bc-8fa6-4c95-abe2-60b9516548f0` passed in 195.176 seconds with exit
code 0 and ingested snapshot `676010ed-d6db-4002-868d-cb6b9d344bca`. Relative
to snapshot `1014a5f5-4b71-4059-8fe6-24d354df485d`, aggregate coverage gained
7 lines, 12 regions, and 7 branches: 34,448/39,438 lines,
55,335/64,241 regions, 5,587/6,968 branches, and 2,665/3,259 functions.
MCP line evidence records hits on scalar fallback lines 332, 368, 396, 424,
460, 493, and 526. A follow-up bounded run
`6aeacd3d-a846-471c-94fb-cc1c1ddd9fe5` passed in 193.638 seconds and ingested
snapshot `e4b2e00b-27aa-4163-b294-744d64635ccc`; relative to the first RGBa
snapshot it added one line, two regions, and one branch, covering
`scalar.rs:493`. The batch is retained; the straight-alpha `L` fallback arms
at lines 360, 365, 452, and 457 remain red because supported native byte
inputs route through the exact native adapters instead.

## Accepted input: public nonzero-mask `ImageOps.autocontrast`

One hundred valid public `PIL.ImageOps.autocontrast` workflows were added
with a real selected pixel in the public `1` or `L` mask. The existing mask
cases intentionally preserve Pillow's all-zero-mask identity behavior; this
bounded matrix selects one mask pixel so the complementary histogram branch
is exercised without embedding an output or calling a native helper. The
image modes alternate between `L` and `RGB`, and the cases observe the public
result bytes.

Four representative cases passed focused exact CPU parity. The bounded
all-GPU Coverage MCP run `a35ecc53-5d39-4ba1-b880-b83460024fc2` passed in
195.126 seconds with exit code 0 and ingested snapshot
`35502193-0867-4a4f-b641-0f5ed0799fab`; no hang, timeout, cancellation,
GPU-memory change, or setting change occurred. Relative to the preceding
snapshot `e4b2e00b-27aa-4163-b294-744d64635ccc`, aggregate coverage gained
6 lines, 7 regions, and 2 branches: 34,455/39,438 lines,
55,344/64,241 regions, 5,590/6,968 branches, and 2,665/3,259 functions.
Coverage MCP source evidence confirms that
`pillow-rs/src/compute/pool_cpu/ops/imageops.rs:180-181` is now covered.

## Zero-gain audit: typed `I;16*` transpose arms

One hundred public `I;16*` `Image.frombytes` transpose workflows were
temporarily added across the four supported byte orders and five direct
flip/rotate methods. Five representative CPU cases passed exact parity. The
bounded all-GPU Coverage MCP run `543409eb-f201-4032-9753-2603c301c8d0`
passed in 210.599 seconds with exit code 0 and ingested snapshot
`b356f2aa-2b0a-4aca-a6a4-e8c681ff16e8`.

Compared with the accepted snapshot
`35502193-0867-4a4f-b641-0f5ed0799fab`, Coverage MCP reported zero delta in
lines, regions, branches, and functions. The typed `DynamicImage` transpose
arms therefore were not reached by these supported public workflows under
the maintained coverage runner. The temporary matrix was pruned; no
coverage or denominator data was changed.

## Zero-gain audit: materialized empty-image `ImageChops.offset`

Two valid public `ImageChops.offset(...).tobytes()` workflows were temporarily
added for zero-width `L` and zero-height `RGBA` images to target the empty-image
guard at `pillow-rs/src/compute/pool_simd/ops/scalar.rs:753`. Both focused CPU
parity cases passed.

The registered bounded all-GPU Coverage MCP run
`503e7359-513d-4d3c-a6a0-9c3c23485588` passed in 196.316 seconds with exit code
0 and no hang, timeout, cancellation, GPU-memory change, or setting change. It
ingested snapshot `161c39da-85fd-4a88-a6dc-7757853a7bc1`. Compared with the
accepted snapshot `35502193-0867-4a4f-b641-0f5ed0799fab`, Coverage MCP reported
zero delta in lines, regions, branches, and functions. MCP line evidence still
records zero hits on scalar offset line 753; the public workflows therefore did
not reach that SIMD scalar helper under the maintained all-GPU coverage runner.
The temporary two-case batch was pruned; no coverage or denominator data was
changed.

## Accepted input: zero-height native draw rectangle

Coverage MCP ranked the native CPU draw helper's empty-canvas branch at
`pillow-rs/src/compute/pool_cpu/ops/draw.rs:253,259` as a reachable public
edge. The new case
`PIL.ImageDraw.ImageDraw.rectangle.nuanced.coverage-batch-draw-rectangle-zero-height`
creates a valid `L` image of size `8x0`, draws a rectangle with both fill and
outline, and observes `tobytes()` on the receiver. Focused exact parity passed
1/1. Commit `12b4747d4` added only the generator and generated input manifests;
no runtime code, outputs, hashes, thresholds, filters, coverage counts, or
denominators changed.

The managed strict CPU/SIMD/GPU Coverage MCP run
`ef4a2ca0-5a8d-40da-9992-842d40def805` passed all 24 plans with zero failures
and ingested snapshot `6ed880d9-34cd-46d4-9562-41e5a8018e79` in 316.954
seconds. Against snapshot `2569675e-e97f-4711-a87d-7df7babb8814`, MCP
measured `+1` covered region, `+1` covered line, and `+2` covered branches;
function coverage was unchanged. The aggregate is now 55,799/62,330 regions
(89.522%), 34,682/38,285 lines (90.589%), 5,600/6,736 branches (83.135%),
and 2,684/3,161 functions (84.910%). Free memory ranged from 53% to 67%
during the GPU run; it completed without a hang, timeout, or cancellation.

## Reachability classification: typed RGB/RGBA fallback paths

Coverage MCP still reports the typed fallback regions at
`pillow-rs/src/ops/analysis.rs:312-331` and
`pillow-rs/src/ops/pil_resize.rs:113-115` as uncovered. The existing public
16-bit RGB/RGBA PNG inputs are valid, but the focused parity result for
`PIL.Image.Image.transpose.nuanced.coverage-batch-dynamic-typed-transpose-rgb16-png-0`
reports public mode `RGB` and 12-byte RGB output, not a public 16-bit typed
mode. The supported Python `Image.frombytes` surface also accepts only byte
`RGB`/`RGBA` layouts or scalar `I;16*` layouts. The remaining typed enum arms
therefore require an internal `DynamicImage` construction rather than a
supported public input, so adding more PNG or resize cases is expected to be
zero-gain until the public decoder contract changes. No runtime code or
fixture was changed for this classification.

## Zero-gain audit: scalar-source `Image.convert` destinations

Four valid public `Image.convert` workflows (`I/F → CMYK` and `I/F → P`) were
temporarily added to target the nonstandard-source branches at
`pillow-rs/src/ops/convert.rs:423-429` and `794-804`. All four focused CPU
parity cases passed exactly, and the registered coverage artifact selected all
four case IDs.

The registered bounded all-GPU Coverage MCP run
`477fdfa2-caee-4e3c-9f24-c5028f054deb` passed in 196.844 seconds with exit
code 0 and ingested snapshot
`3b54f900-5fcc-4dcc-a054-ddafdc2c0f55`. No hang, timeout, cancellation,
GPU-memory change, or setting change occurred. Compared with accepted snapshot
`6edb6cb5-91d9-40c4-a05a-df175235662d`, Coverage MCP reported zero delta in
lines, regions, branches, and functions: 34,472/39,438 lines,
55,389/64,241 regions, 5,595/6,968 branches, and 2,667/3,259 functions.
MCP source evidence still marks both target ranges red. The temporary
four-case batch was pruned; no coverage, denominator, filter, output, hash, or
threshold data was changed.

## Zero-gain audit: paletted odd-height `ImageOps.flip`

One valid public `PIL.ImageOps.flip` workflow on a `P` image with size `4x3`
was temporarily added to target the packed SIMD fallback's odd-height middle
row at `pillow-rs/src/compute/pool_simd/ops/scalar.rs:184-186`. The focused CPU
parity case passed exactly, and the registered coverage artifact selected the
case ID.

The registered bounded all-GPU Coverage MCP run
`25f5a59b-b12e-4f90-91a7-6bbbbed3f6a4` passed in 197.601 seconds with exit
code 0 and ingested snapshot
`b2632a21-f6f1-4edd-98fd-62dd6ebfc4bd`. No hang, timeout, cancellation,
GPU-memory change, or setting change occurred. Compared with accepted snapshot
`6edb6cb5-91d9-40c4-a05a-df175235662d`, Coverage MCP reported zero delta in
lines, regions, branches, and functions: 34,472/39,438 lines,
55,389/64,241 regions, 5,595/6,968 branches, and 2,667/3,259 functions.
MCP source evidence still marks scalar lines 185-186 red and line 184 as a
branch gap. The public case is valid, but the registered combined backend lane
does not execute this SIMD scalar fallback for it. The temporary case was
pruned; no coverage, denominator, filter, output, hash, or threshold data was
changed.

## Zero-gain audit: deferred `PutData` mode planner

Four public `Image.open(...).putdata(...)` workflows (L/LA/RGB/RGBA) were
temporarily added using committed/inline encoded inputs and immediate `.mode`
observations. All four focused CPU parity cases passed.

The bounded all-GPU Coverage MCP runs
`258c04c1-8fb8-45f7-920d-7e6be9dc3bf9` and
`93b111a6-7630-44b6-9409-90f5d3b1a8d2` passed in 207.964 and 196.917 seconds,
respectively, with exit code 0 and ingested snapshots
`022af5c5-3faa-46be-b998-f87b3fd23993` and
`e62f172b-8d13-4a21-8fb5-bb50decb918b`. No hang, timeout, cancellation,
GPU-memory change, or setting change occurred. Coverage MCP reported zero
delta against accepted snapshot `0ce19b70-332c-4650-898d-c59c6bda6fae`.

MCP source evidence shows `PipelineOp::PutData` is included in
`op_preserves_mode`; `Image::mode` returns the source mode through that fast
path before calling `known_pipeline_op_mode`. The red `PutData` planner branch
is therefore unreachable through supported public inputs under the current
core design. The temporary cases were pruned; no coverage or denominator data
was changed.

## Accepted input: large public transpose/transverse methods

Coverage MCP source and dispatch evidence showed that the existing large
transpose matrix covered methods `0-4` only. Those methods use the flip/rotate
paths; the CPU tiled block at
`pillow-rs/src/compute/pool_cpu/ops/geometry.rs:1332-1334` is selected by the
public `TRANSPOSE` and `TRANSVERSE` methods (`5` and `6`). Eight bounded public
workflows were added across `L`, `LA`, `RGB`, and `RGBA` at the existing
`512x512` size.

All eight cases passed focused exact CPU parity. The registered bounded
all-GPU Coverage MCP run `7a587ec7-00a2-4e38-9b10-1219993c7c8d` passed in
217.121 seconds with exit code 0 and ingested snapshot
`3ba022a1-9b9b-4576-bb5d-e3b316bf4462`; no hang, timeout, cancellation,
GPU-memory change, or setting change occurred. All 24 coverage plans
completed with 10,128 passed and 0 failed cases. Compared with the accepted
snapshot `161c39da-85fd-4a88-a6dc-7757853a7bc1`, Coverage MCP reported a gain
of 2 lines, 12 regions, and 1 branch: 34,457/39,438 lines,
55,356/64,241 regions, 5,591/6,968 branches, and 2,665/3,259 functions.
The geometry file now has only nine uncovered lines; the tiled transpose
region is no longer red.

## Rejected probe: unsupported non-numeric `putpixel` value

A temporary two-case probe used string values for public `Image.putpixel` to
target the invalid-value classifier at `pillow-rs/src/image.rs:2813-2818`.
The fixed manifest declares the public `value` parameter as `integer | number |
sequence`, so contract validation rejected both inputs before parity execution.
No Coverage MCP run was submitted and no coverage or denominator data changed.
The temporary cases were pruned; this classifier remains outside the supported
input surface.

## Zero-gain audit: explicit YCbCr/HSV brightness fallback

Two valid public `ImageEnhance.Brightness.enhance` workflows were temporarily
added for explicit `YCbCr` and `HSV` images to target the no-alpha fallback at
`pillow-rs/src/compute/pool_simd/ops/scalar.rs:133`. Both focused CPU parity
cases passed exact parity, and the all-GPU coverage plan selected and passed
both new case IDs.

The registered bounded all-GPU Coverage MCP run
`3d26a34a-9c07-4a70-8cba-90a12141206b` passed in 210.680 seconds with exit
code 0 and ingested snapshot `0ce19b70-332c-4650-898d-c59c6bda6fae`; no hang,
timeout, cancellation, GPU-memory change, or setting change occurred.
Compared with the accepted snapshot `3ba022a1-9b9b-4576-bb5d-e3b316bf4462`,
Coverage MCP reported zero delta in lines, regions, branches, and functions:
34,457/39,438 lines, 55,356/64,241 regions, 5,591/6,968 branches, and
2,665/3,259 functions. MCP line evidence still records zero hits on scalar
line 133, so the public workflows did not reach that helper under the
maintained all-GPU coverage runner. The temporary two-case batch was pruned;
no coverage or denominator data was changed.

## Accepted input: public FASTOCTREE insertion-sort fallback

One hundred valid public `PIL.Image.Image.quantize(method=2)` workflows were
added with deterministic RGB `frombytes` payloads. Each payload uses 128
distinct high-nibble octree buckets with distinct frequencies, varying bucket
order, frequency order, and requested palette size. This is an ordinary
supported quantization input; the payload generator stores no expected output
or hash.

Representative cases 0–3 passed focused exact CPU parity. The maintained input
regeneration reproduced exactly. The input-check command then exposed one
pre-existing contract-test failure in
`test_grouped_benchmark_timing_excludes_result_encoding` (`StopIteration` from
its mocked `time.perf_counter_ns`), after reporting that the parity inputs and
crash quarantine reproduced exactly; this did not affect the parity cases.

The registered bounded all-GPU Coverage MCP run
`b7df908b-0d5e-413a-a4e3-a4eb675bbf48` passed in 206.805 seconds with exit
code 0 and ingested snapshot `6edb6cb5-91d9-40c4-a05a-df175235662d`. No hang,
timeout, cancellation, GPU-memory change, or setting change occurred.
Compared with accepted snapshot `18badb14-932f-47c1-8f1e-380d13e2b1fa`,
Coverage MCP reported +6 lines, +13 regions, +4 branches, and +1 function:
34,472/39,438 lines, 55,389/64,241 regions, 5,595/6,968 branches, and
2,667/3,259 functions. MCP line evidence marks
`pillow-rs/src/ops/quantize.rs:1521` and `1632-1635` improved, with hits
1116, 1512, 1512, 396, and 1116 respectively. The batch is retained.

## Zero-gain audit: `Image.merge` → `putdata` mode-cache probe

Four valid public `Image.merge(...).putdata(...)` workflows (L/LA/RGB/RGBA)
were temporarily added with immediate `.mode` observations to target the
deferred `PutData` mode planner and `pixel_mode_name` at
`pillow-rs/src/image.rs:240-255,276`. All four focused CPU parity cases passed
exactly, and the registered coverage artifact selected all four case IDs.

The registered bounded all-GPU Coverage MCP run
`c61c6523-af8e-4816-a53a-447d786a7680` passed in 193.253 seconds with exit
code 0 and ingested snapshot
`931000a9-cca0-4fc6-8459-6f9acd90704e`. No hang, timeout, cancellation,
GPU-memory change, or setting change occurred. Compared with accepted snapshot
`6edb6cb5-91d9-40c4-a05a-df175235662d`, Coverage MCP reported zero delta in
lines, regions, branches, and functions: 34,472/39,438 lines,
55,389/64,241 regions, 5,595/6,968 branches, and 2,667/3,259 functions.
MCP source evidence still marks `pixel_mode_name` lines 240-255 and the
`PutData` planner return at line 276 as uncovered. Under the current public
pipeline semantics, these inputs therefore do not reach the intended helper
despite being valid and parity-correct. The temporary four-case batch was
pruned; no coverage, denominator, filter, output, hash, or threshold data was
changed.

## Accepted input: repeated public GPU LUT cache

One hundred valid public `PIL.Image.Image.point` workflows were added across
L, LA, RGB, and RGBA. Each workflow applies the same deterministic expanded
LUT, calls public `ImageOps.mirror`, and applies that LUT again. The mirror
keeps the two point operations non-adjacent, so the GPU planner cannot fuse
them away and the execution-wide repeated-LUT cache is exercised through
supported inputs. No expected output, hash, or coverage metadata is stored in
the generator.

Representative cases 0–3 passed focused exact CPU parity. The maintained input
regeneration produced all 100 cases, and the registered all-backend coverage
lane selected 10,327 parity cases across 24 plans with zero failures.

The registered bounded all-CPU/SIMD/GPU Coverage MCP run
`d4362460-5318-44c5-91b2-509fadf7ea25` passed in 199.610 seconds with exit
code 0 and ingested snapshot `9840bd6a-2147-4490-8fdd-a686392ddecd`. No hang,
timeout, cancellation, GPU-memory change, or setting change occurred.
Compared with baseline snapshot `3b54f900-5fcc-4dcc-a054-ddafdc2c0f55`,
Coverage MCP reported +21 lines, +28 regions, +8 branches, and +0 functions:
34,493/39,438 lines, 55,417/64,241 regions, 5,603/6,968 branches, and
2,667/3,259 functions. MCP source evidence marks
`pillow-rs/src/compute/pool_gpu/mod.rs:147-155` and the associated LUT arena
upload/reuse ranges at `1501-1510`, `1661-1662`, and `1752` covered by this
batch. The retained cases do not reach the separate capacity-overflow and
cache-growth fallbacks at lines 143, 156, 1664, and 1765-1768; those remain
documented as the next bounded GPU target, subject to safe public-input
construction.

## Accepted input: public GPU LUT-arena growth

One hundred valid public `PIL.Image.Image.point` workflows were added across
L, LA, RGB, and RGBA. Each applies repeated LUT A on both sides of a public
`ImageOps.mirror`, mirrors again, and then applies a distinct LUT B. This
keeps LUT A in the execution-wide cache while forcing the bounded local LUT
arena to grow for LUT B; all behavior remains expressed through public image
inputs and no expected output or hash is embedded.

Representative cases 0–3 passed focused exact CPU parity. The maintained input
regeneration produced all 100 cases, and the registered all-backend coverage
lane selected 10,427 parity cases across 24 plans with zero failures.

The registered bounded all-CPU/SIMD/GPU Coverage MCP run
`765afb8c-7436-4e0e-9f44-655e91e64d66` passed in 204.719 seconds with exit
code 0 and ingested snapshot `25b52c40-b837-4990-a2d4-5f1c5f6a8d6e`. No hang,
timeout, cancellation, GPU-memory change, or setting change occurred.
Compared with baseline snapshot `9840bd6a-2147-4490-8fdd-a686392ddecd`,
Coverage MCP reported +7 lines, +5 regions, +2 branches, and +0 functions:
34,500/39,438 lines, 55,422/64,241 regions, 5,605/6,968 branches, and
2,667/3,259 functions. MCP source evidence marks
`pillow-rs/src/compute/pool_gpu/mod.rs:1764-1768` improved and covered by this
family. The remaining repeated-resource capacity fallbacks at lines 143, 156,
and 1664 require exceeding the fixed auxiliary-cache budget or bypassing the
execution-wide cache; they are not attempted with oversized or unbounded
public workloads.

## Accepted input: scalar I/F-to-PA conversion dispatch

Coverage MCP source review found that the maintained public I/F-to-PA cases
already existed, but `Image::convert_with_input` intercepted them in the
generic scalar I/F dispatch and recursively converted through `L`. That left
the existing dedicated PA converter's direct clamped-index helpers
(`color.rs:994-1022`) unreachable. The core dispatch now excludes `PA` from
that scalar shortcut, so supported I/F-to-PA inputs reach
`convert_to_palette_alpha` without adding a private probe or changing the
public input contract.

Focused exact parity passed for representative I-to-PA and F-to-PA cases and
for both existing unsupported-source compatibility cases. The change was
committed and pushed as `cdab5218a`.

The strict managed CPU/SIMD/GPU Coverage MCP run was
`a03b1371-1f15-45db-bcd0-bb7b22601b47`; all 24 plans and 10,873 tests passed
with zero failures, and the run ingested snapshot
`f34b8a36-5e3e-4526-abe5-216d9d7a2483` in 290.582 seconds. Compared with the
parent snapshot `db30d43a-7594-46ef-8f4c-ee92806f01c9`, Coverage MCP measured
`+102` covered regions, `+42` lines, `+4` branches, and `+4` functions. The
aggregate result is 55,732/62,314 regions (89.437%), 34,628/38,275 lines
(90.472%), 5,578/6,728 branches (82.907%), and 2,684/3,161 functions
(84.910%). The one-region denominator increase is the executable branch
introduced by the source change; no fixture denominator, filter, threshold,
output, hash, or coverage-count file was edited. MCP line evidence marks the
I/F-to-RGB helpers green.

## Accepted input: public L-source palette remap

Coverage MCP source review found that `op_remap_palette` already had a
publicly appropriate L-source branch, but the pipeline passed its P result tag
as the kernel's mode. That caused an L image to be interpreted as raw palette
indices. The executor now passes the source mode for a one-step
`RemapPalette`; the result remains P-tagged and retains its reordered palette.

Focused exact parity passed for the maintained L-source pipeline case, the
manifest L-mode case, and an attached-palette P-source case. The core fix was
committed and pushed as `bb2c84823`.

The strict managed CPU/SIMD/GPU Coverage MCP run was
`2f54a9e3-42f1-4e43-9f25-503b8b0b6b56`; all 24 plans and 10,873 tests passed
with zero failures, and it ingested snapshot
`cd281200-a551-48ae-aa74-846a6ee7df6e` in 272.616 seconds. Compared with the
prior executable snapshot `f34b8a36-5e3e-4526-abe5-216d9d7a2483`, Coverage
MCP measured `+33` covered regions, `+14` lines, `+6` branches, and `+0`
functions. The aggregate result is 55,765/62,326 regions (89.473%),
34,642/38,280 lines (90.496%), 5,584/6,732 branches (82.947%), and
2,684/3,161 functions (84.910%). The source edit added 12 regions, 5 lines,
and 4 branches to the executable denominator; no fixture denominator,
filter, threshold, output, hash, or coverage-count file was edited. MCP line
evidence marks the L-source branch green; its non-L fallback remains
classified as unreachable under the supported public mode contract.

## Current MCP ceiling audit: active supported-input corpus

Coverage MCP insight at snapshot
`cd281200-a551-48ae-aa74-846a6ee7df6e` ranks the remaining uncovered source
regions as follows. This is the measured ceiling for the current maintained
public-input corpus, not a claim that every hypothetical private or unsupported
input should execute. The strict managed CPU/SIMD/GPU run passed all 24 plans
and 10,873 tests with zero failures; it reports 55,765/62,326 regions
(89.473%), 34,642/38,280 lines (90.496%), 5,584/6,732 branches (82.947%),
and 2,684/3,161 functions (84.910%).

| rank | file | uncovered lines | MCP reachability reason | disposition |
| ---: | --- | ---: | --- | --- |
| 1 | `pillow-rs/src/compute/pool_gpu/mod.rs` | 457 | Device, allocation, cache-capacity, and error fallbacks; forcing them requires unsafe-sized or failure-inducing workloads. | Keep bounded; do not force. |
| 2 | `pillow-rs/src/image.rs` | 354 | Planner fallbacks for invalid modes, malformed descriptors, and non-finite dimensions rejected or normalized at public boundaries. | Keep as invariant guards. |
| 3 | `pillow-rs/src/compute/pool_simd/ops/adapters.rs` | 315 | Invalid LUT lengths and impossible fixed-width conversions are checked before the native adapter is called. | Keep as adapter guards. |
| 4 | `pillow-rs/src/raster/dynamic.rs` | 286 | Typed clone/clone-from arms are not reached by the maintained public decoder routes; the prior typed PNG batch produced zero unique gain. | Pruned as zero-gain for now. |
| 5 | `pillow-rs/src/compute/pool_simd/ops/scalar.rs` | 264 | Remaining alpha/channel fallback arms belong to internal packed-mode dispatch; supported public routes select other adapters. | No input-only route found. |
| 6 | `pillow-rs/src/compute/registry.rs` | 241 | Registry initialization succeeds; missing entries are internal descriptor/error paths or operations that bypass this lookup. | Keep registry safety paths. |
| 7 | `pillow-rs-py/src/lib.rs` | 241 | Iterator constructor/error branches and wrapper helpers are not selected by the active public-input plans. | Defer until a maintained public case exists. |
| 8 | `pillow-rs/src/compute/mod.rs` | 227 | Invalid backend parsing and telemetry controls are control-plane APIs outside the active image-input manifest. | Defer; no filter changes. |
| 9 | `pillow-rs/src/lib.rs` | 216 | The largest remaining ranges are font constructors and other exported entry points outside this campaign’s allowed font lane. | Deferred; do not touch fontdone. |
| 10 | `pillow-rs/src/raster/traits/primitive.rs` | 103 | Generic primitive implementations are not instantiated by the supported public raster paths measured here. | Keep generic implementations. |

The lower-ranked CPU candidates were also sampled with MCP source review:
empty-image histogram returns, invalid ImageOps colors, unsupported merge
modes, zero-coefficient resize guards, raw-buffer shape errors, and malformed
transform inputs are either rejected before queueing, impossible after checked
allocation, or outside the supported public input contract. No output, hash,
threshold, filter, denominator, or coverage-count file was changed during
this audit.

## Pruned probe: typed PNG `copy()` arms

Two maintained `PIL.Image.Image.copy()` cases were added for 16-bit RGB and
RGBA PNG inputs. Focused parity passed for both cases. The first managed
Coverage MCP run (`811c0772-d466-467f-95b2-ede0610fe00a`, snapshot
`2dc1339e-4f1b-4a66-993b-bc2fccc45938`) showed no coverage delta because the
opened image remained byte-backed until copied. A second run
(`63c93190-351b-48ff-b012-bd71ee0e10d2`, snapshot
`07323456-6669-4a98-a0f0-b31ea9c0376a`) materialized the image with public
`load()` first and still showed zero delta: 55,765/62,326 regions (89.473%),
with unchanged lines, branches, and functions.

Read-only inspection of the maintained PNG decoder confirmed that 8- and
16-bit RGB/RGBA PNG samples are normalized to the public `Rgb8`/`Rgba8`
routes. The typed `DynamicImage` clone arms therefore cannot be reached by
these supported public inputs. The two cases and their generator chain were
removed; no outputs, hashes, thresholds, filters, coverage counts, or
denominators were changed.

## Pruned parity-invalid candidate: `I;16` shape default ink

Coverage MCP source review identified `pillow-rs/src/draw/mod.rs:514` as the
fallback arm of `Draw::default_shape_ink`. A temporary input-only public case,
`PIL.ImageDraw.ImageDraw.shape.nuanced.coverage-batch-draw-shape-i16-default-ink`,
used `ImageDraw.shape` on an `I;16` image with both colors omitted. Focused
parity failed 0/1: Pillow produced a nonzero implicit shape ink, while the
Rust path returned a no-op for that mode. The candidate was removed before a
managed run; no runtime code, outputs, hashes, thresholds, filters, coverage
counts, or denominators changed.

## Pruned zero-gain candidate: empty ImageEnhance contrast means

Coverage MCP ranked `pillow-rs/src/compute/pool_cpu/ops/enhance.rs:21,135,169`
as uncovered empty-buffer and `n == 0` fallbacks. Two supported public
workflows were added temporarily:
`PIL.ImageEnhance.Contrast.nuanced.zero-size-cmyk-empty-mean` and
`PIL.ImageEnhance.Contrast.nuanced.zero-size-l-empty-mean`. Both passed focused
parity and were selected by the managed ImageEnhance plan.

The correctly attributed managed run
`8df0becd-2385-42cb-8917-36d95da70b6f` passed all 24 plans in 318.331 seconds
and ingested snapshot `b7106530-f59b-4d4e-be45-6bce9d8bb449` at commit
`10073ea8d`. MCP measured zero change from the prior snapshot: 55,801/62,331
regions (89.524%), 34,684/38,286 lines (90.592%), 5,600/6,736 branches
(83.135%), and 2,684/3,161 functions (84.910%). Source review kept all three
regions red. The public lazy zero-area pipeline short-circuits before
`op_enhance_contrast` executes, so its internal empty fallbacks are not
reachable through supported public input. The temporary cases were removed;
no runtime code, outputs, hashes, thresholds, filters, coverage counts, or
denominators were changed.

## Accepted input: non-positive `ImageOps.scale` factors

Coverage MCP source review identified `pillow-rs/src/image.rs:283-296` as a
candidate invalid-dimension fallback. A public negative-factor probe showed
that this fallback is not reachable through supported `ImageOps.scale`
inputs: Pillow rejects `factor <= 0` during the public call, while the Rust
wrapper previously deferred the value and returned a clamped 1x1 image.

The core now validates the factor in
`pillow-rs/src/ops/imageops.rs:828-835`, preserving the public error boundary
and leaving the planner fallback as an invariant guard. Focused exact parity
passed for both maintained probes:

- `PIL.ImageOps.scale.nuanced.coverage-batch-imageops-edge-scale-negative-factor-rgb-0`
- `PIL.ImageOps.scale.nuanced.coverage-batch-imageops-edge-scale-negative-factor-l-1`

The fix and inputs are in commit `3aa557bdc`. The managed strict CPU/SIMD/GPU
Coverage MCP run `ef390d93-1e44-4e17-a2a9-3e78aff9adc5` passed all 24 plans
with zero failures and ingested snapshot
`6aff81e6-d00a-4c0f-abea-419211e21d37` in 282.934 seconds. Against snapshot
`07323456-6669-4a98-a0f0-b31ea9c0376a`, MCP measured `+4` covered regions,
`+5` covered lines, and `+3` covered branches; the function count was
unchanged. The aggregate is now 55,769/62,330 regions (89.474%),
34,647/38,285 lines (90.498%), 5,587/6,736 branches (82.942%), and
2,684/3,161 functions (84.910%). Free memory stayed between 70% and 72%
during the GPU run.

## Accepted input: non-uniform `quantize(colors=1)`

Coverage MCP source review identified `pillow-rs/src/ops/quantize.rs:1247-1248`
as a reachable one-color palette-mapping early return that the existing
quantizer matrix did not exercise because it started at `colors=2`. The new
public case
`PIL.Image.Image.quantize.nuanced.coverage-batch-quantize-single-color-rgb-0`
uses a non-uniform RGB `frombytes` payload, requests `colors=1`, and observes
`tobytes()` so the lazy quantizer is materialized.

Focused exact parity passed 1/1. Commit `f692ef9ef` added only the generator
and generated input manifests; no outputs, hashes, thresholds, filters,
coverage counts, or denominators changed. The managed strict CPU/SIMD/GPU
Coverage MCP run `e9c5ad96-7f8d-45ed-b72d-394d47d6eee4` passed all 24 plans
with zero failures and ingested snapshot
`088a15bd-1fdc-4cd4-9f6d-6702bbb21b8e` in 236.909 seconds. Against the prior
snapshot `6aff81e6-d00a-4c0f-abea-419211e21d37`, MCP measured `+2` covered
regions, `+1` covered line, and `+1` covered branch; function coverage was
unchanged. The aggregate is now 55,771/62,330 regions (89.477%),
34,648/38,285 lines (90.500%), 5,588/6,736 branches (82.957%), and
2,684/3,161 functions (84.910%). Free memory stayed between 69% and 70%
during the GPU run.

## Accepted input: large native-byte `RankFilter`

Coverage MCP source review identified the native-byte rank dispatch in
`pillow-rs/src/compute/pool_simd/ops/adapters.rs:1957-1963` and
`2304-2320` as reachable when a public byte image exceeds 64x64 pixels and no
explicit mode override is supplied. Existing public rank cases were all
below that threshold and therefore exercised only the scalar adapter.

The new public case
`PIL.ImageFilter.RankFilter.nuanced.coverage-batch-filter-large-rank-l-0`
uses a 513x16 `L` image and a valid 5x5, rank-12 filter. Focused exact parity
passed 1/1. Commit `9cf4675df` added only the generator and generated input
manifests; no runtime code, outputs, hashes, thresholds, filters, coverage
counts, or denominators changed. The managed strict CPU/SIMD/GPU Coverage MCP
run `bf4eed47-3ea2-4b7d-97c4-1c2dda0ad107` passed all 24 plans with zero
failures and ingested snapshot `2c0392cf-be29-47ec-8856-e1e5e04dafd3` in
230.406 seconds. Against snapshot
`088a15bd-1fdc-4cd4-9f6d-6702bbb21b8e`, MCP measured `+5` covered regions,
`+2` covered lines, and `+1` covered branch; function coverage was unchanged.
The aggregate is now 55,776/62,330 regions (89.485%), 34,650/38,285 lines
(90.505%), 5,589/6,736 branches (82.972%), and 2,684/3,161 functions
(84.910%). Free memory stayed between 67% and 68% during the GPU run.

## Accepted input: invalid `Image.merge` band item

Coverage MCP source review identified the public wrapper fallback at
`pillow-rs-py/src/lib.rs:363`, where a merge band item that is neither an
image nor `None` is converted into the core invalid-input variant. The new
case `PIL.Image.merge.nuanced.invalid-band-item-int` uses the supported public
workflow `Image.merge("L", [1])`; both source and target return the same
`AttributeError` with no fixture or output divergence.

Focused exact parity passed 1/1. Commit `dc8d382a2` added only the generator
and generated input manifests; no runtime code, outputs, hashes, thresholds,
filters, coverage counts, or denominators changed. The corrected managed
strict CPU/SIMD/GPU Coverage MCP run
`b2ba6977-2391-41ee-ac3d-98d788b4546d` passed all 24 plans with zero failures
and ingested snapshot `3994ba1b-9d78-4313-9098-eed7fe9372ea` in 263.317
seconds. Against snapshot `2c0392cf-be29-47ec-8856-e1e5e04dafd3`, MCP
measured `+4` covered regions and `+4` covered lines; branch and function
coverage were unchanged. The aggregate is now 55,780/62,330 regions
(89.491%), 34,654/38,285 lines (90.516%), 5,589/6,736 branches (82.972%),
and 2,684/3,161 functions (84.910%). Free memory ranged from 57% to 69%
during the GPU run and recovered to 64% at completion.

## Accepted input: GPU secondary-image cache budget fallback

Coverage MCP source review ranked the GPU auxiliary-cache guards at
`pillow-rs/src/compute/pool_gpu/mod.rs:127,143,156` as the highest reachable
GPU target. Existing public cache workflows reused only small images and
covered the within-budget side. The new case
`pipeline-composition.gpu-auxiliary-cache-budget` creates five distinct valid
2048x2048 `L` images and reuses each through public `ImageChops.multiply` and
`ImageChops.screen` operations. Their packed representations total 80 MiB,
so the fifth secondary-image insertion takes the existing bounded fallback.

Focused exact parity passed 1/1 on both the CPU and strict GPU target; the
observed pixel was `225` on source and target. Commit `f8d4188c8` added only
the generator and generated input manifests; no runtime code, outputs, hashes,
thresholds, filters, coverage counts, or denominators changed. The managed
strict CPU/SIMD/GPU Coverage MCP run
`00880aea-7161-44f1-bb7c-d8dcf0129cfe` passed all 24 plans with zero failures
and ingested snapshot `bac8edd2-fbb0-4eb1-a83c-3d5766cb214c` in 269.720
seconds. Against snapshot `3994ba1b-9d78-4313-9098-eed7fe9372ea`, MCP
measured `+10` covered regions, `+20` covered lines, and `+5` covered
branches; function coverage was unchanged. The aggregate is now
55,790/62,330 regions (89.507%), 34,674/38,285 lines (90.568%),
5,594/6,736 branches (83.046%), and 2,684/3,161 functions (84.910%). MCP
source review confirms line 127 is covered; the third-image and LUT fallback
lines 143 and 156 remain uncovered. Free memory ranged from 57% to 69% during
the GPU run and was 58% at completion.

## Accepted input: GPU third-image cache budget fallback

The remaining reachable GPU cache guard at
`pillow-rs/src/compute/pool_gpu/mod.rs:143` handles repeated public paste
masks after the bounded auxiliary cache is full. The new case
`pipeline-composition.gpu-paste-mask-cache-budget` uses one valid public
2048x2048 `L` source and five distinct 2048x2048 `L` masks, reusing each mask
through two `Image.paste` calls and observing one final pixel. The packed
source plus masks exceed the existing cache budget without changing any
runtime limit.

Focused exact parity passed 1/1 on both the CPU and strict GPU target; the
observed pixel was `233` on source and target. Commit `9b282ea64` added only
the generator and generated input manifests; no runtime code, outputs, hashes,
thresholds, filters, coverage counts, or denominators changed. The managed
strict CPU/SIMD/GPU Coverage MCP run
`0dca577c-90c3-469b-910f-d1ecc4b451bb` passed all 24 plans with zero failures
and ingested snapshot `f6de21e5-646f-477a-a34f-3b48ed53ba72` in 258.244
seconds. Against snapshot `bac8edd2-fbb0-4eb1-a83c-3d5766cb214c`, MCP
measured `+5` covered regions, `+6` covered lines, and `+2` covered branches;
function coverage was unchanged. The aggregate is now 55,795/62,330 regions
(89.515%), 34,680/38,285 lines (90.584%), 5,596/6,736 branches (83.076%),
and 2,684/3,161 functions (84.910%). MCP source review confirms line 143 is
covered; the LUT fallback at line 156 remains uncovered. Free memory stayed
between 65% and 67% during the focused and managed GPU runs.

## Accepted input: GPU LUT cache budget fallback

The final GPU auxiliary-cache gap was the LUT fallback at
`pillow-rs/src/compute/pool_gpu/mod.rs:156`. A valid public pipeline now fills
the cache with five reused 2048x2048 secondary images, applies one LUT through
`Image.point`, separates the second point call with public `ImageOps.mirror`,
and applies the same LUT again. The separator is required because adjacent
point operations are intentionally fused into one GPU `PointOp`; without it,
the cache sees only one LUT identity.

The focused exact parity case passed 1/1 on both CPU and strict GPU; the final
observed pixel was `169` on source and target. Commit `dbe5eedfa` added only
the generator and generated input manifests; no runtime code, outputs, hashes,
thresholds, filters, coverage counts, or denominators changed. The managed
strict CPU/SIMD/GPU Coverage MCP run
`7c7599fa-de88-45a3-ac2a-6c86078b7102` passed all 24 plans with zero failures
and ingested snapshot `8640e614-6a83-4ca9-a756-f714423c9227` in 275.887
seconds. Against snapshot `de75832a-c9de-447f-b547-cd06452046d8`, MCP
measured `+3` covered regions, `+1` covered line, and `+2` covered branches;
function coverage was unchanged. The aggregate is now 55,798/62,330 regions
(89.520%), 34,681/38,285 lines (90.586%), 5,598/6,736 branches (83.106%),
and 2,684/3,161 functions (84.910%). MCP source review confirms lines 127,
143, and 156 in the GPU auxiliary-cache guards are all covered. Free memory
ranged from 66% to 68% during the focused and managed GPU runs.

## Pruned zero-gain candidate: `I;16` to `LA` conversion

Coverage MCP ranked `pillow-rs/src/raster/dynamic.rs:245-251` as a possible
publicly reachable typed conversion gap. A temporary valid parity case,
`PIL.Image.Image.convert.nuanced.i16-frombytes-to-la`, used the existing public
`Image.frombytes("I;16")` input and a public `convert("LA")` call. Focused
parity passed 1/1, and the managed all-GPU coverage plan selected and passed
the case.

The managed run `2cd4c9d6-4c05-40cd-a39e-914f201be976` ingested snapshot
`540ee131-7a24-415f-a27e-2d6d26a27e2c` at commit `315d6a161`. Against snapshot
`8640e614-6a83-4ca9-a756-f714423c9227`, MCP measured no change in covered
regions, lines, branches, or functions; `dynamic.rs:245-251` remained red.
Source tracing explains why: `convert("LA")` queues `PipelineOp::Convert`,
and the SIMD/CPU convert adapter builds LA from `pil_grayscale` directly. It
does not call `DynamicImage::to_luma_alpha8`, so the Luma16 arm is not a
supported public-input route. The temporary case is therefore pruned rather
than retained as zero-gain coverage debt. No runtime code, outputs, hashes,
thresholds, filters, coverage counts, or denominators changed.

## Pruned zero-gain candidate: empty median-cut quantization materialization

Coverage MCP ranked `pillow-rs/src/ops/quantize.rs:258-259` as an uncovered
median-cut fallback. The maintained public case
`PIL.Image.Image.quantize.nuanced.mediancut-zero-size` already supplied a
supported empty RGB image, but observed only the returned image. A temporary
input-only change added a public `tobytes()` observation to force
materialization; focused parity passed 1/1.

The managed run `b4ef8f4e-d486-49c1-b50c-dabb179cd6de` ingested snapshot
`2569675e-e97f-4711-a87d-7df7babb8814` at commit `5662b1069` and passed all 24
plans. MCP measured no change in covered regions, lines, branches, or
functions; the aggregate remained 55,798/62,330 regions (89.520%),
34,681/38,285 lines (90.586%), 5,598/6,736 branches (83.106%), and
2,684/3,161 functions (84.910%). Source evidence shows the supported empty
input returns at `quantize.rs:234-235`, which are already covered; reaching
`258-259` would require `n > 0` with an empty collected hash table. The
temporary observation was removed and the candidate pruned. No runtime code,
outputs, hashes, thresholds, filters, coverage counts, or denominators
changed.

## Accepted input: RGBX default draw ink

Coverage MCP source review identified `pillow-rs/src/draw/mod.rs:512` as the
default shape-ink fallback for modes outside the known ImageDraw families. A
supported public `RGBX` rectangle with both `fill=None` and `outline=None`
reached that dispatch. The first focused parity run exposed a real semantic
divergence: Pillow draws the implicit white outline for RGBX, while Rust's
omitted RGBX arm returned no outline. Core dispatch now treats RGBX as an
RGB-family mode, and the focused live-oracle case passes 1/1.

Commit `fb2f07c61` contains the core fix and the input-only parity/coverage
case `PIL.ImageDraw.ImageDraw.rectangle.nuanced.coverage-batch-draw-rectangle-rgbx-default-ink`.
The managed strict CPU/SIMD/GPU Coverage MCP run
`5bd4f2f7-6ffb-4885-93ac-0b3357e168f0` passed all 24 plans and ingested
snapshot `c678251c-acee-43c9-b048-fcdb9b28495c` in 327.464 seconds. Against
snapshot `6ed880d9-34cd-46d4-9562-41e5a8018e79`, MCP measured `+1` covered
region and `+1` covered line; branch and function coverage were unchanged.
The aggregate is now 55,800/62,331 regions (89.522%), 34,683/38,286 lines
(90.589%), 5,600/6,736 branches (83.135%), and 2,684/3,161 functions
(84.910%). Memory ranged from 56% to 65% free during the run and ended at
61% free. No outputs, hashes, thresholds, filters, coverage counts, or
denominators were edited.

## Pruned zero-gain candidate: SIMD scalar flip middle row

Coverage MCP ranked `pillow-rs/src/compute/pool_simd/ops/scalar.rs:160-161` as
an uncovered region. A temporary valid public case,
`PIL.ImageOps.flip.nuanced.scalar-p-odd-height`, used an odd-height palette
image and passed focused parity 1/1. The managed run
`499601f0-6208-4726-b4cb-0e5ba98209ad` selected and passed the case, but MCP
measured no change in regions, lines, branches, or functions; the aggregate
remained 55,800/62,331 regions (89.522%), 34,683/38,286 lines (90.589%),
5,600/6,736 branches (83.135%), and 2,684/3,161 functions (84.910%).

Source tracing showed that SIMD `native_transpose` intentionally accepts
palette indices as a native one-byte layout, so the public `P` flip never
enters `scalar::flip`. The remaining middle-row branch requires a non-native
mode encoded as non-alpha; supported public mode/layout combinations do not
provide that state. The temporary input was removed rather than retained as
zero-gain coverage debt. No runtime code, outputs, hashes, thresholds,
filters, coverage counts, or denominators were edited.

## Coverage campaign continuation: exact-list `putdata` error edge

Coverage MCP identified `pillow-rs-py/src/putdata.rs:172` as the uncovered
error edge of the exact built-in-list loop. The retained public case
`PIL.Image.Image.putdata.nuanced.rgb-exact-list-invalid-tuple` supplies an
exact list whose pixel tuples have the wrong arity; focused source/target
parity passed 1/1.

The managed strict CPU/SIMD/GPU Coverage MCP run
`874a2640-2562-41b7-994e-a623c25f3476` passed all 24 plans in 282.979 seconds
and ingested snapshot `8cd5b843-97db-4b8e-8848-60cac21a7f92`. Against the
previous snapshot, MCP measured `+1` covered region and `+1` covered line;
branches and functions were unchanged. The aggregate is now 55,801/62,331
regions (89.524%), 34,684/38,286 lines (90.592%), 5,600/6,736 branches
(83.135%), and 2,684/3,161 functions (84.910%). Memory remained healthy and
ended at 68% free. Commits `47f3afe89` and the generated input manifests
contain only this input-driven case; no runtime code, outputs, hashes,
thresholds, filters, coverage counts, or denominators were changed.

## Pruned zero-gain candidates: NaN rotation and 16-bit PNG conversions

The public `Image.rotate(angle=NaN, expand=True)` probe was removed because
source Pillow raised `ValueError: cannot convert float NaN to integer`, while
the target raised `ValueError: image dimensions cannot be zero: 0×0`.

Six focused-parity-valid public conversions from RGB16/RGBA16 PNG inputs to
L, LA, RGB, and RGBA were also tested. The managed run
`61f4c9c2-9e08-4075-bcff-d0d9100eb9bd` passed all 24 plans in 307.578 seconds
and ingested snapshot `c5e6b097-64ec-42b3-88a5-0eb25e530216`, but MCP measured
zero aggregate change. Source tracing therefore classifies those typed
conversion implementations as unreachable through the current supported
public decoder path; commits `2e3e23326` and `6fe0a5c4f` add then prune the
batch. No runtime code, outputs, hashes, thresholds, filters, coverage counts,
or denominators were changed.

## Pruned zero-gain candidate: odd-height integer flip fallback

Coverage MCP ranked `pillow-rs/src/compute/pool_simd/ops/scalar.rs:160-161` as
an uncovered region. The temporary public case
`PIL.ImageOps.flip.nuanced.coverage-batch-simd-flip-i-odd-height` used a valid
5×3 integer image and passed focused parity 1/1. The managed strict
CPU/SIMD/GPU run `104b2ffd-94c8-4a47-b655-b194e48c9532` selected and passed
the case in 304.595 seconds, but MCP measured no aggregate change: coverage
remained 55,802/62,331 regions (89.525%), 34,687/38,286 lines (90.600%),
5,601/6,736 branches (83.150%), and 2,684/3,161 functions (84.910%).

Source tracing found that SIMD does execute, but `mode_to_u32` maps public
tagged `I`, `F`, and other non-native four-byte modes to mode code 3. The
middle-row condition therefore sees `has_a=true`; public supported inputs do
not reach the non-alpha branch. The input was removed rather than retained as
zero-gain coverage debt. No runtime code, outputs, hashes, thresholds,
filters, coverage counts, or denominators were edited.

## Accepted input: stringified resample-object fallbacks

Coverage MCP source review identified the unsupported-host-object conversion
paths at `pillow-rs-py/src/lib.rs:160-162`, `176`, and `181`. The maintained
image-core native corpus now supplies custom public Python objects whose
`__str__` returns an unknown filter name to `Image.resize` and `Image.rotate`.
These inputs exercise the binding's `value.str()` conversion while core owns
the resulting unknown-filter validation; the native lane passed 220/220.

Commit `e9a845bb2` contains only this coverage-only input corpus change. The
managed strict CPU/SIMD/GPU Coverage MCP run
`04dc696f-d3f1-48c5-ab4e-9d9f523e5fe2` passed in 308.588 seconds and ingested
snapshot `4ea6f42a-6054-4f0d-9a88-41f650775e96`. Against snapshot
`41c51477-4243-4a97-a37a-6657b7526315`, MCP measured `+10` covered regions,
`+5` covered lines, and `+2` covered branches; function coverage was
unchanged. The aggregate is now 56,175/62,331 regions (90.124%),
34,962/38,286 lines (91.318%), 5,620/6,736 branches (83.432%), and
2,725/3,161 functions (86.207%). MCP source review reports all five target
regions green. Memory ranged from 42% to 66% free during the run and ended
at 63% free. No runtime code, outputs, hashes, thresholds, filters, coverage
counts, or denominators were changed.
