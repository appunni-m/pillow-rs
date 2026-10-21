# GaussianBlur RGBA AVX2 transpose A–B–B–A — x86_64 — 2026-10-20

## Frozen workload and result

- Workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Exact parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- Context: RGBA 1024×768, release build, warm cache, one warmup, 3 iterations × 2 samples, concurrency 1
- Boundary: whole public workflow through terminal materialization
- Oracle: Pillow 12.2.0; CPython 3.12.3; Linux x86_64, kernel 7.0.0-38
- Git revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf` (dirty worktree)

| Run | Role | Target median | Target p95 | Same-run Pillow median | Pillow / target | SIMD receipt |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| A1 | baseline control | 21.299 ms | 22.095 ms | 37.004 ms | 1.737× | `vector`, 589,824 blocks |
| B1 | candidate | 17.974 ms | 18.120 ms | 37.100 ms | 2.064× | `avx2-transpose`, 786,432 blocks |
| B2 | candidate | 17.454 ms | 17.610 ms | 36.507 ms | 2.092× | `avx2-transpose`, 786,432 blocks |
| A2 | baseline control | 20.987 ms | 22.222 ms | 36.636 ms | 1.746× | `vector`, 589,824 blocks |

Across the two run medians, the candidate reduced target latency from 21.143 ms to 17.714 ms (16.2%). Mean of the two p95 values fell from 22.158 ms to 17.865 ms (19.4%). Pillow is paired within each report, so these ratios use the Pillow row measured in the same run.

The candidate passed exact parity in B1 (1/1, no diffs); B2 reused that report-bound proof for the unchanged candidate binary. A1 and A2 also passed 1/1. All three parity reports produced identical 3,145,728-byte RGBA output with SHA-256 `29a097670d78bb2c9ade5dec7f89b3d246338819f8f5c9dd6daec693ce33f630`. The selected source, target, and report hashes are in [parity-selected-summary.json](parity-selected-summary.json); B1's full parity report is retained as [B1/parity-result.json.gz](B1/parity-result.json.gz).

The integrated tail guard in [odd-size-parity-probe.py](odd-size-parity-probe.py) compares Pillow and the public target call on deterministic 65×47 RGBA noise at radius 7.5. It passed exact parity with output SHA-256 `c9c5e4af816457d8499d552509deb636b0a2ac673b962b7a9605699bc6cb3c66`; the target receipt was actual `simd`, no fallback, path `avx2-transpose`, and `scalar_tail_count=3,960`. The direct API receipt leaves `requested_backend` null, so the probe selects SIMD through `pillow_rs.enable_backend` and uses the actual backend and kernel path as execution proof. This is a parity guard, not a timing result. Its output is in [odd-size-parity-result.json](odd-size-parity-result.json).

The candidate receipt proves requested=actual `simd`, `terminal_complete=true`, no fallback, path `avx2-transpose`, and zero scalar tail. The path is recorded only after both full-image transposes succeed through the existing AVX2 RGBA transpose adapter. The benchmark includes setup, filter execution, output materialization, and synchronization at the public call boundary. Per-operation latency is above 2× Pillow on this x86 workload, but remains below the repository's 5× SIMD target. This does not establish a Pillow-SIMD comparison; no matched Pillow-SIMD workload exists for this case.

## Candidate and source identity

The change dispatches the two large RGBA GaussianBlur transposes to the existing AVX2 RGBA transpose helper only when the already-admitted Gaussian RGBA SIMD kernel is active. The ARM64 tiled transpose, serial CPU, Parallel CPU, and GPU paths are not selected by this x86-only branch. ARM64 was not remeasured after this edit; its separate current-source SIMD rebaseline is 1.039× Pillow.

The A–B–B–A used the single existing checkout at `/home/appunni/work/pillow-rs-current-x86-20261020`; no project checkout or target directory was copied to that host, and no remote files were deleted. A1/A2 used adapter-file SHA-256 `918a112e0a824c2f40f6ba8f17fc2def0a29d02caa47f416b993c4b59a777465` and extension SHA-256 `61ff5b53119feff37667c4130ede525b0318268da3362c2bb8eb8573817f6fb4`. B1/B2 used adapter-file SHA-256 `dcb1a80c926088b4204231fa77e68c9c4d25ae70d81cba08f205ca5a00e5681b` and extension SHA-256 `83af13ac8fda38a21ae69a57e7837a89f9745cfb57116d2da29dadd5b41fd92e`.

The full local and remote dirty trees were not identical, but the selected operation code was source-matched: `simd_blur_transpose_rows` hashes to `68310d9fceb3e1cf0fccc83de3e557784faaf3d3afa73cd7f1c1e5cfcc6b5569`, and `simd_pil_box_blur_xy` hashes to `0a6c77cfac72e5cf5ae19d3f329f4876892d49ace1b28bcc10357cbde306adc9` in both trees. The A/B source and extension hashes were also checked on the runner before each phase.

## Retained artifacts and validation

Each A/B directory contains its selected benchmark report, six raw samples, execution attestation, and producer attestation. B1 additionally retains the compressed exact-parity result and its attestations. A1/A2 full parity payloads remain on the runner; this evidence keeps their report hashes and compact source/target output digests instead of copying those large payloads. B2's proof points to the unchanged B1 parity report.

The candidate release wheel built successfully with the pinned Maturin command. `validate_migration_parity_result.py` accepted the selected benchmark and parity reports. Only this one workload and its linked parity case ran; no full benchmark, full parity campaign, or coverage run was performed.

The x86 candidate crosses the 2× Pillow screening threshold, so work can move to the next slower case. GaussianBlur is not complete: the ARM64 SIMD row remains near Pillow, the serial CPU and GPU profiles were not changed here, and the 5× SIMD target remains open.
