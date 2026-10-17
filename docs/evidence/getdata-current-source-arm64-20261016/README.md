# GetData HSV band 2 current-source ARM64 refresh — 2026-10-16

This one-workload refresh checks the highest-priority CPU miss in the pinned
benchmark matrix. It was run after the read-only identity audit in Kata
`pillow-rs#a53w`; it does not change the pinned published matrix because the
checkout is dirty.

## Frozen workload and parity

- Workload: `pipeline-chain.terminal-read.getdata.hsv-band2-1024x768`
- Exact parity case: `PIL.Image.Image.getdata.performance.material-hsv-band2-1024x768`
- Profile: serial `python-cpu` only; `parallel` is disabled.
- Timing: `band-data` observed step, warm cache, five warmups, 20 iterations ×
  five samples, concurrency one (100 timed executions per subject).
- Parity: completed, 1 selected and executed, 1 passed, 0 failed.
- Benchmark and parity result schema validation both passed.

## Results

| Subject | Median | P95 | Throughput |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 5.2384165 ms | 5.372125 ms | 190.897 ops/s |
| pillow-rs serial CPU | 2.4360415 ms | 2.472084 ms | 410.502 ops/s |

Pillow/CPU latency is **2.15038×** in this same-run comparison. The target
receipt requests and executes CPU, reports one completed `GetData`, terminal
completion, and no fallback. It records one 786,432-byte host pixel buffer,
zero extra host-allocation telemetry, and zero mode conversions. Python
`PyBytes` allocation and filling are inside the measured `band-data` call.

## Identity and disposition

- Benchmark run: `migration-benchmark-7500dfa113cd416faec76053b8e7c7db`.
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty tree.
- Loaded core SHA-256: `cf5edefafcee72aa0d2f9163f62f04a46cfd97d82a8d272779cdcaf49166c379`.
- `pillow-rs/src/image.rs` SHA-256: `e8670886bbf370900487407a782d6bdbd0a262dfec55dedf6289108bcd2ba009`.
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`.
- Benchmark-input SHA-256: `7eaf80ff5b3b32123c066d7e52f8ce7dcbbd359ff716c116df0256c694bae838`.
- Parity-input SHA-256: `ae2b332e03ddb8b5531c9123ac5b69970258828c65323269830a32666189c1c8`.
- Oracle: Pillow 12.2.0 / CPython 3.12; `PIL._imaging` SHA-256
  `ed534830281dd092be3755632eac8422cdee53d2d8f045ff1779a7d795414d26`.

This identity-complete dirty-tree diagnostic is consistent with the
2026-10-15 current-source run at 2.1996×. It shows that the pinned clean row's
0.765807× result does not describe the current local source/build. It does not
rewrite that historical row or prove other hosts, sizes, modes, backends, or
the 2× floor across a clean committed release. Keep it diagnostic until the
source changes enter the scheduled commit batch and a matching clean cohort is
collected.

No test suite, broad parity or benchmark campaign, coverage, commit, or push
was run.

## Recheck with the currently loaded core — 2026-10-16

After the extension identity changed, I repeated only this CPU/Pillow pair using
the same workload and parity case. The new artifacts are in
[`recheck-current-core-315a2f32/`](recheck-current-core-315a2f32/).

| Subject | Median | P95 | Throughput |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 5.385855 ms | 5.590291 ms | 185.672 ops/s |
| pillow-rs serial CPU | 2.484896 ms | 2.508125 ms | 402.431 ops/s |

The same-run Pillow/CPU ratio is **2.167437×**. The exact parity case passed
1/1. The target receipt requests and executes CPU, records one completed
`GetData`, terminal completion, and no fallback; the returned 786,432-byte
buffer is included in the measured `band-data` call. Benchmark and parity
result schema validation passed.

This run used dirty source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`
and loaded core SHA-256
`315a2f3278395a2a2f9ae65221ad2fcbdb2cc463558931c0acd3ca35334ce198`. Its
manifest and input hashes are in the benchmark report. The source/build/input
identity differs from the prior rebaseline above, so this is a separate
post-build observation; it does not replace or modify the clean published
matrix row. CPU is the only target profile in this run. No SIMD/GPU profiles,
tests, broad campaigns, coverage, commit, or push ran.
