# GetData LA band 1 current-source ARM64 rebaseline

This current-source diagnostic rechecked the LA 32×32 GetData queue row. It changed no source and closes no operation.

## Frozen scope

- Workload: `pipeline-chain.terminal-read.getdata.la-band1-32x32`
- Exact parity case: `PIL.Image.Image.getdata.performance.material-la-band1-32x32`
- Boundary: `observed_steps`, `band-data`; returned bytes are observed before timing stops
- Warm release; five warmups, 20 iterations × 5 samples, concurrency 1
- Pillow 12.2.0; serial CPU, SIMD, and GPU target profiles

All three target profiles passed exact parity for this case (3/3). Each executed its requested backend with no fallback.

## Matched results

| Profile | Median | Speedup vs Pillow | Actual backend |
| --- | ---: | ---: | --- |
| Pillow | 10.625 µs | 1.000× | pillow, no fallback |
| Serial CPU | 5.791 µs | 1.835× | cpu, no fallback |
| SIMD | 5.625 µs | 1.889× | simd, no fallback |
| GPU | 215.209 µs | 0.049× | gpu, no fallback |

GPU latency was 38.26× slower than SIMD. This public terminal read uploaded 2048 bytes and read back 1024 bytes; sustained throughput was not measured.

The CPU and SIMD local measurements beat Pillow but remain below the 2× sequencing target. GPU is not competitive for this small materialized band read. The published operation matrix still ranks CPU GetData HSV band 2 and RGB GetChannel below Pillow; those regressions remain the CPU-first queue. This dirty-source ARM64 cohort is separate and does not replace published evidence.

## Identity and artifacts

- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty)
- Loaded extension SHA-256: `dedbed1799f2f61972af51503d4b965e814c76ac272c1383ac861a4c88b71768`
- Host: macOS-15.7.7-arm64-arm-64bit, Lazys-MacBook-Pro.local
- Benchmark run: `migration-benchmark-cf062fb3e3024fa4b2f2f2843b3de0d1`
- Parity gate: `migration-parity-benchmark-gate-6602978f226e4e18bd8b54a8ae47f2ae`
- Benchmark report SHA-256: `ed7872ae978130e0c6bc66df757fb1a079091f34654ce7c5b8aa8c17ea8cb704`
- Uncompressed parity report SHA-256: `1e54f76bf064bb3b8e82898d290028d72200f617ea97584499f1a2796b6f9ef5`

No edge guard was run in this diagnostic; exact performance-case parity is embedded in the benchmark gate. Focused benchmark-selection tests passed 38/38. No source edit, full campaign, or coverage ran.
