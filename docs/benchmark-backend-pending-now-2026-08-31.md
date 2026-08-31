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

## Verified at source commits `ba1efa700`–`96d89056a`

- [x] Full all-backend parity at source revision `96d89056a`: CPU, SIMD, GPU,
  Node WASM, and browser WASM each pass 10,952/10,952; GPU smoke passes 1/1.
  Receipt: `build/migration-parity/all-backends-test-result.json` (SHA-256
  `b6f97d5b97167aca9db64c71e8a788e63de26783f79ee92ea74a0ab194289fa8`).
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

The named maintained rows below are now value-exact native GPU paths. Keep the
remaining broader nonconstant/mixed-F boundary explicit: it is still host
semantic control and needs a generally exact f64 device accumulator before it
can be claimed as native coverage.

- [x] `pipeline-op.fit.matrix-32x24`: fractional crop with default bicubic
  sampling now uses two exact GPU resize dispatches (6/6 actual GPU).
- [x] `pipeline-chain.resize-typed.simd-i-resize-transform`: I-mode nearest
  resize followed by transform now uses exact typed GPU geometry (6/6 actual
  GPU, three dispatches).
- [x] `pipeline-chain.resize-cache.f64-identical-geometry`: the maintained
  finite constant-F workload now uses exact bit-pattern GPU lowering. Direct
  Pillow checks are byte-exact for 336/336 dimension/filter/constant cases;
  the two named serialized outputs also match byte-for-byte (SHA-256
  `e6e5c7e553ee0986292c2a6fe7b973b62918b7d69e822752431dfd599d6c24e1` and
  `0491b1637c64fad1506740ab67b7cfd964e97d4c58caad38120eccc1d6bcd7b4`).
  Nonconstant/mixed-F inputs remain explicit host semantic control until a
  generally exact f64 device accumulator exists.
- [x] Re-run the historical GPU matrix through the full public corpus; it is
  value-exact with zero failures. The per-row native-receipt classification is
  recorded in the next item.
- [x] Classify the historical 70 rows from the current source: **70 native
  GPU**, **0 host-control**, and **0 failures**. Receipt:
  `build/migration-parity/gpu70-classification-current-after-f64.json`
  (SHA-256
  `41ac5189e97568a24c8c6aa8a37813805049e493c872518faedf7f6bef876930`), with
  its focused parity sidecar at
  `build/migration-parity/gpu70-classification-current-after-f64-parity.json`.
  This closes the named maintained cohort; broader nonconstant/mixed-F
  behavior remains tracked by the explicit host-control note above.
- [ ] Generalize f64 resize beyond the constant-F lowering: nonconstant and
  mixed-F inputs still use host semantic control. Add an exact device
  accumulator/rounding path, then add direct Pillow byte checks and a native
  receipt before marking this boundary complete.

Acceptance: focused parity, full all-backend parity, terminal actual-GPU
receipts with no fallback, and a maintained regression input for every row.

## P1 — evidence and denominator correctness

- [x] Persist the exact exception, failing step, requested backend, and actual
  backend for every timed failure; partial receipts retain the failure details.
- [x] Add a terminal-completeness bit to receipts; drained prefixes and failed
  workflows are explicitly false, while only a completed terminal workflow is
  true. `make migration-parity-receipt-test` passes 5/5, and the evidence and
  schema validators pass with the new state model.
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

- [ ] Diagnose the stable 11-ID cohort before touching the noisy union. There
  are 44 comparable pairings in the five fresh terminal-valid runs; the budget
  reports are `stable-cohort-budget-2-vs-1.json` (5 violations),
  `3-vs-2.json` (1), `4-vs-3.json` (12), and `5-vs-4.json` (3). Identify and
  fix the recurring operation/backend regressions below, then rerun the same
  IDs.
- [ ] Produce two consecutive budget reports with zero violations on the same
  source and equal receipt cohorts. No zero-violation pair exists yet.

### Recomputed cohort and bounded optimization (2026-08-31)

The original same-source receipts had 14 raw pair intersections, but three were
not comparable: a target row lacked a terminal `actual_backend` receipt or had
a non-completed execution. Applying the equal-ID/no-fallback filter leaves 11
stable rows. The fresh five-run cohort above now has explicit terminal receipts
for all 44 target pairings; its nonzero violation counts remain open performance
evidence, not a passing gate.

Stable IDs:

- Pillow: `pillow/pipeline-chain.reviewed.draw-batch-rgb-shapes`
- CPU: `pipeline-chain.simd-constant.1024x768`,
  `pipeline-chain.terminal-read.analysis-masked-rgb-1024x768`,
  `pipeline-chain.terminal-read.imagestat.cmyk-1024x768`,
  `pipeline-matrix.expanded.autocontrast.32x32`
- SIMD: `pipeline-chain.matrix-002`, `pipeline-chain.matrix-020`,
  `pipeline-chain.metadata-cache.extractband-rgba`,
  `pipeline-chain.reviewed.draw-filter-invert`,
  `pipeline-chain.reviewed.resize-rotate-crop`,
  `pipeline-matrix.expanded.brightness.256x256`

The only repeated violation across the four reports is CPU
`pipeline-chain.reviewed.draw-filter-invert` (three reports); the remaining
violations are one-off cohort variance to classify before changing code.

- [x] SIMD Brightness factor `1.0` now takes an exact native-copy path in
  direct and owned-segment execution, avoiding the identity LUT build and
  frame scan. Focused SIMD parity remains exact (`1/1`); the maintained
  benchmark's SIMD median for `pipeline-matrix.expanded.brightness.256x256`
  moved from `0.0870625 ms` to `0.0442915 ms` (backend phase
  `46666.5 ns` to `4354.5 ns`).

## Publication

- [x] `make fmt`, `RUSTC_WRAPPER= make clippy-core`,
  `make repo-map-check`, fixture/evidence checks, and `git diff --check` pass.
- [x] Commit the PA parity fix as `ba1efa700`.
- [x] Commit the exact GPU Fit/I routing fixes as `8f780c4bb`, `e146ca1b3`,
  and `b465e8f83`.
- [ ] Push `main` after the refreshed checklist and verification commits; record
  the final remote commit here.

## Goal tracking

The Codex goal remains **active**: the named 70-row P0 cohort and receipt P1
gate are now closed, while the broader nonconstant/mixed-F boundary and the
zero-violation P2 performance gate remain unfinished. This checklist is the
updated goal state; it must not be marked complete until every unchecked item
is resolved with reproducible evidence.
