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

The manifest and [maturity guide](COMPATIBILITY.md) define the selected scope.
Missing records, unavailable adapters, and unexecuted paths stay unmeasured.
