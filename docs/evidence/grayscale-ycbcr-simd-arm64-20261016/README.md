# YCbCr Grayscale NEON unroll trial — ARM64, 2026-10-16

This focused trial evaluates a four-block NEON unroll in the existing
YCbCr-to-L byte extraction kernel. It preserves direct Y-byte extraction,
allocates the final output once, and retains the 16-pixel vector and scalar
tail behavior. The code change is in
[`ycbcr.rs`](../../../pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs).

## Workload and parity

- Benchmark workload: `pipeline-op.grayscale.material-ycbcr-noise-1024x768`.
- Main parity case: `PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`.
- Edge guard: `PIL.ImageOps.grayscale.nuanced.performance-ycbcr-odd-byte-tail` (3×1).
- Input asset SHA-256: `8d09ced0ef27917e84162a9bc924b41913252d85adfb87a2464a307c2a8cd110`.
- Boundary: warm public `call` plus `observe-result`; release build; 5 warmups,
  20×5 measured iterations, concurrency one. Benchmark setup remains outside
  the timed workflow.
- Both cases passed strict SIMD parity for the control and four-block candidate.
  The candidate's second timing run reused that same-source, same-binary parity
  receipt. Parity and benchmark schema validators passed.
- Focused Rust boundary test
  `native_ycbcr_grayscale_handles_unrolled_vector_boundaries` passed at widths
  15, 16, 17, 63, 64, 65, 79, and 80, covering vector tails and the four-block
  boundary.

## Results

All medians are from Pillow 12.2.0 and the target on the same Mac ARM64 host.
The three candidate runs used the same loaded core hash and workload contract.

| Run | Pillow median | SIMD median | Same-report SIMD speedup | SIMD throughput |
| --- | ---: | ---: | ---: | ---: |
| Pre-change control | 98.001 µs | 76.229 µs | 1.286× | 13,119 ops/s |
| Four-block B1 | 78.334 µs | 49.271 µs | 1.590× | 20,296 ops/s |
| Four-block B2 | 126.375 µs | 48.500 µs | 2.606× | 20,619 ops/s |
| Four-block B3 | 87.521 µs | 47.334 µs | 1.849× | 21,126 ops/s |

The candidate SIMD median remained stable at 47.334–49.271 µs, about 35–38%
below the pre-change SIMD control median. Pillow's same-host medians varied
from 78.334 to 126.375 µs, so two of the three same-report ratios remain below
2× (1.590×, 2.606×, and 1.849×). Keep the candidate as a measured local
latency improvement, but do not mark the 2× checkpoint complete or average
away the per-run misses. This is ARM64 NEON evidence and says nothing about x86
SSSE3 or Pillow-SIMD.

## Executed path and materialization

The B1–B3 attestations show `requested=actual simd`, a completed terminal
result, and no fallback. Operation telemetry reports `neon-deinterleave`,
49,152 vector blocks of 16 pixels, zero scalar-tail pixels, and one Grayscale
operation. The measured result records one 786,432-byte host output allocation
and 3,145,728 peak live host bytes. This includes the public-call result
materialization boundary; no GPU work or transfer is involved.

The control extension SHA-256 was
`7e6fced7ae0a16ea94847861e99c91a3d8be8c5a791613a91f2de525fffbd57c`; the
four-block extension SHA-256 was
`dedbed1799f2f61972af51503d4b965e814c76ac272c1383ac861a4c88b71768`.
All runs used dirty source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`,
macOS 15.7.7 ARM64, and CPython 3.12.13.

## Receipts

- Control benchmark: `migration-benchmark-054486977c054fc8bfd79af01635721a`;
  parity: `migration-parity-613812e2ebd94d9194e8091d22baed59`.
- Four-block B1 benchmark: `migration-benchmark-8e440386689f4e9aadd27fa97bd0d1a9`;
  parity: `migration-parity-7f4283e6b0d348648fdc950cc5086af7`.
- Four-block B2 benchmark: `migration-benchmark-5ee0d89978a24ffe9c953b365518b900`;
  it reused B1's exact parity receipt.
- Four-block B3 benchmark: `migration-benchmark-4cf3aae72ae243009807ea13528ea017`;
  it reused B1's exact parity receipt.
- The control, B1–B3 reports, ordered raw samples, execution attestations,
  producer receipts, parity result, and parity attestation are retained beside
  this README. The local run ledger records all eight subject rows.
- Kata candidate tracking: `pillow-rs#evx5`; the full operation remains open.

No coverage ran. No commit or push was made; this result does not count as a
completed item in the 33-item batch.
