# AVX2 RGBA Transpose 2×2 macroblock experiment — no-go

## Decision

Reject the 2×2 schedule. It passed the selected parity cases, but regressed
the full materialized SIMD call by 6.69–7.87% against the prior current-source
control in two independent runs. No candidate code was retained. The original
kernel source and release extension were restored and their control hashes
verified.

## Hypothesis and frozen scope

The experiment kept the 8×8 AVX2 transpose body and visited tiles in local
16×16 regions, pairing two neighboring output-column bands while preserving
source-row traversal. The intended benefit was closer placement of each
32-byte store with the adjacent half of its 64-byte output cache line. Scalar
edge handling, output mapping, allocations, and materialization were unchanged.

- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`.
- Input: RGBA 772×768; `transpose-0;transpose-1;materialize`.
- Timed boundary: warm end-to-end public call including Python `bytes` output.
- Sampling: release, 5 warmups, 20 iterations × 5 samples, concurrency 1; 100 observations per subject.
- Host: AMD Ryzen 5 2600X, Linux x86_64, CPython target 3.12.3, Pillow 12.2.0.
- Aligned parity case passed 1/1 on actual SIMD. The 1025×1023 odd-size guard passed 1/1 on actual SIMD.

## Results

Ratios are same-report Pillow median divided by SIMD median. Each candidate
target was slower than its same-report Pillow result and the prior control.

| Run | Pillow median | SIMD median | SIMD p95 | Pillow/SIMD | Change vs control SIMD |
| --- | ---: | ---: | ---: | ---: | ---: |
| Control | 2.665251 ms | 2.696892 ms | 3.621239 ms | 0.9883× | — |
| B1 | 2.281029 ms | 2.909261 ms | 4.085456 ms | 0.7841× | 7.87% slower |
| B2 | 2.569770 ms | 2.877246 ms | 3.300843 ms | 0.8931× | 6.69% slower |

The Pillow median moved between runs, so use each report's paired ratio along
with the target median. Both measures reject the candidate. SIMD median
throughput was 343.731 ops/s for B1 and 347.555 ops/s for B2, versus 370.797
ops/s in the control report.

Both candidate receipts proved `actual_backend=simd`, terminal completion, and
no fallback. The `avx2-transpose` record remained at 73,728 vector blocks and
12,288 scalar-tail bytes; the second operation used `native-copy`. Resource
receipts were unchanged: one 2,371,584-byte host allocation, two host buffers
totaling 4,743,168 bytes, no conversions or transfers, and two fused
operations. The observed regression is in the end-to-end SIMD path, not a
fallback or changed measurement boundary.

## Parity and runner note

The aligned 772×768 case passed on the candidate extension as benchmark
preflight. The 1025×1023 odd-size case passed separately. The B1 and B2
benchmark reports reused the same passing report-bound parity attestation for
the same candidate build and inputs.

The first wrapper invocation completed parity and produced a complete parity
execution attestation, then aborted before timing while merging the optional
producer closure with `backend producer attestation is not complete`. That
failed invocation is not counted as benchmark evidence. The selected timing
runs reused the matching passing parity report and execution attestation;
parity assertions and benchmark workloads were unchanged.

## Source and artifacts

- Revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf` (dirty worktree).
- Control kernel SHA-256: `a66fb6b97212b0504bb355d2501c84113c1e787b08f58e0f28507722937bf3ff`.
- Candidate kernel SHA-256: `dc083770946c38623cfea2fcd11962c46224c9713d4c3b7f9cc71d719c802f39`.
- Candidate release extension SHA-256: `07bfb25cfd1e6a898b7e29ec336b4459151601de30f84b8a34651b894881102c`.
- Restored control extension SHA-256: `13e9f40576c0956b96b8895285f2c4bd1dabda12f28d56c2f61f7988ff09a6b3`.
- Benchmark B1: `migration-benchmark-8d7e88f5e670494293459e7c1c246cc1`; report SHA-256 `e1f500a9ef633a17ee80e31a8caeaf5bdae43262b5290f24e9899ecee655c0bc`.
- Benchmark B2: `migration-benchmark-f7a5e4547a7f42f88dd63e735aaeea7d`; report SHA-256 `4652ea31fc80e81308cd1f42eef06e6f1d46eb122a29e871b39c2edd1381ddfd`.
- Aligned parity: `migration-parity-benchmark-gate-d048c9e3391447c5a41e9e82e49d79c6`; report SHA-256 `bc4c984d0a7cb373cf21b2fd0dc4f0758a4ee539ebc6e758c1e8ed1feaed09df`.
- Odd parity: `migration-parity-99ec4d61bec346f382fa1a119346edba`; report SHA-256 `37155a20fe613f6e5120efec4a2d1c414cf62d29a0cd7db83a9c2d8c568ff7a6`.
- Runner artifacts remain under `/tmp` on the existing x86 host: `transpose-avx2-2x2-B1-x86-20261020.json`, `transpose-avx2-2x2-B1-raw-x86-20261020.jsonl`, corresponding B2 files, `transpose-avx2-2x2-B1-parity-x86-20261020.json`, and `transpose-avx2-2x2-odd-parity-x86-20261020.json`, with their report-bound attestations. No project tree or target directory was copied.

Only the selected transpose workload and its aligned and odd parity cases ran.
No full benchmark, full parity, test suite, or coverage campaign ran. Both
candidate and restored-control release builds succeeded with the existing
seven missing-documentation warnings.
