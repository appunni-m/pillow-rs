# Coverage evidence

Coverage answers which instrumented source ran. Parity answers whether observed
behavior matched the pinned oracle. Neither a declared API nor a selected case
is automatically covered.

## Release 0.1.3

At released commit `fd78eb80402a0d6d99e6b696d0a1ebf9ba11d5fb`:

| Observation | Result | Boundary |
| --- | --- | --- |
| Python 3.10 parity | 11,348 / 11,348 | Selected public inputs on CI's runner |
| Python 3.12 parity | 11,348 / 11,348 | Selected public inputs on CI's runner |
| Node and browser WASM parity | 11,348 / 11,348 each | Selected runtimes and corpus |
| Rust coverage plans | 24 complete; zero failed executions | 11,328 target coverage executions |
| Measured changed Rust lines | 25 / 25 hit | Native macOS LCOV against base below |
| Added lines without LCOV records | 7 | Comments, attributes, or blank lines; no credit |
| WGSL source lines | Unmeasured | Rust coverage does not instrument shaders |

[Main CI](https://github.com/appunni-m/pillow-rs/actions/runs/35014896711) and
[release CI](https://github.com/appunni-m/pillow-rs/actions/runs/35017008075)
identify that source. The changed-line base is
`fcfb956d172d744238a86f30129233eb00ca8b8e`. These observations do not imply
100% repository coverage, every platform combination, or complete Pillow support.

The measured LCOV SHA-256 is
`0622dde0a4ceba6ad0a5987e9249490742bb6c58d8d44654dbbeab2fa3d1a995`.
Its retained collection uses
`target/coverage/release-0.1.3-fd78eb804-rust.lcov` with its
`.context.json`, and the matching coverage/changed-line JSON under
`build/migration-parity/release-0.1.3-fd78eb804-*`.
Later documentation commits do not change those receipts.

## Collect evidence for a change

```sh
make migration-parity-inputs-check
make migration-parity-coverage-rust
make migration-parity-changed-line-coverage MIGRATION_COVERAGE_DIFF_BASE=<base>
make migration-parity-coverage-receipt-test
```

Use distinct report paths for independent or incremental campaigns. The source
receipt binds revision, source hashes, actual assets, instrumented binary,
selected inputs, and execution status. The collector rejects source/input
changes during collection. Never attach an old context to a regenerated report.

Combined reports preserve each backend's selected IDs and execution results.
Unique plans form the plan denominator; test totals count executions across
requested backends. A failed or incomplete backend fails collection.

## Reverse coverage and missing paths

```sh
make migration-parity-pillow-coverage
make migration-parity-pillow-missing-manifest
```

Reverse coverage measures which pinned Pillow source the corpus reaches. It
helps find missing application paths; it does not measure Rust coverage.
Generated missing-feature reports are dated execution artifacts under
`build/migration-parity/`.

## Reduce repetitive parity inputs

Use `make migration-parity-reduction
MIGRATION_REDUCTION_ARGS='--candidates <reviewed-pairs.json> --output-dir <new-directory>'`
after reviewing inputs and live observations for equivalent behavior. The
candidate file is an array of `case_id` / `replacement_case_id` objects; each
replacement must remain selected with the same target profiles, and every
requirement/profile mapping must remain represented in the corpus. Merge the
requirement labels when identical workflows are consolidated. Measurement artifacts belong in ignored output
directories, never in active fixture inputs.

The command measures the same canonical parity cases separately on CPU, SIMD
and GPU, then tries removals in batches of 100. A changed coverage set restores
half the batch and recursively checks both halves against the initial baseline.
Rust executable lines, functions, regions and branch outcomes and Python lines
and branches must match exactly, including their instrumented denominators.
Unchanged percentages alone do not pass. External dependency source and WGSL
shader internals are outside these measured locations.
The GPU report must reach an actual hardware dispatch; selecting GPU while its
adapter is unavailable and all work falls back to CPU does not pass.

`MIGRATION_COVERAGE_PARITY_ONLY=1` omits native-only supplements from both
baseline and trials; the regular coverage command continues to run them.
Reports retain source, input and instrumented-build identities. The reduction
command never changes the corpus. Apply accepted removals in the generator,
regenerate parity/coverage/benchmark inputs, then verify parity and coverage
again. Matching coverage is necessary here but is not proof that two tests
protect the same behavior. Keep distinct contracts and numerical regressions.

`make migration-parity-reduction-test` checks the restoration and comparison
guards. Reusing an output directory resumes matching completed measurements;
changed sources, inputs, candidates or execution failures stop the comparison.

The manifest and [maturity guide](COMPATIBILITY.md) define the selected scope.
Missing records, unavailable adapters, and unexecuted paths stay unmeasured.

## Native font coverage observations

`make migration-parity-font-native-coverage` runs the supplemental font inputs
against the checkout's Python binding and a test-only Rust driver. The driver
calls `text_bbox`, `getbbox_binary`, and `render_text_binary` directly; these
methods are not exposed through Python. The command writes individual returned
values or API exceptions to `build/migration-parity/font-native-observations.json`.
These are coverage probes, **not parity passes**: an API exception records what
ran without asserting that the exception is correct. Harness failures, unknown
operations, duplicate case IDs and unavailable drivers fail the command.

The full Rust coverage lane builds and instruments that driver, includes font
wrapper execution in Python coverage, and binds each backend's observation
report to its coverage receipt. `make migration-parity-font-native-test` checks
the adapter and failure-reporting guards. Keep native input removals separate
from generator-owned parity removals and compare fresh full-lane measurements
before and after any native removal.
