# GetChannel RGB current-source ARM64 CPU baseline

This focused rebaseline checked whether the scalar ARM64 CPU route has the same slowdown as the published x86 row. It does not; the host-specific cohorts remain separate.

## Frozen scope

- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`
- Exact parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`
- RGB 1024×768; observed boundary `call + observe-result`
- Release build, warm cache, 5 warmups, 20 iterations × 5 samples, concurrency 1
- Pillow 12.2.0; only the serial CPU target was selected

The exact parity case passed (1/1). The benchmark's CPU receipt names actual backend `cpu`, terminal completion, and no fallback. Kernel telemetry reports zero SIMD vector blocks.

## Matched latency

| Subject | Median | Speedup vs Pillow | Actual backend |
| --- | ---: | ---: | --- |
| Pillow | 0.170250 ms | 1.000× | pillow |
| Serial CPU | 0.047000 ms | 3.622× | cpu |

This confirms the ARM64 CPU path is already above the 2× triage threshold for this workload. It is not evidence for the published x86 result: the latest clean x86 row remains 0.760568× Pillow and needs a source/build-matched x86 run. No optimization is justified for this ARM64 CPU route from these measurements. The operation remains open for x86, other modes, sizes, and the other backends.

## Identity and artifacts

- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty)
- Loaded extension SHA-256: `dedbed1799f2f61972af51503d4b965e814c76ac272c1383ac861a4c88b71768`
- Host: Lazys-MacBook-Pro.local, macOS-15.7.7-arm64-arm-64bit
- Benchmark run: `migration-benchmark-143026df8c204449a4a484a4e5740ada`
- Parity gate: `migration-parity-benchmark-gate-337337798cb349b9864883e34b4b58a7`
- Benchmark report SHA-256: `719ff633c4dcba77a360ef975050c5812db540ccf271d4327b49f0d4b11d7383`
- Uncompressed parity report SHA-256: `53377e9e3f4154f8b7f175da9df389b97d3127bb7475910026b49b02a3307596`

Focused benchmark-selection tests passed 38/38. No full suite, additional parity case, source edit, or coverage ran. This is a baseline, not a completed operation. The 33-operation commit/push batch is unchanged.
