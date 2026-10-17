# GaussianBlur nonuniform RGBA CPU screen

Date: 2026-10-16. Host: macOS-15.7.7-arm64-arm-64bit (arm64). This is a dirty-worktree diagnostic; it does not count as a published clean matrix result or a completed operation.

## Frozen workload

- Parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- Benchmark workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Input: RGBA 1024×768 deterministic noise, GaussianBlur radius 7.5
- Boundary: `whole_workflow` whole workflow, including `frombytes`, filter creation/call, and `tobytes` materialization
- Policy: one warmup and 3 iterations × 2 measured samples, concurrency 1
- Canonical source asset: `generated/34e223e165d2ad684903acd71f162248a31e34892bb852914e020b9be5e48b9e.bin`; SHA-256 `34e223e165d2ad684903acd71f162248a31e34892bb852914e020b9be5e48b9e`
- Input document digests are recorded in `summary.json` and both attestations.

## Result

Exact parity passed 1/1 on Pillow 12.2.0 and strict `python-cpu`. Pillow median latency was 6.445979 ms; serial CPU was 7.604063 ms. Pillow/CPU is 0.8477×, so this nonuniform case remains about 18.0% slower than Pillow. Median throughput was 155.138 ops/s for Pillow and 131.543 ops/s for CPU.

The target receipt requested and used CPU, completed the terminal materialization, and reported no fallback. The loaded `pillow_rs._core` SHA-256 is `af985e7b98977920ed0ec21ba59092b9ee7e4b7e0d4a241cab0703ec9dd874bb`. Target phase medians (setup / pipeline / terminal / total) were 0.181 / 0.013 / 7.333 / 7.610 ms. This is diagnostic only; no candidate change or SIMD/GPU claim follows. The prior large uniform RGBA fast path does not cover this noisy input.

## Artifacts

`benchmark.json`, its execution attestation and raw samples preserve the timed run; `parity.json.gz` and `parity.attestation.json` preserve the exact comparison and source/target execution identities. `summary.json` contains the compact report. `SHA256SUMS.txt` hashes each retained artifact.
