# Screen GPU arithmetic identity trial (2026-10-17)

## Scope

- Public operation: `PIL.ImageChops.screen`
- Timed workload: `pil-imagechops.screen.standard`
- Exact parity case: `PIL.ImageChops.screen.nuanced.performance-materialized-rgb-noise-1024x768`
- Frozen input: RGB 1024×768 noise pair; warm cache; 5 warmups; 20 iterations × 5 samples; concurrency 1.
- Boundary: `call` plus `observe-result`, including public byte materialization, GPU transfers, synchronization, and allocation.
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7, arm64; CPython 3.12.13.
- Checkout: dirty revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.
- Baseline `_core`: `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Candidate `_core`: `95727567fc0a3aae2592c22a712b7c49d1e020eb885fbefea92f3e703634b463`.
- Restored source shader SHA-256: `a789cec4b5f08d3eea6d2bcb673c44cff43a30d8ac41db95d63e4f72a8564bd8`.

The candidate rewrote each byte channel from `255 - ((255-a)*(255-b)/255)` to `a + b - (a*b/255)`, preserving integer floor division. Only `pillow-rs/src/compute/pool_gpu/shaders/screen.wgsl` changed during B1/B2. The original shader was restored and rebuilt before A2.

## A–B–B–A latency

Medians in milliseconds; each report contains 100 measured samples. All runs selected only SIMD and GPU targets, plus Pillow as the oracle.

| Subject | A1 control | B1 candidate | B2 candidate | A2 restored control |
| --- | ---: | ---: | ---: | ---: |
| Pillow | 3.882021 | 3.892271 | 3.815646 | 3.832208 |
| SIMD | 0.383709 | 0.387562 | 0.379417 | 0.423146 |
| GPU | 1.042584 | 1.125292 | 1.284813 | 1.189959 |

Run IDs and parity gates: A1 `migration-benchmark-49a7249c71fb4ee6a4b48d99f5633062` / `migration-parity-benchmark-gate-27d4841c47d149f384701a1273441392`; B1 `migration-benchmark-b710d20a13d64a43bdc7326d06372303` / `migration-parity-benchmark-gate-4ff0b1cb9ed147c39fbe8954973a7fdc`; B2 `migration-benchmark-44b99f718f7648ec85aa6e56503f792d` / reused B1 gate; A2 `migration-benchmark-0aac55e0057b418d82ef0104566be3da` / fresh `migration-parity-benchmark-gate-f87a2016f6a54d92bc689402c2e29917`. Tail-guard parity is `migration-parity-61b25672298047ac8818e544f77b17ff`.

Median of the two control report medians: GPU 1.116271 ms, SIMD 0.403427 ms. Median of candidate medians: GPU 1.205052 ms, SIMD 0.383490 ms. The candidate GPU median is 7.96% slower than control; its GPU latency is 2.90–3.39× the matched SIMD latency. The rewrite is rejected. A2 parity reuse was refused because the runtime identity differed; the recorded A2 result is the fresh `A2-retry` report.

The timed GPU receipts for A1, B1, B2, and A2 show actual GPU execution, terminal completion, one dispatch, zero fallback, 2,359,296 uploaded bytes, 2,359,296 readback bytes, and no mode conversion. The SIMD receipt proves 147,456 vector blocks and zero scalar tail. CPU was not part of this shader-only trial. Candidate completed-window throughput was not measured because direct latency already regressed; the existing baseline queued Screen cohort remains 0.328× SIMD and is recorded in Kata issue `pillow-rs#85kj`.

## Parity

- A1 baseline parity: 2/2 selected SIMD/GPU comparisons passed.
- B1 candidate parity: 2/2 selected SIMD/GPU comparisons passed.
- B2 reused B1 parity after runtime identity validation.
- A2 restored baseline parity: fresh 2/2 selected SIMD/GPU comparisons passed.
- Candidate strict GPU byte-tail guards: L, LA, RGB, and RGBA 5×5 cases passed 4/4.

The tail-guard selectors are `PIL.ImageChops.screen.nuanced.gpu-native-byte-tail-{l,la,rgb,rgba}-5x5`. No full test or parity campaign and no coverage ran.

## Artifacts

- `A1-*`: control before the candidate.
- `B1-*`, `B2-*`: candidate reports and receipts; B2 parity is the runtime-validated B1 parity copy.
- `B1-gpu-tail-*`: four candidate GPU byte-tail guards.
- `A2-retry-*`: restored source control with fresh parity.
- `A2-*`: failed parity-reuse attempt only; excluded from results because the harness rejected the runtime identity.

The validated benchmark rows were appended to `docs/evidence/performance-optimization-local-runs.csv`. No source change from this trial remains.
