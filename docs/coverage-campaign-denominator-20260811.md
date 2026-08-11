# Pillow-RS coverage campaign denominator

This is the immutable denominator record for the 2026-08-11 coverage
campaign. It separates public-contract evidence from whole-project execution
coverage and does not remove source files, operations, inputs, thresholds, or
failed cases.

## Revisions and corpus

- Requested baseline commit: `4fafda8ef320f12a3fdae21dc573fa65e03c9338`.
- Current checkout when this record was created: `3111a048fb7b401abc3ab7614a4a844a9959d3d1`.
- Final measured checkout: `e05cd9ee7c7a22ff7afab68f396ce86e25018f88`.
- CPU baseline snapshot: `fa368338-3d54-4e67-b01f-42c7ef525dcc`.
- SIMD baseline snapshot: `e0d57644-1c2b-4793-9b9d-91c55a3096f3`.
- Public manifest: `pillow-rs/tests/fixtures/manifest.yaml`, schema
  `migration-parity/manifest@2`.
- Public inventory at campaign start: 209 operations, 1,801 manifest
  requirements, and 3,167 active unique workflows. The input check reproduced
  the tracked inputs and crash-quarantine ledger exactly.

The reviewed typed-format batch added one active workflow,
`PIL.Image.Image.getdata.nuanced.l16-png-band-zero`, bringing the current
corpus to 3,168 workflows. The generator and input reproducibility checks
passed after regeneration; no expected output, oracle hash, threshold, or
coverage percentage denominator was edited.

The final focused batch added one further valid workflow,
`PIL.Image.Image.transform.nuanced.perspective-nan-denominator-fill`, bringing
the current corpus to 3,169 workflows. It passes source/target parity and
reaches the non-finite perspective denominator branch in
`pillow-rs/src/compute/pool_cpu/ops/effects.rs`; the focused CPU and SIMD LLVM
snapshots both record line 1016 at 4/4 branches (up from 3/4). No expected
output, oracle hash, threshold, or coverage percentage denominator was edited.

`make migration-parity-fixtures-check` also exposed a pre-existing
manifest-authority diff in this dirty checkout (for example, the tracked
manifest has `has_transparency_data` and extra typed requirements that the
current generator does not emit). The command's input, inventory, and
evidence checks passed, but the manifest diff was not silently accepted or
regenerated during denominator establishment. This campaign preserves the
tracked manifest and treats the authority drift as a reviewed integration
item rather than changing the denominator.

The current checkout contains pre-existing user changes and many linked
worktrees. Those changes were preserved; this record does not treat the
current checkout as a clean replacement for the supplied snapshots.

## Source inventory

| inventory | files | treatment |
| --- | ---: | --- |
| `pillow-rs/src/**/*.rs` | 72 | Core Rust production inventory. 56 files have LLVM records in the supplied snapshots; 16 have no executable record and remain inventory-visible. |
| `pillow-rs-py/src/**/*.rs` | 1 | PyO3 ABI/binding production source. `lib.rs` is measured by the Rust snapshots. |
| `pillow-rs-py/python/**/*.py` | 15 | Thin Python facade and protocol marshaling. Not part of the LLVM Rust numerator; it requires a separate managed Python component for line/branch claims. |
| `pillow-rs-js/src/**/*.rs` | 1 | WASM binding production source. No file is present in either supplied snapshot and no explicit managed JS coverage component exists, so it is not silently scored. |
| `pillow-rs/src/compute/pool_gpu/shaders/**/*.wgsl` | 85 | Generated/runtime GPU shader inventory. GPU execution is excluded from this safe CPU/SIMD campaign. |
| sibling `fontdone` | 52 measured files in the supplied snapshot | Read-only separate parity/coverage audit; never merged into the Pillow-RS numerator. See `docs/fontdone-coverage-gap-audit.md`. |

The 16 core Rust files absent from the supplied LLVM file set are:

```text
pillow-rs/src/compute/backend_op.rs
pillow-rs/src/compute/op_def.rs
pillow-rs/src/compute/pool_cpu/ops/mod.rs
pillow-rs/src/compute/pool_simd/ops/arm.rs
pillow-rs/src/compute/pool_simd/ops/mod.rs
pillow-rs/src/compute/pool_simd/ops/x86.rs
pillow-rs/src/image_utils.rs
pillow-rs/src/ops/mod.rs
pillow-rs/src/par.rs
pillow-rs/src/raster/color/blend.rs
pillow-rs/src/raster/color/invert.rs
pillow-rs/src/raster/color/mod.rs
pillow-rs/src/raster/error.rs
pillow-rs/src/raster/mod.rs
pillow-rs/src/raster/traits/mod.rs
pillow-rs/src/raster/traits/pixel.rs
```

Their absence is recorded, not used to improve a percentage. A future report
may add them only if a managed artifact produces executable records for the
same reviewed scope.

## Denominators

### Supplied baseline snapshot denominators

This is the literal supplied LLVM snapshot denominator, including sibling `fontdone`,
GPU coordinator code, and all other measured files. It is useful for
reproducibility, but it is not a public-contract score.

| lane | lines | branches | functions | regions |
| --- | ---: | ---: | ---: | ---: |
| CPU | 39,111 / 68,478 | 6,513 / 14,052 | 3,023 / 5,314 | 60,400 / 106,451 |
| SIMD | 40,667 / 68,478 | 7,196 / 14,052 | 3,042 / 5,314 | 63,241 / 106,451 |

### Supplied baseline active Pillow Rust scope

This is the union of the measured `pillow-rs` core and `pillow-rs-py/src`
files. It excludes sibling `fontdone`, but retains the measured GPU
coordinator source in the denominator even though GPU execution is not part of
this campaign. It is therefore the strict active-project denominator, not the
GPU-excluded safe implementation denominator below.

| lane | lines | branches | functions | regions |
| --- | ---: | ---: | ---: | ---: |
| CPU | 23,300 / 31,695 (73.51%) | 3,529 / 5,306 (66.51%) | 1,969 / 2,730 (72.12%) | 38,004 / 53,289 (71.32%) |
| SIMD | 24,856 / 31,695 (78.42%) | 4,212 / 5,306 (79.38%) | 1,988 / 2,730 (72.82%) | 40,845 / 53,289 (76.65%) |

### Post-batch managed evidence

These are the final safe managed snapshots for the checkout at commit
`e05cd9ee7c7a22ff7afab68f396ce86e25018f88`. The source/input changes are
present in that commit; the pre-existing user changes remain preserved.
The final whole-LLVM denominator is not a stable Pillow denominator: between
the prior full snapshot and this run, pre-existing dirty sibling `fontdone`
changes altered `fontdone/src/ffi/handles.rs` (-3 lines, -2 branches,
-3 regions), `fontdone/src/font.rs` (+18 lines, +1 function, +13 regions),
and `fontdone/src/render.rs` (+2 lines). The Pillow active and public
denominators did not change; the whole rows below remain a reproducibility
reference that includes the separately audited sibling. The reviewed
`checked_dims.rs` regression guard and `image.rs` I;16 contract fix add 9
counted lines, 2 branch sites, and 16 regions relative to the earlier
Pillow-scoped source inventory; no source file was removed from the measured
scope.
The `active safe` rows exclude only `pillow-rs/src/compute/pool_gpu/mod.rs`;
the `active project` rows retain that source file but do not claim GPU
execution. The `public Rust union` is the de-duplicated 28-path manifest
component union.

| scope | lane | lines | branches | functions | regions |
| --- | --- | ---: | ---: | ---: | ---: |
| whole LLVM snapshot | CPU | 39,434/68,561 (57.52%) | 6,611/13,976 (47.30%) | 3,047/5,333 (57.13%) | 61,017/106,554 (57.26%) |
| whole LLVM snapshot | SIMD | 41,067/68,561 (59.90%) | 7,313/13,976 (52.33%) | 3,066/5,333 (57.49%) | 63,987/106,554 (60.05%) |
| active project (core + PyO3) | CPU | 23,615/31,932 (73.95%) | 3,625/5,358 (67.66%) | 1,993/2,753 (72.39%) | 38,605/53,684 (71.91%) |
| active project (core + PyO3) | SIMD | 25,248/31,932 (79.07%) | 4,327/5,358 (80.76%) | 2,012/2,753 (73.08%) | 41,575/53,684 (77.44%) |
| active safe (GPU coordinator excluded) | CPU | 23,609/30,669 (76.98%) | 3,625/5,228 (69.34%) | 1,991/2,673 (74.49%) | 38,599/51,966 (74.28%) |
| active safe (GPU coordinator excluded) | SIMD | 25,242/30,669 (82.30%) | 4,327/5,228 (82.77%) | 2,010/2,673 (75.20%) | 41,569/51,966 (79.99%) |
| public Rust component union | CPU | 14,624/15,491 (94.40%) | 2,685/2,876 (93.36%) | 1,119/1,315 (85.10%) | 23,657/25,215 (93.82%) |
| public Rust component union | SIMD | 14,044/15,491 (90.66%) | 2,651/2,876 (92.18%) | 1,096/1,315 (83.35%) | 22,682/25,215 (89.95%) |

The SIMD implementation-only group (`pool_simd/mod.rs` plus its `ops`
adapters.rs and scalar.rs) is 3,888/3,982 lines, 893/1,014 branches,
195/204 functions, and 7,303/7,508 regions. This is reported separately
because the public component paths intentionally name CPU implementations.

Prior managed run IDs and snapshots (before the final source commit):

- CPU run `99c8ce2b-e258-4311-ba82-d3fac07a890e`, snapshot
  `85909121-065c-498c-8c83-1445a8c6bbe0`.
- SIMD run `7ca8937f-4e51-4294-912b-9fccec4b8615`, snapshot
  `2dffe5d2-3685-4efe-b3a7-56298fdda6dc`.

Final managed run IDs and snapshots:
- Focused non-finite transform CPU run `f63d1621-34f4-418a-8355-6ec9063e9796`,
  LLVM snapshot `a1ad8946-9544-4802-8093-d2cf507a52e9`.
- Focused non-finite transform SIMD run `8df0919a-aa92-4154-8a3f-d432dccf0514`,
  LLVM snapshot `c54265b2-9b5a-496d-b4d3-e42689df1c04`.
- Final CPU run `fc0f294d-4460-4721-ac8b-0ff15d0a7053`, snapshot
  `1df97c40-11ce-46c0-b425-e6a60ebde1a2`.
- Final SIMD run `0cefd6a7-dabc-4038-b888-acc551c600d4`, snapshot
  `095aef5e-bb1c-4e07-b4c0-b1b8bc76bffa`.
- Focused typed-case and non-finite-transform parity passed; final full CPU
  parity executed 3,169 cases with 3,168 passed and one retained
  variable-font mismatch.

### Public-contract Rust component union

The public-contract source denominator is the de-duplicated union of the 28
Rust paths named by the manifest's `coverage_components`. Shared paths such as
`color.rs` are counted once. The manifest's 15 Python facade paths remain
contract evidence but are not falsely represented as LLVM Rust lines.

| lane | lines | branches | functions | regions |
| --- | ---: | ---: | ---: | ---: |
| CPU | 14,624 / 15,491 (94.40%) | 2,685 / 2,876 (93.36%) | 1,119 / 1,315 (85.10%) | 23,657 / 25,215 (93.82%) |
| SIMD | 14,044 / 15,491 (90.66%) | 2,651 / 2,876 (92.18%) | 1,096 / 1,315 (83.35%) | 22,682 / 25,215 (89.95%) |

The contract input denominator is separately 209 operations / 1,801
requirements / 3,169 workflows. A workflow is contract evidence only when
the source and target both pass; an operation with a signature-only or
unmeasured result is not counted as covered.

## Classification rules

- Generated manifest, input, report, and documentation files are artifacts of
  the maintained generator and are not executable-source denominator entries.
- `default_aileron.rs` and embedded binary/font assets are generated or
  vendored data used by production code; they remain visible when LLVM records
  executable code for them.
- PyO3 and WASM export/record-copy/error-mapping code is ABI boilerplate or
  binding protocol unless a public input reaches its behavior. It is never
  removed from a measured denominator; JS is currently unmeasured rather than
  treated as covered.
- Defensive, malformed-input, impossible-state, backend-routing, and
  unreachable branches are classified from bounded source context. They are
  not targets for fabricated public inputs.
- GPU shaders and GPU-only execution are outside this safe CPU/SIMD campaign.
- Pending 16-bit TIFF cases and crash-inducing cases remain quarantined.
- `fontdone` format/rendering gaps are recorded in a separate document and do
  not alter any Pillow-RS denominator.

Any later denominator change must add a reviewed scope entry and preserve this
baseline table. It must not be achieved by deleting files, operations, cases,
thresholds, or expected outputs.
