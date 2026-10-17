# Full-width RGB Crop current-source ARM64 screen

Date: 2026-10-16. Host: macOS-15.7.7-arm64-arm-64bit (arm64). This is dirty-worktree local evidence, not a clean published matrix result.

## Frozen pair

- Workload: `pil-image-image.crop.materialized.full-width-rgb-1024x768`
- Parity case: `PIL.Image.Image.crop.nuanced.performance-material-rgb-noise-1024x768-full-width-box-0-65-1024-704`
- Input: deterministic RGB noise 1024×768; crop box `[0, 65, 1024, 704]`; output materialization is 1,963,008 bytes
- Boundary: `observed_steps` covering `crop` and `tobytes`; image setup is outside timing
- Policy: 5 warmups, 20 iterations × 5 measured samples, concurrency 1
- Asset SHA-256: `dd83fcb3d95919b5500c8a30b0760cae0f0d5d321e1b0a8f3fd84f267d976456`
- Input document digests are in `summary.json` and execution attestations.

## Result

Exact parity passed 1/1. Pillow 12.2.0 median latency was 0.533271 ms; serial CPU was 0.205437 ms, or 2.5958× Pillow. Median throughput was 1875.224 ops/s for Pillow and 4867.661 ops/s for CPU. The target receipt confirms actual CPU, completed terminal materialization, and no fallback. The loaded `pillow_rs._core` SHA-256 is `af985e7b98977920ed0ec21ba59092b9ee7e4b7e0d4a241cab0703ec9dd874bb`.

This workload already exceeds the 2× CPU sequencing checkpoint on this ARM64 host. No implementation change was made. SIMD, Parallel CPU, GPU, other modes/sizes/boxes, and the full Crop operation matrix remain unmeasured. No coverage ran.

## Artifacts

The benchmark report, raw samples, producer/build attestations and child receipts, parity attestation and compressed parity report, plus compact `summary.json`, preserve this exact pair. `SHA256SUMS.txt` covers the retained artifacts.
