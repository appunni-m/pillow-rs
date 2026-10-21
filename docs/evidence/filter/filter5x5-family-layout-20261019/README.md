# Filter5x5 family-layout check

This records the performance and parity guard for moving the SIMD Filter5x5
entry from the large dispatch facade into its existing family adapter and
kernel tree. It is a layout/code-ownership change, not an algorithm claim.

## Result

All four selected 1024×768 public workloads passed exact parity on ARM64 and
native x86_64. Every target receipt reported the requested backend: CPU used
`cpu`; SIMD used `simd` with the `vector` operation path and zero fallback.
The end-to-end workload includes setup and terminal materialization.

The retained native x86 RGB control was measured on the same Ryzen 5 2600X
runner as the candidate. Its median moved from 6.329 to 6.303 ms for CPU and
from 5.812 to 5.729 ms for SIMD. These are separate six-sample cohorts, not an
interleaved A–B–B–A experiment; this is a regression guard, not evidence of a
speedup.

| Architecture / workload | Pillow ms | CPU ms | SIMD ms | Comparison |
| --- | ---: | ---: | ---: | --- |
| ARM64 RGB Filter5x5, before | 11.884 | 2.508 | 2.287 | control |
| ARM64 RGB Filter5x5, after | 12.107 | 2.417 | 2.308 | SIMD +0.9% |
| ARM64 L noise 5×5, before | 3.903 | 1.266 | 1.205 | control |
| ARM64 L noise 5×5, after | 3.880 | 1.268 | 1.214 | SIMD +0.7% |
| ARM64 LA noise 5×5, before | 7.205 | 1.817 | 1.752 | control |
| ARM64 LA noise 5×5, after | 7.261 | 1.813 | 1.741 | SIMD −0.7% |
| ARM64 I noise 5×5, before | 6.080 | 1.990 | 2.033 | control |
| ARM64 I noise 5×5, final repeat | 6.265 | 2.041 | 2.058 | SIMD +1.2% vs control |
| x86_64 RGB Filter5x5, before | 35.631 | 6.329 | 5.812 | retained control |
| x86_64 RGB Filter5x5, after | 35.271 | 6.303 | 5.729 | SIMD −1.4% |

The x86 candidate also checked L, LA, and I mode workloads. Their medians were
1.161/1.979/9.436 ms, 2.398/8.358/22.238 ms, and 7.401/6.744/15.021 ms for
SIMD/CPU/Pillow respectively. Those three modes have no retained x86
before-measurement here, so they establish parity and actual execution only,
not a before/after latency conclusion. The ARM I final-repeat run used 100
samples; its initial candidate run was noisy and is not used as the final
comparison.

## Workloads and reports

The same four IDs were selected on both architectures:

- `pipeline-matrix.expanded.filter5x5.1024x768`
- `pipeline-op.filter.material-l-noise-5x5-1024x768`
- `pipeline-op.filter.material-la-noise-5x5-1024x768`
- `pipeline-op.filter.material-i-noise-5x5-1024x768`

Compressed machine-readable benchmark reports are beside this README:

- `arm64-before.json.gz` — `migration-benchmark-1fdff336949d471cbf4b3e5baff5a6b7`
- `arm64-after.json.gz` — `migration-benchmark-43353078759b4c4085ed017c1c1ec323`
- `arm64-i-repeat.json.gz` — `migration-benchmark-59748de1d13d4b44add5608d01f4649a`
- `x86-after.json.gz` — `migration-benchmark-8a1716e044684302addda73f86298a1f`

The x86 RGB control report is retained at
`docs/evidence/filter/rgb-blur-ring-simd-x86-20261019/noise/identified-runner/benchmark.json`
(`migration-benchmark-a8bdc7ac4016428d9f233e78de723c63`). The large raw x86
parity dump was not copied; the benchmark report records the exact-parity
gate outcome and evidence ID for every selected workload.

## Placement decision

`adapters.rs` re-exports the Filter5x5 family entry. `adapters/filter.rs` owns
admission, mode routing, backend receipts, and result materialization. The
SIMD-only LA single-outlier loop is under `adapters/filter/kernels/la.rs`.
LA-binomial and I32 loops remain in `ops/native_filter.rs` because serial CPU
and SIMD both call them, and the byte-block loader remains shared with 3×3.
Do not duplicate these kernels to make paths look uniform. Their current
location below `pool_simd` is a concrete cross-backend ownership exception;
move them only after defining a genuine shared compute-kernel owner and
rechecking CPU and SIMD parity, receipts, and complete-call latency.

Kata: `pillow-rs#rxcj`.
