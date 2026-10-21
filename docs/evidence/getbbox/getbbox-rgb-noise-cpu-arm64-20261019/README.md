# GetBBox RGB noise CPU evidence (2026-10-19)

This folder records the current-source 1024×768 RGB noise GetBBox public call. It follows the shared `docs/IMPLEMENTATION_STRUCTURE.md` evidence layout.

- Workload: `pil-image-image.getbbox.materialized.rgb-noise-1024x768`
- Parity case: `PIL.Image.Image.getbbox.nuanced.performance-rgb-noise-1024x768`
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf` (dirty tree); ARM64 macOS, runner `lazys-macbook-pro-arm64`
- Boundary: warm `call`; Pillow median 0.007979 ms, pillow-rs CPU median 0.0023545 ms (3.39× Pillow)
- Exact parity passed. Receipt confirms one `GetBBox` CPU operation, no fallback, and no mode conversion.
- Direct operation proof is retained in `direct-operation-link.json`; [validation](direct-operation-check.json) reports complete parity, backend execution, runtime equivalence, source/build binding, and verified performance admission. It reuses the exact current-source release build witness at [grayscale evidence](../../grayscale/grayscale-ycbcr-cpu-arm64-baseline-20261019/build-witness-v3.json), SHA-256 `9955b09f6c4079f832bc340083427134f84d462ddda8673ef9ebccfcc6ff264a`; direct-link validation independently binds the target extension hash to that witness.
- The per-workload generated baseline was promoted as `getbbox-rgb-noise-arm64-cpu-20261019` from benchmark artifact `2fca5a6cb4a9`; [baseline-channel.txt](baseline-channel.txt) records its strict, one-row channel. The DuckDB remains generated local state in the separate `pillow-perfdb` project.

This is a CPU baseline for one RGB noise workload only. It does not qualify the older 2026-10-15 GetBBox rank row from revision `9230f5c55dd` as current evidence, nor establish mode, size, SIMD, GPU, or cross-architecture results. The benchmark result is kept source-bound to the current dirty revision.
