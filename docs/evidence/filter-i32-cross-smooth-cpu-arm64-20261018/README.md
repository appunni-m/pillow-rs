# I-mode cross-smooth CPU kernel evidence

This evidence covers the `pipeline-chain.convolution-i.3x3-varied-1024x768`
whole-workflow benchmark on one Apple ARM64 host and one native x86_64 host. Both
runs used the same release workload definition, warm cache, concurrency 1,
1 warmup, 3 measurement iterations, and 2 samples. The boundary includes image
setup, the public pipeline call, and terminal materialization. The benchmark
correctness gate is successful execution; exact output parity was checked
separately against Pillow using the two parity cases below.

The operation-specific kernel lives under the CPU filter family in
`pillow-rs/src/compute/pool_cpu/ops/filter/kernels/`, following the repository
family → backend → operation → role structure described in
`docs/IMPLEMENTATION_STRUCTURE.md`. The SIMD and GPU implementations were not
changed.

## Exact parity

Both focused cases passed on ARM64 after the candidate build and on native x86_64:

- `PIL.Image.Image.filter.nuanced.i-mode-cross-smooth-varied-1024x768`
- `PIL.Image.Image.filter.nuanced.i-mode-cross-smooth-int32-edges`

The compressed ARM64 candidate parity result (`.json.gz`) is stored beside this report; decompress it to run the schema validator. The x86_64
parity run passed the same two IDs before the candidate was accepted.

## Whole-workflow median latency

Lower is better. Speedup is same-run Pillow median divided by serial-CPU median.

| Host / variant | Pillow | CPU | Pillow / CPU speedup | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| ARM64 base | 2.488 ms | 4.157 ms | 0.60× | 1.390 ms | 1.961 ms |
| ARM64 typed-view candidate | 2.556 ms | 1.773 ms | 1.44× | 1.215 ms | 1.655 ms |
| x86_64 base | 10.664 ms | 17.442 ms | 0.61× | 6.767 ms | 7.237 ms |
| x86_64 typed-view candidate | 9.767 ms | 7.553 ms | 1.29× | 6.004 ms | 5.329 ms |

The candidate's CPU execution receipts identify `cpu`, report terminal completion,
and have no fallback reasons on both hosts. The measured SIMD and GPU paths also
reported their requested actual backends with no fallback. In this workload,
CPU is now faster than Pillow on both hosts. GPU latency remains above SIMD on
ARM64, so the broader GPU target is not demonstrated by this CPU change.

The ARM64 base was measured with the pre-candidate compiled extension already
present on the host; the candidate source had not yet been installed into that
extension. The candidate measurement followed a fresh local release
`make build-parity`. The x86_64 base and candidate were measured on the supplied
native runner in the same frozen workload cohort policy. Small sample counts
mean the SIMD/GPU deltas should be treated as regression guards, not precise
performance estimates.

## Rejected experiments

Two sparse-tap arithmetic candidates passed parity but still measured slower
than Pillow on x86_64 (CPU medians 12.006 ms, then 10.931 ms and 10.783 ms on a
repeat). They were replaced by the checked typed-view candidate and are retained
in the x86 evidence directory for audit; they are not accepted results.

## Artifacts

This directory contains the ARM64 base and typed-view benchmark result/raw
samples plus candidate parity. The sibling
`filter-i32-cross-smooth-cpu-x86-20261018` directory contains the corresponding
x86_64 runs, including the rejected intermediate experiments.
