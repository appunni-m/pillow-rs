# RGB Thumbnail SIMD PutPixel-prefix fusion — 2026-10-18

Candidate: defer a fusible RGB `PutPixel*` prefix into the same batch as Thumbnail. On sparse RGB input, reduce with the existing 2×2 reducer, apply ordered writes to the affected reduced blocks, then keep the native SIMD resize. Dense input returns to the ordinary sequence. This preserves the specialized resize path and avoids materializing the full source before the downscale.

The selected workload is `pipeline-op.thumbnail.material-rgb-1024x768`, a 1024×768 RGB whole-workflow case including setup, the pending pixel write, Thumbnail, and final materialization. Each run used 5 warmups, 20 iterations × 5 samples, concurrency one. Candidate parity was exact on CPU and SIMD on both hosts (2/2 comparisons each).

| Host | Pillow | CPU | SIMD | CPU/Pillow | SIMD/Pillow | Backend proof |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Apple ARM64 | 1.404 ms | 0.262 ms | 0.417 ms | 5.37× | 3.36× | CPU/SIMD actual, terminal complete, no fallback; SIMD 96 vector blocks |
| Ryzen 5 2600X x86_64 | 2.073 ms | 0.267 ms | 0.484 ms | 7.77× | 4.29× | CPU/SIMD actual, terminal complete, no fallback; SIMD 96 vector blocks |

## Limits

- This is a candidate screen, not an A–B–B–A causal comparison. Keep the change in progress and do not treat it as operation completion.
- The x86 run used pinned Pillow 12.1.1 but could not measure Pillow-SIMD: this machine lacks the JPEG development headers required by the repo workflow, and non-interactive sudo is unavailable. No Pillow-SIMD ratio is claimed.
- The published Pillow-SIMD row is on a different x86 runner, so it is only an investigation reference. Compare it only after a matched source cohort runs successfully.
- The local run used Pillow 12.2.0 on ARM64; the x86 run used Pillow 12.1.1. Treat the two host cohorts separately.
- Focused Rust tests passed on ARM64, including ordered duplicate and edge pixel writes; strict public benchmark parity passed on x86 CPU and SIMD. No full campaign or coverage ran.

The compact x86 benchmark result and raw samples are stored alongside this note. Full materialized parity JSON and build artifacts remain on the runner.
