# x86 serial CPU RGBA transpose 32×32 tile candidate — 2026-10-18

## Scope

This is a discarded tile-size experiment under Kata `pillow-rs#m2w4` and its
read-only investigation `pillow-rs#awj9`. The serial CPU RGBA tile was changed
from 16×16 to 32×32. SIMD and Parallel CPU dispatch were unchanged. The local
and native x86 runner source were restored to 16×16 after measurement.

## Focused verification

- Native x86 runner: CPython 3.12.3, Linux x86_64, Pillow 12.2.0; release
  `make build-parity` completed before the candidate run and again after
  restoring the 16×16 baseline.
- Exact CPU parity passed 2/2: the aligned 772×768 transpose/mirror case and
  the odd 1025×1023 guard. Direct parity run:
  `migration-parity-cd5189f24e9642d9ae88fde6fe3d170b`.
- Benchmark correctness gate passed for the selected workload. The report
  records actual CPU execution, terminal completion, no fallback, two fused
  operations, one 2,371,584-byte pixel allocation, and two host buffers
  totaling 4,743,168 bytes.

## Result

Workload `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`,
warm, 5 warmups, 20 iterations × 5 samples, concurrency 1, including both
transpose calls and final materialization:

| Subject | Median | p95 | Speedup vs Pillow |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 2.097 ms | 2.649 ms | 1.00× |
| Serial CPU, 32×32 candidate | 3.154 ms | 3.403 ms | 0.665× |

This run does not show an improvement. A preceding 16×16 run measured CPU at
3.148 ms; these separate runs are not an interleaved A–B–B–A trial, and Pillow
also moved substantially between them. Do not attribute their small CPU
difference to tile size. The candidate misses the CPU ≤ Pillow target, so the
change was reverted and Transpose remains open. The independent AVX2 SIMD
candidate also remains slower than Pillow on x86.

## Artifacts

- Benchmark run: `migration-benchmark-0c833b3f47bd4befa08638f272e4df52`.
- Benchmark parity gate: `migration-parity-benchmark-gate-4893614a3baa4fdc87ed68a128677de1`.
- Benchmark report SHA-256: `a0cf358fd48bd86fa6c8d4a48499e15d62d58da7550d277a3fabd0ec82ffc64e`.
- Benchmark attestation SHA-256: `25f22792b3d43aea0c6836fb9f7831e838a19a32b6a9b0f1422449282ae0a0fd`.
- Raw samples SHA-256: `a86e1f77bf69d919fb493f56230330781f85ebcbd201731b42373ceece0ffc45`.
- Benchmark parity attestation SHA-256: `35d6cd629de566aa1e43624f304867aba58052a65ef55b37c539eb9ea4f3d675`.
- Direct parity attestation SHA-256: `902bbed409d65fbfef88e66a41702975d4c05bdffa919a46373a1362395287d2`.

The full parity report is 35 MB and remains on the runner; its attestation is
saved here. The x86 runner received no target directory or large source copy.
