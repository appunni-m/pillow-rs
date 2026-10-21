# AVX2 transpose without output zero-fill — no-go (2026-10-19)

## Scope

This SIMD-only trial followed the scout in Kata `pillow-rs#nt5z` under the
open operation goal `pillow-rs#m2w4`. It moved AVX2 admission before the
initialized output allocation, then let the AVX2 kernel own a `MaybeUninit<u8>`
buffer and convert it to `Vec<u8>` after writing full tiles and scalar edges.
Serial CPU and every fallback path stayed unchanged.

The proposed work reduction was one avoided zero-fill of the 772×768 RGBA
output (2,371,584 bytes) in the selected composed call. It added no allocation
and did not change output bytes. Exact-parity guards passed, but the trial did
not demonstrate a repeatable full-call gain, so the candidate was reverted.
The parent Transpose operation remains open.

## Frozen workload and verification

- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`
- Main parity: `PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`
- Odd-size guard: `PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0`
- Boundary: `transpose-0;transpose-1;materialize`
- Warm release; 5 warmups; 20 iterations × 5 samples; concurrency 1; 100 latency observations per benchmark run.
- Runner: `x86_64-ryzen-5-2600x-native-01`, AMD Ryzen 5 2600X, Linux x86_64; CPython 3.12.3; Pillow 12.2.0; target revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty worktree.

The selected main case and odd guard each passed on A1, B1, and A2. B2 reused
the B1 main-case proof for the same candidate binary. The target receipt
reported `actual_backend=simd`, one SIMD backend execution, `avx2-transpose`
with 73,728 vector blocks, no fallback, one 2,371,584-byte host allocation,
and two host-buffer boundaries totaling 4,743,168 bytes. The second composed
operation retained its `native-copy` receipt. Path and allocation receipts
were unchanged by the candidate.

## A–B–B–A results

Latency is the median in milliseconds. `SIMD/Pillow` is the target-to-oracle
latency ratio from the same report; values above 1 are slower than Pillow.

| Run | Source | Pillow | SIMD | SIMD/Pillow | Main parity |
| --- | --- | ---: | ---: | ---: | --- |
| A1 | Initialized output | 2.150592 | 2.769769 | 1.2879× | Pass |
| B1 | Uninitialized output | 2.152608 | 2.773510 | 1.2884× | Pass |
| B2 | Uninitialized output | 2.583954 | 2.772835 | 1.0731× | Reused B1 |
| A2 | Initialized output | 2.586709 | 2.828985 | 1.0937× | Pass |

The first adjacent pair showed no gain: B1's target median was 0.14% slower
than A1 and its within-report ratio was effectively unchanged. B2 was 1.98%
faster than A2, but the mean within-report `SIMD/Pillow` ratios across the two
A and two B runs differ by less than 1%, while the ratio shifted by roughly
15% between the two run pairs. This is not a repeatable effect that supports
accepting the unsafe allocation change. SIMD also remained slower than Pillow
in all four runs.

## Artifact identity

The captured extension hashes were stable within each treatment:

- A1/A2 initialized baseline: `c80fbc6fed570bb2e61928ffe23fe73b8af0a0c052b8f0d96edc42c7543d0ff7`
- B1/B2 uninitialized candidate: `e2b782043b2962ddedc9692252906ff4ed54426af9ff0dd79cd9b42ffe5b697e`
- Baseline AVX2 source SHA-256: `ce9af390206a105fa4b58e65e73ed5d2155248db72aa265036a1bde6654cedfd`
- Candidate AVX2 source SHA-256: `a7952517675b719ac921a1334234b9c010fce04cff4889cd1b43c0093c5c3daf`
- Baseline adapter source SHA-256: `7e5fcee16ea1d338a7573696f50145e613999bd2f770588e4bccfc39ee1e3b1f`
- Candidate adapter source SHA-256: `58d5b863ebedd42a3332905bd12477253063d9346035c52b4b11c58c0fa11ec1`

Benchmark run IDs, in A–B–B–A order:

1. A1 `migration-benchmark-f4d47e7f1b0043deb8642ad7a111b8e9`
2. B1 `migration-benchmark-ce2ef98e0d8440b0a06e7fbb8e2f367e`
3. B2 `migration-benchmark-d637c7060c314f6ab935495e836a0433`
4. A2 `migration-benchmark-31f12d32b299487297f89eaf1de6c8c4`

The corresponding benchmark reports, execution and producer attestations,
raw samples, and parity attestations are retained beside this README. Main
parity run IDs were `migration-parity-benchmark-gate-bfc9ae80d5154087b0fee5a2af5faa4e`
(A1), `migration-parity-benchmark-gate-e68e3b90a7fd4c9daa0094b0f056d33d`
(B1), and `migration-parity-benchmark-gate-af65debe635f41d48f273336419fa03b`
(A2). Strict odd-guard parity run IDs were `migration-parity-1c2549be94f74f7e8c6985d2ee47cbb0`
(A1), `migration-parity-334fa3a576a2448bb48a3d670cd4f729` (B1), and
`migration-parity-c6b0a111506d47828fd2484c60c6762c` (A2).

Full parity JSON reports (13–22 MB each) remain on the native runner at
`/home/appunni/work/pillow-rs-current-local-x86-20261019/build/migration-parity/transpose-avx2-uninit-abba-20261019/`;
their report digests are bound by the copied attestations. No runner files
were deleted, and no target tree or generated parity corpus was copied.

Reusing A1 parity after the A2 rebuild was rejected by the identity check.
The extension SHA-256, path, and inode matched, while `ctime_ns` and
`mtime_ns` changed. A2 therefore ran fresh selected parity instead. This
matches the open identity-churn follow-up in Kata `pillow-rs#bz5d`; it is not
a parity failure.

## Decision

Reject this candidate and retain the initialized AVX2 output path. Do not infer
that avoiding zero-fill improves a public call from source-level store counts
alone. Continue Transpose separately: serial CPU is still a current-source
miss, and SIMD still misses Pillow and the repository's 5× target. Placement
remains governed by the single standard in
[`docs/IMPLEMENTATION_STRUCTURE.md`](../../../IMPLEMENTATION_STRUCTURE.md).
