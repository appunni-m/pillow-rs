# Serial RGB nearest Rotate contiguous copies

This trial evaluates one ARM64 serial-CPU workload: `pipeline-op.rotate.material-rgb-nearest-noise-recovered-20261016-1024x768`, corresponding to exact case `PIL.Image.Image.rotate.nuanced.rgb-nearest-1deg-noise-recovered-20261016-1024x768`. It measures the public `rotate` call and terminal `tobytes`; source-image setup is outside the timer. The warm policy is five warmups, 20 iterations × five samples, and concurrency one. Pillow 12.2.0 and the target run on the same macOS 15.7.7 ARM64 host.

## Matched A–B–B–A result

| Run | State | Pillow median | CPU median | Pillow / CPU | Exact parity |
| --- | --- | ---: | ---: | ---: | --- |
| A1 | Control | 1.328521 ms | 1.216042 ms | 1.092× | 2/2 |
| B1 | Candidate | 1.285459 ms | 0.473813 ms | 2.713× | 2/2 |
| B2 | Candidate | 1.176563 ms | 0.467646 ms | 2.516× | Reused B1 2/2 |
| A2 | Control | 1.348438 ms | 1.349771 ms | 0.999× | 2/2 |

The two control CPU medians average 1.282907 ms; the two candidate medians average 0.470729 ms, a 2.725× matched-control improvement. Each candidate run also clears 2× against its own Pillow median. The controls show meaningful host timing variation, so the matched average and both per-run Pillow ratios are retained instead of selecting one favorable control.

The production form was rebuilt and checked after removing the temporary compile-time A/B control switch. Its confirmation run, B3, measured Pillow at 1.219625 ms and CPU at 0.473500 ms (2.576×); CPU p95 was 0.554792 ms. It passed the same two exact parity cases. The final loaded `_core.abi3.so` SHA-256 is `af985e7b98977920ed0ec21ba59092b9ee7e4b7e0d4a241cab0703ec9dd874bb`; final `geometry.rs` SHA-256 is `24645acf442d8356fe021f2baf861dd34275ac3e85ffda6b2913600e624696d9`.

## Change and correctness

The serial RGB nearest-neighbor path now computes the longest fixed-point run for which source y stays in the same row and source x advances by one pixel per output pixel. For proven runs of at least four pixels it copies the contiguous RGB byte span with `copy_from_slice`. If the arithmetic cannot prove a safe run or the run is shorter than four pixels, the existing fixed-point per-pixel sampler handles the rest. Out-of-bounds fill handling is unchanged. The helper and call site are in [`geometry.rs`](../../../pillow-rs/src/compute/pool_cpu/ops/geometry.rs#L1500).

The A1, B1, A2, and production B3 builds each passed exact CPU parity for the 1° noise case and the expanded red-fill guard (2/2). B2 reused the B1 parity report with matching identity. An additional focused fractional-angle RGB case, `PIL.Image.Image.rotate.nuanced.modes-RGB-0-arbitrary` (37.5°, 17×11), passed 1/1 on the final extension. The helper contract test passed:

```text
cargo test -p pillow-rs --release --lib rotate_contiguous_rgb_run_length_matches_fixed_point_scan -- --nocapture
1 passed; 609 filtered out
```

No genuine test defect was found. The helper exists only in the serial CPU geometry path. SIMD uses its separate affine gather kernel, and GPU dispatch uses a separate path; neither was benchmarked in this trial, so this evidence makes no SIMD or GPU performance claim.

## Reproducibility and limits

The exact one-workload manifest and benchmark input are under [`inputs/`](inputs/). The selected parity case is also preserved there. Benchmark reports, raw samples, execution and producer attestations, and producer child receipts are under [`reports/benchmark/`](reports/benchmark/). Full parity reports are gzip-compressed under [`reports/parity/`](reports/parity/); each decompressed report digest was checked against its attestation. The benchmark-side reused-parity reports were byte-identical to the corresponding preserved full parity reports; their identities are recorded in [`reports/reused-parity/index.json`](reports/reused-parity/index.json). [`SHA256SUMS.txt`](SHA256SUMS.txt) covers the dossier.

This is one serial CPU RGB workload and one small fractional-angle edge case. It does not close Rotate across modes, sizes, angles, SIMD, Parallel CPU, or GPU. No full campaign or coverage run occurred. The 33-item commit batch remains open; this result has not been committed or pushed.
