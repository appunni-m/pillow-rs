# YCbCr Grayscale SSSE3 four-block unroll screen — 2026-10-17

This experiment screened a four-block SSSE3 unroll in the x86 YCbCr luma
gather. The selected workload was
`pipeline-op.grayscale.material-ycbcr-noise-1024x768`; the measured public
boundary was `call;observe-result`, including output materialization. Runs used
the native AMD Ryzen 5 2600X runner, CPython 3.12.3, Pillow 12.2.0, warm cache,
5 warmups, 20 iterations × 5 observations, and concurrency one.

| Leg | Pillow median | CPU median | CPU / Pillow | Exact parity |
| --- | ---: | ---: | ---: | --- |
| A1 control | 1,169.077 µs | 818.637 µs | 1.428× | fresh pass, 1/1 |
| B1 unroll | 1,170.428 µs | 840.667 µs | 1.392× | fresh pass, 1/1 |
| B2 unroll | 1,160.898 µs | 819.217 µs | 1.417× | reused matching B1 pass |
| A2 control | 1,189.933 µs | 849.377 µs | 1.401× | fresh pass, 1/1 |

The two control medians average 834.007 µs and the two candidate medians average
829.942 µs, a 0.49% difference. This is below a useful optimization signal.
The four-block unroll is rejected and its source change was reverted. The
focused SSSE3 test passed for pixel counts 0, 1, 15, 16, 17, 31, 32, 33, 63,
64, and 65, including vector-block and scalar-tail accounting.

This is not a valid matched-runtime A/B/B/A result: A1, B1/B2, and A2 recorded
different target runtime identities (`09aa090a…`, `43fbfcc7…`, and
`07e266b1…`, respectively). The parity harness correctly refused to reuse A1
parity for A2 when the target identity changed, so A2 received a fresh parity
run. The candidate pair matched each other, but the two control binaries did
not; do not attribute the small median difference to the unroll. The complete
reports, parity attestations, producer records, samples, and compact evidence
archive are retained beside this README. The runner retains the full parity
JSONs; they were not copied because the attestations and reports suffice for
this no-go record.

All benchmark receipts report completed CPU execution with no fallback and
include the one-operation public call and result materialization. This local
screen does not replace the hosted CPU comparison or change the published
matrix. It gives no SIMD, GPU, throughput, or operation-completion credit.
