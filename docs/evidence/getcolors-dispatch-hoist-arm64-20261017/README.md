# GetColors signed high-cardinality CPU dispatch hoist — ARM64 (2026-10-17)

This source-matched A/B/B/A trial evaluates moving the `PillowColorCounts::Direct` versus `Sparse` match outside the per-sample I/F counting loop. The selected primary workload is `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64` (5 warmups, 20 iterations × 5 samples, 100 observations, warm cache, concurrency one). Its measured boundary is the GetColors call; output allocation and the operation result are part of the existing public-call behavior.

| Run | CPU median (µs) | Pillow median (µs) | CPU p95 (µs) | Pillow p95 (µs) |
| --- | ---: | ---: | ---: | ---: |
| A1 control | 186.208 | 175.480 | 192.916 | 180.458 |
| B1 candidate | 176.855 | 173.459 | 184.917 | 179.834 |
| B2 candidate | 182.500 | 172.604 | 341.458 | 182.167 |
| A2 source-matched control | 186.396 | 179.459 | 302.000 | 252.333 |

Control CPU medians averaged 186.302 µs; candidate medians averaged 179.677 µs (3.56% lower). Each candidate median was below both control medians. Candidate CPU remained slower than Pillow in both runs (per-run Pillow/CPU 0.981× and 0.946×). Candidate p95 was noisy: B2 reached 341.458 µs versus its Pillow p95 of 182.167 µs. Treat this as a small median improvement, not a latency-target close or tail-latency win.

The candidate parity preflight passed 4/4, covering the high-cardinality signed-run case, varied I slot order, late negative values, and no-adjacent scalar I/F. A2 ran a fresh exact-case parity after restoring the baseline source/build identity. The I/F shared-loop guard parity passed 1/1 for both A2 and B1. Both guard workloads executed actual CPU, completed their terminal read, and reported no fallback:

| Guard workload | A2 baseline CPU (µs) | B1 candidate CPU (µs) | Pillow A2 (µs) | Pillow B1 (µs) |
| --- | ---: | ---: | ---: | ---: |
| `pipeline-chain.terminal-read.getcolors.no-adjacent-scalar-i-1024x768` | 478.562 | 473.625 | 864.167 | 871.271 |
| `pipeline-chain.terminal-read.getcolors.no-adjacent-scalar-f-1024x768` | 862.792 | 853.750 | 6161.958 | 6070.542 |

The I workload improved slightly; the F workload also improved slightly. This trial has no SIMD or GPU result. No source change is credited as completing GetColors: the operation remains below the serial CPU target, and no item is counted toward the 33-item commit batch.

The candidate source SHA-256 was `dcd38ebeeba2b76b0c65ee1430b109833af158bc11a932c451716f3417cd07c8`; the baseline source SHA-256 was `b1c5fccaeea4395e02bd839a4d6e9e3deb5e8cdfab7aa75984304c0e849128ad`. Candidate runs loaded core SHA-256 `bdafdc95d73debbd2cb9c8969bccdc2b079ede28bb1c8a0e2c1c6e2d80a62360`; the A2 baseline loaded `4e594a52264cda6166da3adf2dcb2241dcd9aec4e4ca3102d0e01a45d4204cf9`.

Each run subdirectory includes benchmark/parity reports, raw samples, and available producer/execution attestations. The six new runs contribute 16 oracle/target rows to `../performance-optimization-local-runs.csv` (8 primary A/B/B/A rows and 8 I/F guard rows). No coverage, full parity campaign, commit, or push ran.
