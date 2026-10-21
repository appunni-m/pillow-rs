# YCbCr Grayscale current-worktree CPU/SIMD rebaseline — 2026-10-19

Selected only the 1024×768 materialized YCbCr workload
`pipeline-op.grayscale.material-ycbcr-noise-1024x768` and exact parity case
`PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`.

- Host: macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0.
- Target: dirty revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`, release,
  default features. Both profiles loaded `_core` SHA-256
  `400c2ccc2b3644d4a87c9c604b112189cee53aa82b02426491cd77d623bb33e3`.
- Boundary: warm public call and result observation, 5 warmups and 20 iterations
  × 5 samples, concurrency one. Exact parity passed 1/1 for each target profile.
- Pillow median: 0.1186455 ms; CPU median: 0.079291 ms (1.496× Pillow), actual
  CPU, one mode conversion, terminal complete, no fallback.
- SIMD median: 0.0501455 ms (2.366× Pillow), actual SIMD path
  `neon-deinterleave`, 49,152 vector blocks, zero scalar tail, one mode
  conversion, terminal complete, no fallback.
- Both target profiles recorded one 786,432-byte host allocation and no
  full-frame copies.

This current dirty ARM64 result does not reproduce the older published CPU
miss. The SIMD result remains below the 5× Pillow target. This does not compare
against Pillow-SIMD on the same host and does not close the operation across
modes, architectures, GPU, or Parallel CPU.

Run: `migration-benchmark-685e4d78f731452ab7e1d6c2dd5bf5d8`.
Parity: `migration-parity-benchmark-gate-8c3c1ae9b8ce4ffbbca71c46b88b7c36`.
Benchmark report SHA-256: `e67f69dc3dc7385b295ef928bd4e9717d7c8eb590b018e1f073a610fcf9febe8`.
Parity report SHA-256: `4171772f0ab295d016cff6bd2dbc85c0c8d9040023ee2e6e8f79969415828583`.
