# Current-source RGB 5×5 BLUR SIMD check — ARM64

This is one selected, current-source whole-workflow measurement for the exact
RGB `BLUR` kernel. It is not an A/B comparison, does not promote a baseline,
and does not complete `PIL.Image.Image.filter` across modes or workloads.

## Selected workload

- Workload: `pipeline-matrix.expanded.filter5x5.1024x768`
- Exact case: `PIL.Image.Image.filter.nuanced.rgb-blur-5x5-noise-1024x768`
- Input: RGB noise, 1024×768; Pillow 12.2.0; warm release build; concurrency 1
- Boundary: whole public workflow, including construction, filtering,
  allocation and terminal materialization
- Sampling: one warmup, three iterations × two samples
- Backend selection: Pillow and `python-simd` only
- Source: revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty working tree
- Stable runner: `lazys-macbook-pro-arm64` (Apple ARM64)

Exact parity passed 1/1 for `python-simd`. The timing report records Pillow at
11.414438 ms median and SIMD at 2.303771 ms, for **4.954676× Pillow**. The
execution receipt reports requested and actual SIMD, terminal completion, no
fallback, and 291,848 vector blocks with 3,056 scalar-tail components. The
result is 0.91% below the fixed 5× SIMD target, so it remains unpromoted.

The retained native x86_64 measurement for this same workload reports 35.631
ms Pillow and 5.812 ms SIMD (6.13×), with exact CPU/SIMD parity and positive
vector receipts. Its hot Filter5x5 function and ring-kernel hashes match the
current source; the separate [x86 evidence](../rgb-blur-ring-simd-x86-20261019/README.md)
documents remaining dirty-tree differences and the unavailable remote parity
report. Do not combine ARM64 and x86_64 medians or treat either single report as
an A–B–B–A source comparison. The earlier 53.409 ms generic-path x86 result has
a different runner identity and is not a matched control for the ring result.

## Artifacts and checks

The selected ARM64 benchmark run is
`migration-benchmark-7797bff6b49842b5b759052e0a47ee56`; its report SHA-256 is
`18d719efd535f5cb22fc15ee5661a6631c114a691760d5a92ae12e7a0cd7e007`. The
parity run is `migration-parity-benchmark-gate-8bc2886bc2974a79ac792a3eb303dc06`;
the report SHA-256 is
`f408b022ef486e03b12005a3b6b953cf13b32ee12424f91ba5e3cb6790a252bb`.
Reports, attestations and raw samples are in
[`reports/selected-noise/`](reports/selected-noise/). The ignored DuckDB now
contains this report; no baseline was promoted.

`make migration-parity-benchmark` ran with the single workload ID and
`MIGRATION_BENCHMARK_BACKENDS=simd`. Its 38 focused selection contracts passed.
The first attempt exposed a genuine fixture defect: the reused-parity test's
synthetic case only declared CPU while the environment selected SIMD, producing
an empty comparison set and stale summary counts. The test now declares the
frozen profile set and intersects it with the active profile selection; its
assertions and benchmark thresholds are unchanged.

A fresh x86 rerun was not needed: an alternate remote checkout lacked `maturin`
and its virtual environment lacked `pip` and `PyYAML`. Build/parity stopped
before any benchmark workload ran. The existing source-matched x86 evidence is
retained; no target directory or large file was copied to or removed from the
runner. No full benchmark or parity campaign and no coverage ran. The ring
operation remains open for other kernels, modes, sizes, Pillow-SIMD, and GPU
latency/throughput.
