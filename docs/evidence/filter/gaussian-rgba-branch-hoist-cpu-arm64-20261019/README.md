# GaussianBlur RGBA fractional-branch hoist — serial CPU A–B–B–A

**Disposition:** retained as a measured partial improvement. The candidate is about 28.3% faster than the restored control for this one RGBA workload and now beats Pillow on it. The GaussianBlur operation remains open: this does not establish all modes, sizes, edge cases, SIMD, GPU, or the next 2× CPU checkpoint.

## Frozen workload and method

- Workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Exact parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- Input: deterministic noise, RGBA, 1024×768; `GaussianBlur(radius=7.5)`
- Boundary: warm `whole_workflow`, including construction, filter call, and terminal materialization via `tobytes`
- Measurement policy: 1 warmup, 3 iterations × 2 samples, concurrency 1; six samples per subject, unchanged from the manifest
- Backend selection: CPU only (`MIGRATION_BENCHMARK_BACKENDS=cpu`); no SIMD, Parallel CPU, or GPU measurements in this trial
- Host: Apple M3 Pro, macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0
- Build: release/default features via `make build-parity`; source revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty tree
- Changed path: `pillow-rs/src/compute/pool_cpu/ops/filter.rs`, within the existing CPU filter family owner; no new source file or dispatch layer was added

The candidate makes the fractional-weight decision once per line and specializes the per-output helper with a const boolean. It preserves the same edge regions, pixel order, accumulator updates, and fixed-point arithmetic; it does not unroll the four-channel pixel loop. This is a focused family implementation change and follows the repository layout standard at `docs/IMPLEMENTATION_STRUCTURE.md`.

## A–B–B–A results

| Run | Source | Pillow median | CPU median | CPU / Pillow | Exact parity |
| --- | --- | ---: | ---: | ---: | --- |
| A1 | control | 6.371188 ms | 7.451645 ms | 1.170× | pass 1/1 (`migration-parity-benchmark-gate-95a3de2624834e1bbf25648b4488b1b2`) |
| B1 | candidate | 6.449896 ms | 5.468063 ms | 0.848× | pass 1/1 (`migration-parity-benchmark-gate-013cc95859524759a76dbcf81ab891bd`) |
| B2 | candidate | 6.442291 ms | 5.332437 ms | 0.828× | pass 1/1 (`migration-parity-benchmark-gate-013cc95859524759a76dbcf81ab891bd`) |
| A2 | restored control | 6.659125 ms | 7.610750 ms | 1.143× | pass 1/1 (`migration-parity-benchmark-gate-e35e7e35e750456888feab173088f91f`) |

Control CPU mean: 7.531 ms. Candidate CPU mean: 5.400 ms, a 28.3% reduction. Candidate samples range from 5.112 to 5.715 ms; restored-control samples range from 7.191 to 7.866 ms. The two control binaries have the same SHA-256, the two candidate binaries have the same SHA-256, and the rebuilt final extension matches the candidate binary exactly.

Each run used actual backend `cpu`, completed terminal materialization, and recorded no fallback. B1 ran the fresh parity preflight; B2 reused B1 only after the runner accepted the matching bound parity evidence. A1 and A2 each ran fresh parity. The selected case passed in all four reported runs.

Resource receipts were unchanged between control and candidate: two host buffers, 6,291,456 allocated bytes, no GPU transfers or dispatches. The measurement is end-to-end and includes those allocations and final materialization.

## Focused regression guards

After the A–B–B–A sequence, the same rebuilt candidate binary passed four additional CPU parity cases (4/4):

- `PIL.Image.Image.filter.nuanced.gaussian-blur-zero-radius` — L mode and zero radius.
- `PIL.Image.Image.filter.nuanced.pa-putpalette-gaussian` — PA palette input and integer radius.
- `pipeline-composition.filter-invert-mirror` — RGB, fractional radius 1.5, followed by composition.
- `PIL.Image.Image.filter.nuanced.performance-gaussian-rgba-sparse-512x512-radius-2` — RGBA, integer radius, and sparse edge content.

The focused core test command `cargo test --locked --manifest-path pillow-rs/Cargo.toml --lib gaussian_blur_row_fusion_tests` passed 12/12. Those tests exercise line widths 1–4, short lines, fractional and integral weights, and radii larger than the line, against direct fixed-point references or full-frame passes. No coverage was run. The selected public parity report is stored gzip-compressed in `parity-guards/`; its raw SHA-256 is `e56bb75e33a2e2a6631b1a54e9150a665e45fa5c0c6f85931c74de5adf243e26`.

## Identity and omitted parity payloads

- Control `filter.rs` SHA-256: `67cc33f5c8355372cb8a8baedeaa3f3f4e8cace0c9a2c55c4efe9511cf997fc8` (the pre-candidate file, reconstructed by replacing only the experimental block with its unchanged HEAD version).
- Candidate `filter.rs` SHA-256: `0452fd48c8f486bcb97666abfe178cc38e2504192ec8a890706b52c22d4c0110`.
- Control `_core.abi3.so` SHA-256 (A1/A2): `3cf57846971538066774337970dd828f36a9841f2c8376ae9cdfa02531778ca9`.
- Candidate `_core.abi3.so` SHA-256 (B1/B2 and final rebuilt binary): `b80535f7292b0f60d552b8788f359969c1100fc15ec3f67cc0e2cb9e44777361`.
- A1 parity report SHA-256: `0bd72404760a766b1bec9a671b2378a72b27175163df222ec7e7e19c3a1d61c6`.
- B1 parity report SHA-256: `b476027f1268c2dcffb4cc3f91d6c31e1794006b72940646e46227a41b533158`.
- A2 parity report SHA-256: `98ae1282e5477a6a6a56b28960dc8f2065004de567497cd32bfda8e8cdf36fb9`.

The three full parity JSON reports are intentionally not duplicated here because each is about 16 MB. Their report-bound attestations, benchmark reports, ordered samples, benchmark attestations, producer bundles, and child receipts are retained in each run directory. The full reports remain under ignored `build/migration-parity/gaussian-blur-branch-hoist-abba-20261019/`.

This result clears the CPU ≤ Pillow gate for this single RGBA noise workload only. It does not satisfy the later 2× sequencing checkpoint, nor demonstrate SIMD/GPU performance. Keep `pillow-rs#vc6w` open for the remaining GaussianBlur matrix.

No coverage, commit, or push ran. The shared commit/push batch remains 0/33.
