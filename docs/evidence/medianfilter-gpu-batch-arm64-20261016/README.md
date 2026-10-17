# MedianFilter L explicit GPU batch cohort — 2026-10-16

This diagnostic measures Image.filter(MedianFilter(3)) on native L 1024×768
images. The batch workload uses 32 frozen deterministic noise inputs, two
warm-up windows, and 20 measured windows per backend.

Each timed window includes fresh Image.frombytes inputs and filter graphs,
execution, result materialization as CPU images, and tobytes(). GPU upload,
dispatch, synchronization while draining results, and readback are inside the
window. Source byte generation and byte-for-byte Pillow comparisons are outside.
All 704 outputs per backend, including warmups, matched Pillow 12.2.0 exactly.

| Backend | Median completed images/s | Median 32-image window | P95 window |
| --- | ---: | ---: | ---: |
| Serial SIMD | 165.432 | 193.433 ms | 196.981 ms |
| Queued GPU | 1,700.327 | 18.820 ms | 19.735 ms |

The queued GPU path completed 10.278× as many images per second as serial SIMD
on this cohort. Both target lanes loaded extension SHA-256
315a2f3278395a2a2f9ae65221ad2fcbdb2cc463558931c0acd3ca35334ce198.
SIMD's representative receipt proves MedianFilter/vector, 49,152 vector blocks,
zero scalar tail, actual SIMD, and no fallback. The explicit GPU executor
initializes and health-checks the GPU, rejects unsupported graphs, and counts
real shader dispatches. It reported 704 submitted jobs and dispatches,
553,648,128 upload bytes, 553,648,128 readback bytes, 176 grouped submissions
(largest size 4), and zero live jobs or retained host/GPU bytes at completion.

A separate source-matched direct-call run in the sibling latency evidence
measured SIMD at 6.013 ms and GPU at 1.413 ms, including single-call transfer
and materialization. That result is separate from batch throughput.

This is a dirty-tree ARM64 result for one mode, size, and kernel. It does not
close other MedianFilter modes/sizes, architectures, Parallel CPU, Pillow-SIMD,
long-duration throughput, or kernel overlap. No implementation was changed.
