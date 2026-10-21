# GaussianBlur RGBA source-bound CPU and SIMD baselines — ARM64 — 2026-10-21

This is one source-bound, whole-workflow rebaseline for the noisy RGBA
GaussianBlur workload. It is not a candidate-vs-control optimization result.

## Frozen selection and identity

- Workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Exact parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- Mode and input: RGBA, 1024×768, radius 7.5, noisy pixels
- Boundary: complete workflow through output materialization
- Policy: warm; one warmup; three iterations × two samples; concurrency one
- Runner: `lazys-macbook-pro-arm64`, macOS 15.7.7, CPython 3.12.13, Rust 1.96.1
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty tree
- Manifest SHA-256: `fa874a45175e2979a8671d28e0b89f1d9508fe9c8410f7acd7af55ab681f6550`
- Benchmark input SHA-256: `9f0a8fdbef7c4eb921ac9897a7ddd1810c213df4fe962fb8e7d9870f541202a8`
- Parity input SHA-256: `fffed899cc0391dfdc2e33d89a5059263b5b3410520e252835ffe1ac31b942d9`
- Materialized `_core.abi3.so` SHA-256: `83541d6d8c53d510a916005c66d13daced41148651aef6c0c9378750bc98484d` (10,055,456 bytes)

The benchmark report is `benchmark-result.json` (SHA-256
`d2c1906e8d76313803d60754686cdede4b6e346791418628bc79d19a5a6fe77e`;
run `migration-benchmark-5dd4da9bb16d4b17a8f4ccd60890da16`). The exact
parity report is `parity-result.json` (SHA-256
`5ac3ea3434c4de63fe91606851473a5d597f8b61591086d194475d35597a12fb`;
run `migration-parity-benchmark-gate-4913aa9216224953a33bb5df1048e260`).
Both selected comparisons passed. The retained source-build witness binds the
Cargo artifact to the Maturin-materialized module. CPU and SIMD direct-operation
links passed parity, runtime, producer, source/build, and backend-execution
admission; the target receipts report actual `cpu` and `simd`, terminal
completion, and no fallback.

## Same-report results

| Profile | Median | P95 | Pillow / target | Backend proof | Generated baseline |
| --- | ---: | ---: | ---: | --- | --- |
| Pillow | 6.079605 ms | 6.391416 ms | 1.000× | Pillow oracle | Reference |
| Serial CPU | 5.281875 ms | 5.356042 ms | 1.151× | Actual CPU; terminal; no fallback | Strict: `gauss-rgba-1024-arm64-cpu-strict` |
| SIMD | 6.367209 ms | 6.653584 ms | 0.955× | Actual SIMD; terminal; no fallback | Tracking: `gaussian-blur-rgba-arm64-simd-20261021` |

The CPU row meets the absolute latency target for this workload. The SIMD row
misses its 5× Pillow target by a wide margin. Neither row demonstrates the
complete operation target across modes, sizes, architectures, or other
backends. No GPU or sustained-throughput result is present.

## Generated baseline records

The DuckDB database is generated state and is not part of this evidence
directory. The strict CPU baseline was promoted as baseline `a44df95c2b6e` from
the benchmark artifact above. The SIMD tracking baseline was promoted as
`4b1e65a73d07`; tracking records the exact parity-backed starting point while
the absolute SIMD target is unmet.

The older CPU tracking baseline uses a different producer/build context. A
cross-run check against it correctly returned `not proven`, so this measurement
uses a separate strict channel. Future checks must use a new candidate artifact
with the same workload, oracle, benchmark producer, runner, and measurement
contract; an active baseline cannot be checked against itself.

Full report, samples, runtime receipts, producer attestations, selected direct
operation links, manifest, selected inputs, and source-build witness are kept
beside this README. No full benchmark campaign or coverage run was performed.
