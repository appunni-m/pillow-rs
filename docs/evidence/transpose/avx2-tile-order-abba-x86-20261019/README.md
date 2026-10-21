# AVX2 RGBA Transpose tile-order A–B–B–A — no-go (2026-10-19)

## Decision

Do not retain the candidate. It swapped the two full-tile loops so one
eight-row output band was visited at a time, while leaving AVX2 admission,
allocation, edge traversal, and the tile shuffle/store body unchanged. The
selected full-call benchmark completed, but the same candidate binary produced
materially different Pillow-relative ratios in B1 and B2. The apparent mean
gain is not repeatable enough to accept.

The candidate changed only
`transpose_full_tiles` in
`pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs`.
Baseline source SHA-256: `a66fb6b97212b0504bb355d2501c84113c1e787b08f58e0f28507722937bf3ff`.
Candidate source SHA-256: `7d25605c11c19581acd4398cd433d7c30540d6297e81bd4da22e55a30c5b1ed3`.

## Frozen workload and execution

- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`
- Timed boundary: `transpose-0;transpose-1;materialize`
- Backend: SIMD only (`MIGRATION_BENCHMARK_BACKENDS=simd`); Pillow and target ran in separate adapter processes.
- Warm release; 5 warmups; 20 iterations × 5 samples; concurrency 1; 100 latency observations per subject.
- Host: AMD Ryzen 5 2600X, Linux x86_64; CPython 3.12.11; Pillow 12.2.0.
- Each report selected and measured exactly one workload. No full benchmark or parity suite ran.

## A–B–B–A results

Latency is the median in milliseconds for the full materialized public
pipeline. `SIMD/Pillow` is the target-to-oracle ratio from the same report;
values above 1 are slower than Pillow.

| Run | Treatment | Pillow | SIMD | SIMD/Pillow | SIMD ops/s | Main parity |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| A1 | Baseline | 2.007872 | 2.567921 | 1.2789× | 389.42 | Pass |
| B1 | Output-row-band traversal | 2.304067 | 2.928835 | 1.2712× | 341.43 | Pass |
| B2 | Same candidate binary | 2.344296 | 2.438197 | 1.0401× | 410.14 | Reused B1 proof |
| A2 | Baseline | 2.163730 | 2.611486 | 1.2069× | 382.92 | Pass |

The first adjacent pair is effectively unchanged: B1 improved the ratio by
0.6% while both Pillow and SIMD medians rose by about 15%. In the second pair,
B2's ratio is 13.8% lower than A2's, but B1 and B2 use the same candidate
binary and their target medians differ by 16.8%. This within-candidate spread
is larger than the apparent paired effect. The four-run mean ratio is 1.243×
for A and 1.156× for B, but that aggregate hides the inconsistent adjacent
pairs. The candidate remains a measurement hypothesis, not an accepted gain.

## Parity scope and executed path

The workload preflight selected
`PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`
for `python-simd`; A1, B1, and A2 passed, and B2 reused B1's report-bound proof
with the same extension hash. The separate odd-size guard
`PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0`
was not selected in this experiment. Do not claim that this trial demonstrated
that guard.

All four benchmark receipts report `actual_backend=simd`, no fallback, and
`avx2-transpose` with 73,728 vector blocks and 12,288 scalar-tail bytes. The
second operation remains `native-copy`. Each call makes one 2,371,584-byte
host allocation and materializes two host buffers totaling 4,743,168 bytes;
these counts are unchanged by the candidate.

## Artifact identities

The runner checkout was at `418d5f0cd` with a dirty worktree. Extension hashes
were stable within each treatment:

- A1/A2 baseline: `756fcf8c00b256656ba32b9f28f54c819a6cd3ee7804561a5aff31784028853f` (12,880,176 bytes)
- B1/B2 candidate: `1ecc31b05295ec100da22f201dc518b734aed892ab5412dfbca276f25b0e181d` (12,880,208 bytes)

| Run | Benchmark report | Parity report |
| --- | --- | --- |
| A1 | `migration-benchmark-ab612366572a48779e2e8763768223a7` | `migration-parity-benchmark-gate-a439a9d704fa4f02b6719d0326706df9` |
| B1 | `migration-benchmark-ffc090b178934b7ab2503ca93c5670a3` | `migration-parity-benchmark-gate-39363ff4437c4937b4ce016bb0419b5e` |
| B2 | `migration-benchmark-e6288936f2774961abd6d991fe8d38a1` | Reused B1 |
| A2 | `migration-benchmark-293bd34f49be4a4a952fe5dc21213410` | `migration-parity-benchmark-gate-5d2c0367b354444398ddeab6e7a1e37d` |

Compact benchmark reports, raw samples, producer/execution attestations, and
parity attestations are retained in `A1/`, `B1/`, `B2/`, and `A2/`. Full
12.7 MB parity reports remain on the native runner; they were not copied into
this checkout. Checksums for the retained files are in `SHA256SUMS`.
