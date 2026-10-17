# GetColors I phase attribution — 2026-10-17

This is a temporary-instrumentation diagnostic for the serial-CPU near-miss in GetColors. It is not an optimization trial and must not be admitted as a performance comparison: the target binding was instrumented with `Instant` reads and a per-call counter update. The timed public call still used the original workload and complete 4,097-row result materialization.

## Frozen scope and proof

- Workload: `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`
- Exact parity case: `PIL.Image.Image.getcolors.nuanced.i-high-cardinality-signed-run-fallback-128x64`
- Boundary: `observed_steps`, `call-fallback-4097`; warm release; 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Profile: `python-cpu` only, default Cargo features; Pillow 12.2.0 oracle.
- Benchmark run: `migration-benchmark-193c9dbb31e242adb66fb3b50bc8f18d`.
- Exact parity: `migration-parity-benchmark-gate-d60cc45fad774360881f1fa6bee40d65`, 1/1 pass.
- Execution receipt: requested and actual CPU, one `GetColors/cpu` operation, terminal complete, no fallback.
- The instrumentation worker drained phase counters after each `run_case` returned. The phase sidecar has 2 untimed preflight calls plus 105 timed calls; its first 5 timed rows align with warmups and the remaining 100 align with measurement labels in `raw-samples.json`.

## Attribution

| Measured phase | Median | P95 | Samples |
| --- | ---: | ---: | ---: |
| Core GetColors call | 25.937 µs | 40.041 µs | 100 |
| Detach plus error mapping window (inclusive) | 25.979 µs | 41.667 µs | 100 |
| Python output conversion for 4,097 rows | 86.666 µs | 149.542 µs | 100 |

The Python tuple/list construction phase takes about 3.34× the measured core phase. The remainder of the instrumented public-call median is outside these three spans. This quantifies where time is spent but does not prove that list construction can be made cheaper while preserving Python types and allocation-error behavior. Prior `PyList::new` and typed-tuple candidates were already slower; do not repeat them without a distinct safe mechanism.

The instrumented report shows Pillow at 0.178292 ms and target CPU at 0.194896 ms, but those values are diagnostic only. Do not use them to claim a regression or optimization: the instrumentation changes the target path, and this was not an A-B-B-A timing trial. None of these rows were added to the performance ledger or operation matrix.

## Identity and restoration

Target source revision was dirty `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, on macOS 15.7.7 ARM64 / CPython 3.12.13; the oracle was Pillow 12.2.0. The probe source hashes were `pillow-rs-py/src/lib.rs` `a336e690b48ad937b7d0d67543c998111757b794b5a962a4f37598707e48a33c` and `scripts/run_migration_parity.py` `1076643fe08c5cd5fc75379cb64d8ed7d1053bbcc7d5418fba4d2903bf701b19`; loaded probe extension SHA-256 was `8aa91fe5030b4d76d068dd2207f21ead23f671f317775f255695fd1b1619081e`.

Both source files were restored byte-for-byte to their pre-probe hashes (`lib.rs` `068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24`, parity runner `e4f692735be23d059d28c6fde2122072210fc1499fc83339fb243083c7c4e564`). A timestamp-preserving restore initially caused Cargo to reuse the instrumented extension; after touching the restored binding source, `make build-parity` rebuilt the normal extension. The loaded extension is now SHA-256 `e0e3e5373c0bb2c2779f2fc6b38631ce35b951528dac852808c6d3cf0cc3f847`, and `_take_getcolors_phase_probe` is absent.

## Artifacts

- `summary.json`: compact identity, receipt, and phase statistics.
- `parity.json`: focused byte-exact ordinary-Pillow comparison.
- `benchmark-instrumented.json` and `raw-samples.json`: full diagnostic report and timing samples; not performance-credit evidence.
- `phase-rows.jsonl`: ordered per-call measurements.
- `phase-probe.patch`: temporary Rust-only probe patch; the parity-worker drain hook was also temporary and has been restored.

No coverage or broad campaign was run. The GetColors issue remains open; this probe adds no optimization credit and does not complete an operation.
