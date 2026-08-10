# CPU color-conversion coverage audit

Date: 2026-08-11
Worktree: `/Users/lazytrot/work/pillow-rs/.worktrees/coverage-audit-color-20260811`
Branch: `codex/coverage-audit-color-20260811`
Baseline: `bf3b284844171e81fff58f8e00b41428b908610f` (`bf3b28484`)

## Scope and evidence

This audit is limited to `pillow-rs/src/compute/pool_cpu/ops/color.rs` and
the callers that determine whether its operations are reached. The managed
CPU snapshot supplied for the audit is:

| Item | Value |
| --- | --- |
| CPU snapshot | `7b33de24-fda1-4a8f-999f-7ca9a82a54d3` |
| Branch/commit | `main` / `bf3b284844171e81fff58f8e00b41428b908610f` |
| Suite | `migration-parity-rust` |
| Corpus | 3,101 parity cases |
| Project lines | 39,320 / 68,650 (57.2760%) |
| Project branches | 6,556 / 14,080 (46.5625%) |
| Project functions | 3,040 / 5,336 (56.9715%) |
| Project regions | 60,777 / 106,732 (56.9436%) |

The supplied SIMD context is `b4faf868-62fc-4edf-8c32-5ba0db2c8f95`; no
SIMD run or GPU/TIFF/crash lane was started for this CPU audit.

The authoritative CPU file query for `color.rs` reports:

| Metric | Covered / total | Rate |
| --- | ---: | ---: |
| Lines | 122 / 229 | 53.2751% |
| Branches | 5 / 26 | 19.2308% |
| Functions | 5 / 7 | 71.4286% |
| Regions | 247 / 531 | 46.5160% |

The query reports 99 relevant gap lines, 94 uncovered lines, 13 partial
branch lines, and 21 missed branch edges. Coverage function grouping reports
5/7 functions; the public entry at line 233 is confirmed uncalled, while the
tool's function-gap field does not identify the other two function records by
name.

Exact missing ranges returned by the file query are:

```text
35 (partial branch, 1 missed)
59 (partial branch, 1 missed)
60 (uncovered + partial branch, 2 missed)
61-68 (uncovered)
70-71 (uncovered)
80 (uncovered + partial branch, 2 missed)
81 (uncovered)
83 (uncovered)
85-87 (uncovered)
90 (uncovered)
91 (uncovered + partial branch, 2 missed)
98-110 (uncovered)
111 (uncovered + partial branch, 2 missed)
112-121 (uncovered)
124 (uncovered)
129 (uncovered)
133-140 (uncovered)
142 (uncovered)
189 (partial branch, 1 missed)
190-191 (uncovered)
192 (uncovered + partial branch, 2 missed)
194 (uncovered)
233 (uncovered function entry)
238-240 (uncovered)
241 (uncovered + partial branch, 2 missed)
242-245 (uncovered)
246 (uncovered + partial branch, 2 missed)
247 (uncovered)
250 (uncovered)
252-255 (uncovered)
257-258 (uncovered)
272 (partial branch, 1 missed)
274 (uncovered)
277 (partial branch, 1 missed)
285 (uncovered)
287 (uncovered + partial branch, 2 missed)
288-292 (uncovered)
294-295 (uncovered)
297-303 (uncovered)
305 (uncovered)
```

## Dispatch map and classification

### `op_convert` (lines 17-229)

The CPU registry dispatches `PipelineOp::Convert` to `op_convert` in
`pillow-rs/src/compute/registry.rs:1141-1159`. The public
`Image::convert_with_input`/`Image::convert` path in
`pillow-rs/src/ops/convert.rs` intentionally handles several modes before
queuing that operation:

- `convert("1")` is eagerly thresholded/dithered at lines 460-568.
- `convert("P")` is eagerly quantized and given a palette at lines 571-624.
- nonstandard sources (`P`, `CMYK`, `HSV`, `YCbCr`, `I`, `F`) are materialized
  and converted before a standard target at lines 306-441.
- only the remaining standard pipeline conversion is queued at lines 627-646.

Consequently, the missing `op_convert` ranges classify as follows:

| Lines | Code path | Classification | Evidence / proposed shape |
| --- | --- | --- | --- |
| 35, 59-71 | RGBA-to-LA alpha guard and P-palette RGB expansion | Direct-core/legacy representation guard in the CPU registry path; public P and nonstandard-source conversion is handled earlier | `Image.convert("RGB")` from a P image exercises the public eager path, not these lines. The SIMD adapter also explicitly falls back to this CPU function for P/Mode1, which is a backend-specific route rather than the managed CPU public aggregate. |
| 80-129 | `ColorMode::Mode1`, including CMYK special handling and no-dither/Floyd branches | Public behavior exists, but the public Python-style conversion is eager and bypasses this registry function | Existing public corpus contains `1` conversions. Adding another normal `convert("1")` case would not cover these lines; a direct `op_convert`/pipeline case would be a direct-core or backend-specific probe. |
| 133-142 | `ColorMode::P` median-cut path | Legacy/direct-core dispatch | Public `convert("P")` uses the eager WEB-palette path in `ops/convert.rs:571-624`; `Image.quantize` is also implemented directly in `ops/quantize.rs`, so this registry operation has no public constructor found by source search. |
| 189-194 | Explicit source mode `"1"` in luma-to-CMYK | Public source handling is pre-expanded before this function | `ops/convert.rs:273-284` normalizes source mode `1` before the pipeline path. A direct core call could hit it, but it is not a missing public parity shape. |

The `I` and `F` arms (lines 144-175), CMYK regular luma branch (196-202),
HSV/YCbCr arms (218-227), and the `op_convert` entry itself are covered in
the aggregate CPU snapshot. `op_convert` entry coverage is 31 hits.

### `op_quantize` (lines 231-258)

The entire function is absent from the CPU aggregate; line 233 has zero hits.
The registry handler exists at `registry.rs:1162-1175`, but the public
`Image.quantize` implementation runs the quantizer directly in
`pillow-rs/src/ops/quantize.rs` (the public methods are around lines 2051 and
2146), and public `convert("P")` uses a separate eager path. No public caller
constructing `PipelineOp::Quantize` was found. This is a legacy/direct-core
dispatch gap, not a justified public parity case for this audit.

### `op_remap_palette` (lines 260-306)

The registry dispatch is at `registry.rs:1178-1195`; the public entry point is
`Image::remap_palette_with_source` in `pillow-rs/src/image.rs:4823-4908`.
That entry point accepts only `L` and `P` modes (`image.rs:4828-4831`).

| Lines | Classification | Evidence / proposed case |
| --- | --- | --- |
| 272, 274, 277, 285 | P-index path, partially covered | Existing P remap cases hit this path. |
| 287-295 | Reachable public gap | The public method accepts L images, but the active 3,101-case corpus contains P remap cases and no L remap case. A minimal future case should create an L image with samples `[0, 1, 2, 3]`, call `remap_palette([2, 0, 3, 1])`, and assert inverse-mapped samples `[1, 3, 0, 2]` plus the returned palette metadata. This audit does not edit the shared generator, manifest, or expected data. |
| 297-305 | Defensive/direct-core only | Public `remap_palette_with_source` rejects every non-P/non-L mode before queuing the operation. Reaching this RGB fallback requires bypassing the public validation. |

`op_remap_palette` entry coverage is 5 hits. The L branch is the only clear
publicly reachable color.rs gap identified by this audit.

### `op_extract_band` (lines 308-340)

The entry has 13 hits and the complete function body is covered in the CPU
snapshot. No additional case is proposed.

## Relevant raster/color callers

These were inspected to avoid treating generic implementation coverage as a
missing PIL endpoint. They were not edited.

### Generic `FromColor` conversions

`pillow-rs/src/raster/color/from_color.rs` is 56/102 lines, 8/16 functions,
and 97/190 regions in the CPU snapshot. Uncovered ranges are:

```text
17-21, 37-41, 48-53, 62-67, 74-79, 86-91, 98-102, 169-175
```

These are generic typed combinations such as `Rgb<S> -> Luma<T>` and
`Rgb<S> -> Rgb<T>`. They are direct-core trait instantiations; the public PIL
paths generally use specialized conversion helpers and do not expose every
source/target primitive pair. They are not evidence for adding a CPU color
parity fixture in this bucket.

`pillow-rs/src/raster/color/from_primitive.rs` is 3/43 lines, 1/9 functions,
and 3/80 regions. Its uncovered ranges are:

```text
19-22, 24-25, 29-32, 34-35, 40-44, 48-50, 55-57, 61-63,
67-69, 71, 73-74, 85-92
```

This is generic primitive conversion infrastructure, including inlined or
macro-generated instantiations, rather than a separate public PIL operation.

### Dynamic typed/16-bit conversion paths

`pillow-rs/src/raster/dynamic.rs` contains typed conversion branches for
`Luma16` and other 16/32-bit variants. The queried public-conversion region
shows zero hits for the `Luma16` special branches in `to_rgba8` (291-296),
`to_luma8` (307-311), and `to_luma_alpha8` (322-326), while their generic
fallbacks are used. These are direct typed paths and also overlap the known
16-bit TIFF/image-slash-star pending lane. The audit deliberately did not run
TIFF input or change that lane.

`pillow-rs/src/color.rs` is otherwise highly covered in the CPU snapshot:
674/687 lines, 165/166 branches, 58/62 functions, and 1,211/1,235 regions.
Its remaining type arms (including 16-bit types) are codec/direct-core or
pending TIFF coverage, not changes justified inside `pool_cpu/ops/color.rs`.

## Focused managed run

To verify the public conversion dispatch without running unrelated lanes, the
maintained operation target was run in this worktree:

```text
VIRTUAL_ENV=/Users/lazytrot/work/pillow-rs/.venv \
PYTHON=/Users/lazytrot/work/pillow-rs/.venv/bin/python \
make migration-parity-operation-coverage \
  MIGRATION_COVERAGE_OPERATION=PIL.Image.Image.convert
```

Run ID: `da41ba43-d66c-4b97-8094-48efc90d7bea`
Result: passed, exit 0, 57.097 seconds
Rust operation snapshot: `705047bd-719a-42ac-867d-f9fd5b7cea45`
Python operation snapshot: `79b1f86b-9ea3-4199-821b-05d44ca06205`

The scoped Rust snapshot reports 3,408/68,256 lines, 335/14,026 branches,
315/5,291 functions, and 5,716/106,185 regions. Within `color.rs` it reports
81/229 lines, 3/26 branches, 3/7 functions, and 161/531 regions. This is
consistent with the dispatch analysis: the operation exercises `op_convert`
but does not reach the P/Mode1 registry arms, `op_quantize`, or the L remap
branch. It is an operation-scoped result and must not replace the aggregate
CPU snapshot above.

## Before/after and decision

No runtime source, generator, manifest, expected data, or lane configuration
was changed. Therefore the aggregate color metrics are intentionally unchanged
after the audit:

| Snapshot state | Lines | Branches | Functions | Regions |
| --- | ---: | ---: | ---: | ---: |
| CPU baseline `7b33de24-fda1-4a8f-999f-7ca9a82a54d3` | 122/229 | 5/26 | 5/7 | 247/531 |
| Post-audit aggregate | 122/229 | 5/26 | 5/7 | 247/531 |

The focused run is the verification artifact, not an aggregate “after”
improvement. The evidence supports one follow-up public case shape: L-mode
`remap_palette`. It does not support a source change in `color.rs`; adding
direct-core tests for `op_quantize`, P/Mode1, or generic raster conversions
would measure internal dispatch rather than improve the maintained public CPU
parity bucket. 16-bit TIFF, SIMD, GPU, and crash-inducing lanes remain
explicitly excluded.

## Verification and scope audit

Commands run:

```text
make help
make migration-parity-inputs
make migration-parity-operation-coverage MIGRATION_COVERAGE_OPERATION=PIL.Image.Image.convert
make migration-parity-inputs-check
```

The input-generation command confirmed 24 benchmark suites, 208 benchmark
workloads, 24 coverage plans, and 3,101 parity cases without modifying the
shared input generator, manifest, or expected data. The operation target
completed with no infrastructure error. The maintained input-check target was
also attempted, but its legacy-accounting test failed before any color audit
assertion: the checkout has no
`deprecated/migration-parity-v0/fixtures/python/suite{0,1}/input/jsons`
directories, so the test observed 0 legacy rows instead of its baseline
expectation of 1,592. This is a pre-existing checkout/deprecated-fixture
condition, not a change made by this audit; the worktree remained unchanged
apart from this document. No `coverage show` command was used.

The only intended committed file from this audit is this document.
