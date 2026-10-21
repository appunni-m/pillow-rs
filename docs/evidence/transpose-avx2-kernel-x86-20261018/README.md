# x86 RGBA transpose AVX2 tile candidate — 2026-10-18

## Scope

This candidate continues the focused `PIL.Image.Image.transpose` work tracked
by Kata `pillow-rs#m2w4`. It adds an x86_64-only AVX2 8×8 transpose for opaque
four-byte samples. The kernel performs integer shuffles and unaligned loads and
stores; it does not interpret or convert RGBA, RGBa, RGBX, CMYK, I, or F sample
bytes. Incomplete bottom and right edges retain exact scalar pixel copies.

The SIMD route reports `avx2-transpose` only when runtime AVX2 detection admits
and executes the kernel. A focused telemetry contract prevents the fused
pipeline wrapper from replacing that label with its generic `native-copy`
classification.

## Verification

- x86 focused Rust test `transpose_native_tiles_preserve_pixels_at_odd_boundaries`
  passed on the native AVX2 runner. It checks all seven transpose methods and
  dimensions through 513×515 against the independent forward-coordinate
  mapping. The test derives tile counts from the returned kernel label.
- Focused telemetry test `fused_native_copy_preserves_avx2_transpose_receipt`
  passed locally.
- Main 772×768 workload parity passed for CPU and SIMD in benchmark gate
  `migration-parity-benchmark-gate-01d811c688a2494ca7f3258ed3275cff`.
- Strict x86 SIMD parity passed 1/1 for
  `PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0` in
  run `migration-parity-16183093ec0e4c7e88669c3d9c8b5b90`. Its 22 MB report
  stays on the runner; SHA-256 is
  `b482eebdbf08bbff2b4eab8a068cd53dfe41c32250c1f69bc3392eab50528095`.

## End-to-end result

Workload `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized` ran warm, with 5 warmups, 20 iterations × 5 samples, concurrency 1, for `transpose-0;transpose-1;materialize`. The report includes allocation, host-buffer boundaries, and terminal materialization.

| Subject | Median | p95 | Speedup vs Pillow |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 2.323 ms | 2.799 ms | 1.00× |
| Serial CPU | 3.148 ms | 3.816 ms | 0.74× |
| SIMD | 2.819 ms | 3.210 ms | 0.82× |

The target report proves actual SIMD execution, terminal completion, no
fallback, two fused transpose operations, one 2,371,584-byte pixel-buffer
allocation, two host-buffer boundaries totaling 4,743,168 bytes, and an
`avx2-transpose` receipt with 73,728 vector blocks. The other fused operation
receipt has zero blocks. CPU remains slower than Pillow, and SIMD remains
about 1.21× slower than Pillow. GPU latency, GPU sustained throughput,
Pillow-SIMD comparisons, and the rest of the mode/size matrix were not measured.
This is a partial candidate; the operation remains open.

The prior x86 4×4 and current 8×8 reports are not an interleaved A–B–B–A
comparison and their complete dirty source manifests differ. Treat their raw
timing difference as diagnostic only. The within-report Pillow ratios above
are valid for this measured run, but do not establish that the AVX2 kernel
caused a stable gain.

## Artifacts

The verified benchmark run is `migration-benchmark-73146c94dfeb4e0784ca3fdd01e770ff`.
The full parity output for its aligned main case remains on the runner at
25 MB; its SHA-256 is
`8d9e60d916032467c00d4992536dbb5b1938c37d2115ca114c95ce4767522180`.
The local artifacts include the compact benchmark report, report-bound
attestations, raw samples, and the small odd-parity attestation. The release
extension hash on the runner was
`bee589fe2e0f4ca1d61edcd208b28ad5757c0b4183a214c8dcc2110479158314`.

An initial candidate report recorded the correct 73,728 AVX2 blocks but the
generic `native-copy` path. The fused wrapper was overwriting the adapter's
more specific path label. The focused telemetry regression test and fresh
benchmark run verify that the path now remains `avx2-transpose`.

A later A–B–B–A trial removed the AVX2 output zero-fill but did not show a
repeatable end-to-end improvement. The candidate was reverted; see the
[2026-10-19 no-go evidence](../transpose/avx2-uninitialized-output-no-go-simd-x86-20261019/README.md).
