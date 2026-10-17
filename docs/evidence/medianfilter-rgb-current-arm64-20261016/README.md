# RGB MedianFilter(3) current-source latency and batch throughput — 2026-10-16

This is a dirty-tree ARM64 diagnostic for one native RGB workload. It does not
close MedianFilter across modes, sizes, hosts, or profiles.

## Direct public-call latency

- Workload: `pipeline-op.medianfilter.material-rgb-noise-1024x768`.
- Parity case: `PIL.ImageFilter.MedianFilter.nuanced.performance-material-rgb-noise-1024x768-size-3`.
- Input: RGB 1024×768; direct parity asset SHA-256
  `d649417bdb3041ab82fcc461dd5366b3dcff6427eabd89bb069f29834af6e8ed`.
- Boundary: `apply-filter;observe-filter-result`, warm release, five warmups,
  20 iterations × five samples, concurrency one. Setup is excluded; result
  observation/materialization is included.
- Pillow 12.2.0 exact parity passed for CPU, SIMD, and GPU (3/3).
- Run: `migration-benchmark-af1e66e0e776477394d148285f67f109`;
  parity gate: `migration-parity-benchmark-gate-1bc76a78fc134d7d9ecfa645e0c52c45`.
- Loaded `_core` SHA-256:
  `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`.

| Subject | Median latency | P95 latency | Speedup vs Pillow |
| --- | ---: | ---: | ---: |
| Pillow | 128.755 ms | 132.608 ms | 1.000× |
| Serial CPU | 16.211 ms | 16.751 ms | 7.942× |
| SIMD | 23.554 ms | 24.327 ms | 5.466× |
| GPU | 1.599 ms | 2.736 ms | 80.542× |

The SIMD receipt proves the vector path, 147,456 vector blocks, zero scalar
tail, terminal completion, and no fallback. SIMD is slower than serial CPU for
this ARM64 RGB row, despite clearing the 5× Pillow threshold. GPU requested and
actual backends match; its receipt proves one operation and dispatch, full
2,359,296-byte upload and readback, terminal completion, and no fallback. GPU
median latency is 14.734× lower than SIMD. Direct single-call latency is not
used as sustained-throughput evidence.

## Explicit one-operation GPU batch

The diagnostic now has a separate `median-only` graph:
`Image.frombytes("RGB", size, pixels).filter(MedianFilter(3))`. The existing
`median` graph remains the distinct two-operation invert-plus-median workload.
The selected generated-input cohort is
`pipeline-batch.median-only.rgb-1024x768` on the same workspace/core. Its 32
inputs are generated in ascending index order using
`random.Random(i + 1024*31 + 768*17 + 3).randbytes(1024*768*3)`; the SHA-256 of
their concatenation is
`ffea84d24877c6d4c717205026a88480edb939bac889c0d45ad71354836a1d5a`.

The corrected cohort in `batch-throughput-byte-exact.json` ran two warm-up
windows followed by 20 measured windows of 32 completed images. Its timer
includes fresh image and graph construction, submission, execution, GPU
synchronization, result materialization to CPU, and `tobytes()`. Input
generation, output capture, exact byte comparisons, and digest calculation are
outside timing. The harness compares every measured window's raw output buffers
to the first measured window, then compares length-prefixed output bytes
byte-for-byte across Pillow, SIMD, and queued GPU. All 32 inputs passed exact
comparison.

| Subject | Median completed images/s | Median 32-image window | P95 window |
| --- | ---: | ---: | ---: |
| Pillow | 7.693 | 4.160 s | 4.273 s |
| Serial SIMD | 42.535 | 752.315 ms | 851.171 ms |
| Queued GPU | 639.681 | 50.025 ms | 60.780 ms |

Queued GPU completed 15.039× as many images per second as SIMD. Its counters
show 704 submitted jobs and real dispatches, 1,660,944,384 upload bytes and the
same readback bytes, 176 submissions, largest group four, and zero live jobs
or retained bytes at completion. SIMD was locked; its representative receipt
proves actual SIMD and the same 147,456-block vector path with no fallback.
This establishes a 20-window completed-throughput advantage for this one
cohort; it does not establish long-duration throughput or overlapping GPU
kernels. The report's benchmark-script SHA-256 is
`f968baf84031a59838bd9e482cec8388a3471cf3f9226a1d10b3abea937884e5`.

**Prior report correction:** `batch-throughput.json` is the earlier
selector-overrun run and contains four combinations (L/RGB × 256/1024 at height
768). Its `exact_bytes_pass` field was not justified: that harness compared
SHA-256 digests rather than raw output bytes. Preserve it as a timing-only
historical artifact and do not treat its parity label as proof. The corrected
run uses `--modes RGB --sizes 1024` and contains exactly the one selected
workload.

The direct report, compressed exact parity, both report-bound execution
attestations, producer attestation, raw samples, both batch reports, and
checksums are retained in this directory. The corrected batch report binds the
benchmark script SHA; the adjacent direct execution attestation records the
loaded core identity, and no core rebuild or Rust edit occurred between the
direct and batch runs.
