# L BoxBlur SIMD vertical-loop unroll — rejected

This focused A-B-B-A trial unrolled four 16-pixel vectors in the vertical
three-row-ring loop for native-L, radius-one, one-pass BoxBlur. The source was
restored after measurement; `box_blur_luma.rs` is back at SHA-256
`8f7c4050757e304f6bc3872d53a16092c6bfdd1bfcf3cadcf2e4a87a2420de6f`.

The selected workload was
`pipeline-op.boxblur.material-l-noise-1024x768-radius-1`, with exact parity
case `PIL.ImageFilter.BoxBlur.nuanced.performance-material-l-noise-1024x768-radius-1`.
The edge parity guard was
`PIL.ImageFilter.BoxBlur.nuanced.backend-noise-l-65x47-radius-1`. The public
timing boundary observed `apply-filter` and `observe-filter-result`; each run
used warm 5 warmups, 20 iterations × 5 samples, and concurrency one. Pillow
setup stayed outside the timed boundary. The target was built with default
features (`parallel_feature_enabled=false`).

| Run | Pillow median | SIMD median | Pillow / SIMD | SIMD p95 |
| --- | ---: | ---: | ---: | ---: |
| A1 control | 1.929792 ms | 0.647646 ms | 2.980× | 1.305834 ms |
| B1 candidate | 1.873438 ms | 0.656605 ms | 2.853× | 0.748250 ms |
| B2 candidate | 1.885584 ms | 0.651625 ms | 2.894× | 0.904791 ms |
| A2 control | 1.883958 ms | 0.654584 ms | 2.878× | 1.169708 ms |

The candidate SIMD median averaged 0.654115 ms, versus 0.651115 ms for the
controls (0.46% slower). Its mean per-run Pillow/SIMD ratio was 2.873× versus
2.929× for controls. The measured median difference is within host sample
variation; it does not show a repeatable improvement. Candidate p95 was lower,
but Pillow and target tail samples varied substantially between cohorts, so
that does not establish a kernel effect. The four-block unroll is rejected.

All four material parity gates passed. The candidate's 65×47 edge parity
passed, as did the existing focused Rust test
`luma_box_blur_radius_one_matches_cpu_at_edges_and_vector_tails` (10
dimensions). Candidate receipts recorded actual SIMD, one BoxBlur operation,
98,304 vector blocks, no scalar tail, terminal completion, and no fallback.
The A1/A2 loaded core SHA-256 is
`e0e3e5373c0bb2c2779f2fc6b38631ce35b951528dac852808c6d3cf0cc3f847`; B1/B2
is `9e0b5240207d2c9ef4f87df10e4fd1a801f05a280d5efdbd44f7e1367d8cb801`.

The first focused Rust test invocation did not execute because
`BOX_BLUR_BIAS` was imported but unused in the CPU Gaussian test module; that
module already refers to it as `super::BOX_BLUR_BIAS`. Removing only the
redundant test import restored compilation, and the selected SIMD edge/tail
test then passed 1/1. No assertions or thresholds changed. This compile defect
is recorded in the reusable performance-parity skill.

No serial CPU, Parallel CPU, GPU, or other SIMD workload was changed or
measured. This does not complete BoxBlur: the current local SIMD result remains
below the final 5× Pillow goal. No coverage, commit, or push ran; the commit
batch remains 0/33.
