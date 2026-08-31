# Active parity-first checklist — 2026-08-31

This is the short, executable queue for the benchmark/backend goal. The
historical row-by-row audit and classifications stay in
[`benchmark-backend-exhaustive-audit-2026-08-30.md`](benchmark-backend-exhaustive-audit-2026-08-30.md);
this file contains only current evidence and unfinished work.

## Goal

**Active: exact parity first, native-backend coverage second, performance last.**
Every selected public case must produce the same value or error as Pillow.
Capability labels, missing receipts, and host-control fallbacks are diagnostic
states, not completed native-backend work. Do not delete inputs, alter
denominators, relabel a backend, or weaken a gate to make this list shorter.

## Verified at source commits `ba1efa700`–`b465e8f83`

- [x] Full all-backend parity: CPU, SIMD, GPU, Node WASM, and browser WASM
  each pass 10,952/10,952; GPU smoke passes 1/1 after the Fit and typed-I
  changes. Receipt: `build/migration-parity/all-backends-after-gpu-native.json`
  (SHA-256
  `077d448c07c6dada6608f4f1658283d041fa36c36c1a66d2c07d3b99008f7acb`).
- [x] Fresh standard benchmark: 744/744 workloads measured, 0 not-run;
  correctness preflight 202/202. Receipts:
  `build/migration-parity/standard-after-pa-fix.json` (SHA-256
  `9e76dbf8964ff8855e359d19903b548e394d0e8b2bbcda7d0e632649063343f3`) and
  `standard-after-pa-fix-parity.json` (SHA-256
  `0b08b0d60290a71e2da3ea4ea89f666aa3c107162060e7aa7ce962fe7beeca93`).
- [x] PA→RGB is exact with and without an attached palette. The empty-palette
  edge now maps every index to black through the lazy evaluator, while the
  palette-backed route remains native: `pipeline-chain.matrix-058` records
  6/6 actual SIMD and 6/6 actual GPU samples.
- [x] The prior one-case all-backend regression is closed:
  `coverage-batch-convert-nonstandard-pa-rgb-16` now passes on CPU, SIMD, and
  GPU. The fix is `ba1efa700`; it keeps Pillow's `libImaging/Convert.c`
  empty-palette semantics and does not change fixtures or denominators.

## P0 — native GPU coverage still pending

These rows are value-exact today through explicit host semantic control, but
they are not yet native GPU implementations. The task is to implement the
remaining typed kernel, prove exact bytes against Pillow, and remove the
host-control receipt without relabeling it as success.

- [x] `pipeline-op.fit.matrix-32x24`: fractional crop with default bicubic
  sampling now uses two exact GPU resize dispatches (6/6 actual GPU).
- [x] `pipeline-chain.resize-typed.simd-i-resize-transform`: I-mode nearest
  resize followed by transform now uses exact typed GPU geometry (6/6 actual
  GPU, three dispatches).
- [ ] `pipeline-chain.resize-cache.f64-identical-geometry`: f64 geometry path;
  prove exact coordinate and rounding behavior on GPU.
- [x] Re-run the historical GPU matrix through the full public corpus; it is
  value-exact with zero failures. The per-row native-receipt classification is
  retained as the remaining P0 bookkeeping item below.
- [ ] Classify the historical 70 rows as native-GPU, exact host-control, or a
  real failure. The current full-corpus receipt is green; f64 geometry remains
  the only named typed native-GPU blocker.

Acceptance: focused parity, full all-backend parity, terminal actual-GPU
receipts with no fallback, and a maintained regression input for every row.

## P1 — evidence and denominator correctness

- [x] Persist the exact exception, failing step, requested backend, and actual
  backend for every timed failure; partial receipts retain the failure details.
- [ ] Add a terminal-completeness bit to receipts; a drained prefix dispatch is
  not a successful terminal workflow.
- [x] Compare timing reports on equal workload-ID intersections and persist
  the common-ID digest plus excluded members for every subject pair.
- [x] Finish the generator-backed disposition for the 48 historical
  all-subject not-run inputs: the maintained default manifest now leaves 744
  successful workloads after the two named Qt-only rows, while matched-error
  and API coverage remains visible.
- [x] Require every default performance workload to preflight to a successful
  Pillow value; the fresh default run is 744/744 measured and 0 not-run.
  Expected-error cases remain separate parity tests.

## P2 — performance gate after P0/P1

- [ ] Produce two consecutive budget reports with zero violations on the same
  source and equal receipt cohorts. Current reports are not closure evidence:
  112 and 56 one-run violations; the stable intersection is 11 rows.
- [ ] Investigate only the stable 11-row cohort first; retain the noisy union
  as variance evidence. The current rows are listed in the historical audit's
  performance appendix.

## Publication

- [x] `make fmt`, `RUSTC_WRAPPER= make clippy-core`,
  `make repo-map-check`, fixture/evidence checks, and `git diff --check` pass.
- [x] Commit the PA parity fix as `ba1efa700`.
- [x] Commit the exact GPU Fit/I routing fixes as `8f780c4bb`, `e146ca1b3`,
  and `b465e8f83`.
- [x] Push `main`; `origin/main` contains the verified source and checklist
  commits through `2140a547f`.

## Goal tracking

The Codex goal remains **active** because the P0 native-GPU rows and the P1/P2
gates are unfinished. This checklist is the updated goal state; it must not be
marked complete until every unchecked item is resolved with reproducible
evidence.
