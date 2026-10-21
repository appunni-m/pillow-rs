# GaussianBlur RGBA NEON MLA trial — ARM64 — 2026-10-20

## Decision

Do not keep the MLA candidate. It passed the selected parity case and retained
the actual NEON vector path, but its full-call comparison did not show a stable
gain. The candidate source was reverted; the current kernel SHA-256 is
`7649aa692c011ed1d76d90f6845db5bffc56008cd7bb6a2296359871de2927ec`.

The trial replaced the middle-block `vmulq_u32(accumulated, weight)` followed
by `vaddq_u32(..., fractional)` with
`vmlaq_u32(fractional, accumulated, weight)`. It left the Q24 bias, shift,
lane narrowing, and byte materialization unchanged. Release disassembly showed
`mla.4s` instructions in the candidate build. The focused contract
`rgba_radius_seven_neon_prefix_blocks_match_scalar_edges_and_tails` passed.

## Frozen workload and measurements

- Workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- Backend: `python-simd`; warm release; whole public workflow through terminal materialization
- Policy: 1 warmup, 3 iterations × 2 samples, concurrency 1 (unchanged)
- Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7, ARM64, CPython 3.12.13, Pillow 12.2.0
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf` (dirty worktree)

| Cohort | Pillow median | SIMD median | Pillow/SIMD | SIMD p95 | Exact parity |
| --- | ---: | ---: | ---: | ---: | --- |
| A1 control | 6.209750 ms | 5.855167 ms | 1.0606× | 6.094625 ms | pass 1/1 |
| B1 MLA | 5.957750 ms | 5.814208 ms | 1.0247× | 6.001000 ms | pass 1/1 |
| B2 MLA | 6.235959 ms | 5.746104 ms | 1.0852× | 5.934792 ms | pass 1/1 |
| A2 control | 6.352522 ms | 6.049271 ms | 1.0501× | 6.320875 ms | pass 1/1 |

Every target receipt reports actual SIMD, terminal completion, no fallback,
`GaussianBlur` vector path, 589,824 vector blocks, and zero scalar tail.
The paired same-report speedup moved in opposite directions: B1 was 3.38%
below A1, while B2 was 3.35% above A2. This does not establish a repeatable
latency or throughput improvement; the Pillow medians also moved between runs.
More decisively, the extension SHA-256 captured after A1, B1, and A2 was
identical (`fd6eb29222650095c88665ffe59d198fea8ebfea3eeff868cf090d805ecca0ce`).
The source-level intrinsic change therefore produced no distinct extension
binary; the compiler had already emitted equivalent code. The timing
differences are not attributable to a changed kernel.

## Evidence limits

The four benchmark JSON reports are retained beside this README. The full
parity JSON reports remain in the ignored `build/migration-parity/` directory;
their hashes are recorded below to avoid adding four large generated payloads
to the repository. The same extension hash was captured after A1, B1, and A2;
B2 reused the unchanged candidate build. `MIGRATION_BENCHMARK_RUNNER_ID` was
unset, so these reports are not admitted as a strict generated baseline. The
identical extension hash is sufficient to reject this intrinsic substitution
as a no-op for this build, but it does not make the results a general baseline.

Parity run IDs:

- A1: `migration-parity-benchmark-gate-6ba9d2d2744747e68c852914654b3858`
- B1: `migration-parity-benchmark-gate-78f83ecfaba840f18a2ad6bd0b42ce20`
- B2: `migration-parity-benchmark-gate-51bd605b37e147f591e5c93d51472a5e`
- A2: `migration-parity-benchmark-gate-d98a5a6e46bf45369a0fcb06ac085c3e`

Full parity JSON SHA-256 values in A1/B1/B2/A2 order are `2fc8d07900abebd6296c3e8e4d63bfff24afc1ee66a205c324cb0c34e4470f39`,
`a9620d7e1ddaf06f80e896d684ed705f13fdb6915f7f8849670f3c3626575214`,
`dcd84e9a4594e4c049fee1cf4052a029a4ce6604ad7278c414b74cd6a21f91a4`, and
`9f8afa204c73934b9fc6bc83a213e15aed7d02810151cbff852781ee281480a4`.

No full benchmark campaign, coverage, or GPU workload ran. No source change is
retained and no operation target is closed.
