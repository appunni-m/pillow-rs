# Coverage campaign results — 2026-08-11

This is the campaign handoff for the fixed, input-driven Pillow-RS scope. It
records what was measured and what was deliberately left out. It does not
change the manifest denominator or claim 99% where the managed evidence does
not support it.

## Outcome

The campaign did not reach 99%. The exact final managed results are recorded in
[`coverage-campaign-denominator-20260811.md`](coverage-campaign-denominator-20260811.md).
The current public Rust component union is:

| lane | lines | branches | functions | regions |
| --- | ---: | ---: | ---: | ---: |
| CPU | 14,624/15,491 (94.40%) | 2,685/2,876 (93.36%) | 1,119/1,315 (85.10%) | 23,657/25,215 (93.82%) |
| SIMD | 14,044/15,491 (90.66%) | 2,651/2,876 (92.18%) | 1,096/1,315 (83.35%) | 22,682/25,215 (89.95%) |

These are the highest percentages established by the fixed public manifest and
the final safe corpus. The active-project and GPU-excluded safe denominators
are also reported separately in the denominator record; no files, operations,
cases, thresholds, or expected outputs were removed to obtain them.

## Retained batch

The typed-format worker added one valid declarative case:
`PIL.Image.Image.getdata.nuanced.l16-png-band-zero`. The maintained generator
was updated and `make migration-parity-inputs` regenerated the tracked input
corpus. The corpus grew from 3,167 to 3,168 workflows. Focused parity passed
after rebuilding the target, and the full lane executed 3,168 cases with 3,167
passes and one existing variable-font mismatch.

The targeted file evidence was positive: `pillow-rs/src/image.rs` lines
covering explicit `I;16` band rejection were reached in both CPU and SIMD
measurements. The aggregate denominators changed with the current source
artifact, so the exact final numerator/denominator tables—not a synthetic
percentage delta—are authoritative.

## Worker reports

All workers used isolated worktrees. No worker pushed or edited `main`.

| bucket | worktree / branch | result | commit / evidence |
| --- | --- | --- | --- |
| CPU core operations | `.worktrees/cpu-public-image-core-coverage-v1-20260811` / `codex/cpu-public-image-core-coverage-v1-20260811` | Three valid `ImageChops.invert` cases passed, but two managed coverage attempts produced zero line/region/function delta; stopped as required. | no commit; parity `f58d76a3-2def-450d-a284-1867faff8270`; final CPU snapshot `905783f4-6dad-4ebe-be4c-ecbbd750be0d` |
| SIMD adapters | `.worktrees/simd-only-coverage-v1-20260811` / `codex/simd-only-coverage-v1-20260811` | Valid brightness/color/contrast/sharpness candidate passed parity and produced zero adapter/scalar delta; stopped as required. | `d5ed64bcbcfde0ac799cdd3d55ca7085991d548e`; parity `4f821ad5-3d1a-4b3a-a0a8-1224792ebd04`; snapshot `6b60d536-50d0-42f3-848c-93fd88f69875` |
| typed formats | `.worktrees/audit-binding-typed-formats-v1-20260811` / `codex/audit-binding-typed-formats-v1-20260811` | One valid `I;16` PNG `getdata(band=0)` case; CPU and SIMD parity passed and the targeted `image.rs` lines were reached. | worker commit `ccb0303cbb45b4d3a10ebe31decb5bdf3d72502a`; worker snapshots `de94b233-3e10-4d36-93bb-413b56d5c0a0`, `5a514a63-2f40-4c96-bb28-f44e7e684b3a` |
| fontdone formats/rendering | `.worktrees/coverage-audit-fontdone-20260811` / `codex/coverage-audit-fontdone-20260811` | Read-only audit of PFR, SVG, SBIX/SBIT, autohint, render/scaler, CFF, CMap/SFNT, and variations. No sibling source or Pillow denominator changes. | report commit `1a4d0d97b47602b0d18a14e80c504b7c35f5f829`; its checkout lineage did not match the required base, so it was not integrated |
| PyO3 / JS / WASM | `.worktrees/thumbnail-aspect-round-ceil-audit-20260811` / `codex/thumbnail-aspect-round-ceil-audit-20260811` | PyO3 is observed by LLVM but has no declared managed component; JS/WASM is absent from the coverage snapshots and has no target profile. No input was invented. | report commit `18dd6ab45e974717a38de38ef4a5f17fd081cc1f` |
| low-level defensive Rust | `.worktrees/lowlevel-defensive-coverage-v1-20260811` / `codex/lowlevel-defensive-coverage-v1-20260811` | Added a direct Rust `CheckedDims` byte-overflow regression test; no fake public input or parity denominator change. | worker commit `547ab03ef9427f0a4c25b72aeb099066e4f3d990`; `make test-core` passed 72/72 in the worker |

## Remaining blockers and classifications

- Whole LLVM snapshots include sibling `fontdone` and are not a Pillow public
  contract score. Fontdone remains read-only and separate in
  [`fontdone-coverage-gap-audit.md`](fontdone-coverage-gap-audit.md).
- CPU-only operations appear as zero files in the SIMD snapshot because the
  backend dispatch selects SIMD closures. SIMD adapter/scalar coverage must be
  reported from its own implementation group, not fixed by duplicating CPU
  inputs.
- Remaining Pillow gaps include defensive validators, generic raster
  conversion kernels, backend routing, registry mismatch guards, GPU-only
  coordinator paths, and some reachable typed/operation branches. The CPU
  worker’s invert batch and SIMD worker’s adapter batch supplied the required
  two-attempt no-gain evidence for their assigned candidates.
- `pillow-rs-py/src/lib.rs` is an LLVM observation only (3,127/3,656 lines,
  286/352 branches, 401/503 functions, 4,684/5,918 regions). There is no
  managed PyO3 component. `pillow-rs-js/src/lib.rs` is not measured.
- The full parity lane retains one known variable-font numeric mismatch:
  `PIL.ImageFont.FreeTypeFont.set_variation_by_axes.nuanced.variable-font-positive-axis-overflow`.
- GPU tests, crash-quarantine inputs, and the pending 16-bit TIFF cases were
  not executed. The manifest-authority drift exposed by the dirty checkout was
  preserved for review rather than regenerated into a new denominator.

## Verification record

- `make migration-parity-inputs` — regenerated 3,168 workflows.
- `make migration-parity-inputs-check` — passed 15 tests.
- `make migration-parity-evidence-check` — passed 11 tests; 209 operations,
  zero stale/incompatible evidence.
- `make migration-parity-case CASE_ID=PIL.Image.Image.getdata.nuanced.l16-png-band-zero` — passed after `make build`.
- `make migration-parity-test` — 3,167 passed, 1 known variable-font failure.
- Managed safe CPU coverage run `99c8ce2b-e258-4311-ba82-d3fac07a890e`, snapshot
  `85909121-065c-498c-8c83-1445a8c6bbe0` — passed and ingested.
- Managed safe SIMD coverage run `7ca8937f-4e51-4294-912b-9fccec4b8615`,
  snapshot `2dffe5d2-3685-4efe-b3a7-56298fdda6dc` — passed and ingested.
- `make test-core` — passed 77 tests with 0 failures, including the new
  byte-overflow guard. The maintained target also invokes five existing
  `pool_gpu` unit tests; no GPU parity or GPU coverage lane was run, and no
  further GPU command is part of this campaign.
- `make build` — passed after the sandbox-required cache escalation.
- `make fmt` remains blocked by unrelated pre-existing import ordering in
  `pillow-rs/src/ops/transform.rs`. The final `make clippy` run reached the
  workspace but failed on existing `unwrap`/`expect` diagnostics in the
  `backend_support_matrix` example and `font_native_public_api` test; these
  were not weakened or bypassed. Workers also encountered the separately
  recorded pinned AVIF dependency blocker in their isolated checkouts.
