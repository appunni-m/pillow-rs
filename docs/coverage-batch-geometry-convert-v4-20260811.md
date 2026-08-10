# CPU geometry/convert coverage batch audit — 2026-08-11

This bounded audit covers only `pillow-rs/src/compute/pool_cpu/ops/geometry.rs`
and `pillow-rs/src/ops/convert.rs`. It does not change runtime Rust, bindings,
generated inputs, expected outputs, thresholds, or manifest counts. No source
fix was proven necessary, so this is an audit-only change.

## Provenance and before/after counts

| item | value |
| --- | --- |
| worktree | `/Users/lazytrot/work/pillow-rs/.worktrees/coverage-batch-geometry-convert-v4-20260811` |
| branch / HEAD | `codex/coverage-batch-geometry-convert-v4-20260811` / `41ea033e21eeb513b7e391f9373e41ec035a1cd4` |
| corpus | 3,100 parity cases |
| suite | `migration-parity-rust` |
| managed snapshot | `2ee6544f-bfa2-4b7b-8ed4-bf0774c96fb6` |
| snapshot metadata | branch `main`, commit `474dd6d91cec7574511c522d81352adfe02fea32` |
| active CPU lines | **before 23,496/26,647; after 23,496/26,647** |
| active CPU branches | **before 3,570/4,198; after 3,570/4,198** |
| active CPU functions | **before 1,982/2,468; after 1,982/2,468** |
| active CPU regions | **before 38,368/44,382; after 38,368/44,382** |

The snapshot is recorded against `main`, not the worker commit. The two owned
source files are byte-identical between `41ea033e2` and the snapshot commit, so
the queried source line numbers apply to this worktree. No post-change managed
coverage run was started; therefore the before/after counts are intentionally
unchanged.

Snapshot file metrics were:

| file | lines | branches | functions | regions |
| --- | ---: | ---: | ---: | ---: |
| `compute/pool_cpu/ops/geometry.rs` | 860/879 | 176/198 | 39/46 | 1,683/1,728 |
| `ops/convert.rs` | 458/468 | 100/110 | 24/29 | 813/850 |

## Exact managed gaps

`P<n>` means a partial-branch line with `n` missed branch arms. `U` means an
uncovered executable line.

```text
geometry.rs:
  23[P2], 29[P1], 49[P1], 118[U], 139[P3], 257[P3], 494[P1],
  815[P4], 819-820[U], 822-823[U], 879[P2], 961[U], 1019[P1],
  1035[P2], 1036[U], 1089[P1], 1247[P1], 1338[P1]

convert.rs:
  276[P1], 296[P1], 300[P3], 301[U], 325[P2], 386[P1], 443[P1],
  638[P1], 644[U], 732[U]
```

These are the exact ranges returned by `coverage_query(view="file")`, followed
by selected-line queries for the same snapshot. The file query reported
geometry `7` uncovered lines plus `13` partial-branch lines, and convert `3`
uncovered lines plus `7` partial-branch lines.

## Reachability classification

### Valid public routes with residual data-dependent branches

- `geometry.rs:23, 29, 49`: filter-kernel boundary predicates reached by the
  normal `Image.resize` route through `execute_resize` and `pil_resize`.
  Existing valid rows include `box-filter`, `box-integer-ratio-boundary`,
  `lanczos-kernel-boundaries`, `hamming-kernel-boundaries`, and named bilinear
  or box filters. The remaining coefficient-boundary arms do not indicate a
  behavior defect.
- `geometry.rs:139, 257`: F/I zero-work guards. Existing valid zero-width and
  zero-height `frombytes` source rows reach the source-dimension side of the
  guard. `ops/resize.rs:85` rejects zero destination dimensions before the
  pipeline, so the remaining destination-zero arms are contract-protected.
- `geometry.rs:879`: the 90/270-degree non-expanding rotation bounds test is a
  valid fill-versus-source decision for non-square images. The line is hit; the
  remaining conjunction arm is a dimension-dependent case, not a source bug.
- `geometry.rs:1019, 1089, 1247, 1338`: thumbnail aspect selection, thumbnail
  pre-reduction, reduce no-op, and partial bottom/right blocks. Existing rows
  cover reducing and non-reducing thumbnails, `[1, 1]` reduce factors, and odd
  or non-square factors. Other valid factor/aspect combinations could change
  branch counters, but no current output mismatch or implementation defect was
  found.
- `convert.rs:276, 296, 325, 386, 443`: mode normalization, same-mode
  non-`P` handling, non-standard dispatch, and PA conversion branches. Existing
  valid rows cover `1` conversions, LA/RGBA same-mode cases, non-standard
  source/target combinations, and palette-alpha paths. The residual arms are
  ordinary mode combinations, not evidence of a broken route.

### Valid input or public route stops before the measured helper

- `convert.rs:300-301`: the existing valid `p-same-mode` row is handled by
  `convert_with_input`'s correct same-mode copy at lines 188-189, before the
  lower-level `convert` method. The uncovered branch is therefore not missing
  from the measured Python route. A direct lower-level/JS call uses a different
  route and remains an unmeasured follow-up; this CPU batch does not change it
  without a scoped regression and corresponding parity evidence.
- `geometry.rs:1035-1036`: `Image.thumbnail` normalizes zero and negative
  requests in `ops/resize.rs:134-140`; an all-zero request returns a public
  no-op. `execute_thumbnail` receives positive dimensions, so its zero-size
  error is an internal defensive guard.
- `convert.rs:638, 644`: `Image::push_op` always constructs an
  `Image::Pipeline` (`image.rs:1716-1850`). The wrong-variant arm is not
  reachable through the valid operation constructors, and line 644 is the
  uncovered closing-brace mapping for that pattern arm.

### Defensive, invariant-protected, or invalid-input arms

- `geometry.rs:118`: `raw_bytes_to_image` is called with the source image's
  channel count, which is restricted to 1–4 by `DynamicImage`; the unsupported
  count is not a public route.
- `geometry.rs:494`: F/I scalar rotation is selected only for the native
  four-byte F/I representation. A valid F/I image reaches the four-channel
  representation; the fallback for a mismatched internal representation is
  retained as a guard.
- `geometry.rs:815, 819-823`: `Image::crop` and `crop_float` normalize and
  validate coordinates before queuing `PipelineOp::Crop`. `execute_crop` only
  receives a positive in-bounds box, making the debug-assert failure and both
  checked-sub underflow errors invariant violations. Existing reversed,
  negative, and zero-size public cases return through the public crop contract
  instead of manufacturing invalid pipeline coordinates.
- `geometry.rs:961`: this is error propagation from arbitrary rotation. Normal
  valid images return a result; forcing allocation/shape failure would require
  invalid or oversized internal state rather than a parity input.
- `convert.rs:732`: the matrix-length error is rejected by
  `convert_with_input` at lines 197-203. The existing `rgb-matrix-wrong-length`
  row proves the public validation route; it does not reach the private matrix
  helper. No malformed matrix was added to drive the uncovered arm.

## Verification record

- Managed queries: `project_context(detailed=false)` reported Coverage MCP
  schema revision 7; explicit `coverage_query` reads covered the snapshot
  summary, both file gap sets, selected line records, and bounded
  `source_context` ranges. `coverage show` was not used.
- Maintained make targets: no parity or coverage target was launched after the
  audit because there is no source fix and the user requested no new broad run.
- Managed run IDs: none for this worker; no post-change snapshot exists.
- Infrastructure failures: none. The only provenance caveat is that the
  supplied snapshot metadata points to `main`/`474dd6d...`; the two owned files
  have no diff from the worker baseline.
- No generated input, expected output, threshold, manifest, binding, GPU/SIMD,
  color/chops, font, TIFF, or crash-lane file was changed.

The remaining work is a future reviewed input/route batch for data-dependent
kernel, aspect, factor, and lower-level P same-mode branches. This audit does
not add denominator-inflating or malformed cases.
