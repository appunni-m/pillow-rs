# ImageDraw.text current ARM64 evidence

This directory records a focused CPU-only baseline for the two selected `ImageDraw.text` workloads on the current Apple ARM64 host. It is an evidence snapshot, not a completed optimization result.

## Frozen scope

- Benchmark run: `migration-benchmark-9bb0f199f62c486dbed4d50192d593fd`
- Workloads: `pil-imagedraw-imagedraw.text.standard`; `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768`
- Backend profile: CPU only. The operation manifest marks this public API as CPU-only; SIMD/GPU results are not applicable.
- Boundary: `call;observe-receiver`, including the public call and receiver observation/materialization.
- Sampling: 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Environment: macOS 15.7.7 ARM64, Pillow 12.2.0, CPython 3.12; target CPython 3.12.13.
- Target extension SHA-256: `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty worktree).
- Workload input SHA-256: `251a2fffbb20da7e26e850dbceb1adfb027f4972f8ad0100c72d8f02d40b305d`.
- Parity input SHA-256: `7d68f5510e985dccfbb910d1c8657228a85f4441d6bfe86aa016589cd8e7f400`.
- Font asset SHA-256: `4c44206cbb0238b4a07bac4c557a9254a2dc87941f6ebebfa1147c32dd73b29a`.
- Input asset SHA-256: `dd83fcb3d95919b5500c8a30b0760cae0f0d5d321e1b0a8f3fd84f267d976456`.

## Results and parity

| Workload | Pillow median | Target median | Diagnostic ratio |
|---|---:|---:|---:|
| `pil-imagedraw-imagedraw.text.standard` | 0.8091665 ms | 0.5035420 ms | 1.607× |
| `pil-imagedraw-imagedraw.text.rgba-context-rgb-noise-1024x768` | 0.7892295 ms | 0.5049165 ms | 1.563× |

The two benchmark parity cases passed:

- `PIL.ImageDraw.ImageDraw.text.nuanced.performance-rgb-noise-1024x768`
- `PIL.ImageDraw.ImageDraw.text.nuanced.performance-rgb-rgba-context-noise-1024x768`

Parity report: `migration-parity-benchmark-gate-b4ff3842956e406c9f60c7a4aaf7ea0d` (2/2 passed).

Focused guards passed 4/4 in report `migration-parity-256a655b3c50452188c8778f34bceb18`:

- `PIL.ImageDraw.ImageDraw.text.mode.rgba`
- `PIL.ImageDraw.ImageDraw.text.nuanced.canvas-rgb-negative-position`
- `PIL.ImageDraw.ImageDraw.text.nuanced.embedded-bgra-rgb`
- `PIL.ImageDraw.ImageDraw.text.nuanced.embedded-bgra-rgba-context-rgb`

## Execution proof status

Do **not** credit either diagnostic ratio as a verified speedup yet. The target receipts report `status=not_proven`, `actual_backend=null`, `terminal_complete=false`, and no operation count. Source tracing shows that Python `Draw.text` delegates through `PyDraw.text` and `Draw::text_with_optional_font_input` into the Rust `text_with_options_inner` and direct native RGB compositor. That identifies the implementation path by inspection, but the benchmark receipt does not currently prove the executed backend for each timed sample.

This public operation currently has a CPU-only manifest profile. No SIMD or GPU execution is claimed. The immediate follow-up is to establish a source/build-bound CPU receipt for this direct Rust path without charging instrumentation to the timed call, or to use a measured call-boundary receipt and quantify its overhead. Then repeat only these two workloads, their two parity cases, and the four listed guards. If the receipt cannot be made trustworthy, retain this snapshot as diagnostic-only evidence.

## Source identity

| File | SHA-256 |
|---|---|
| `pillow-rs/src/draw/mod.rs` | `988d46896eaa77cb8a41078bb52d56cbd44d514e51fd751ef2dde946c0b9ed3e` |
| `pillow-rs-py/src/lib.rs` | `068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24` |
| `pillow-rs-py/python/pillow_rs/imagedraw.py` | `9250d4257edb038184efa883162034120892ea02e995f3699355209a84ad40c2` |
| `pillow-rs-py/python/pillow_rs/image.py` | `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d` |

## Files

- `benchmark.json`, raw benchmark result.
- `raw-samples.json`, individual benchmark samples.
- `benchmark-attestation.json` and its `*.children/` directory, benchmark run attestation.
- `benchmark-producer-attestation.json`, producer identity receipt.
- `benchmark-producer-attestation.json.bundle.json`, producer bundle.
- `parity.json` and `parity-attestation.json`, selected benchmark parity.
- `guard-parity.json` and `guard-attestation.json`, focused regression guards.
- `SHA256SUMS.txt`, hashes for every captured evidence file except this README and the checksum file itself.
