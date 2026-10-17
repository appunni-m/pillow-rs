# GetData HSV band 2 current-source ARM64 rebaseline

This current-source diagnostic rechecked the published HSV 1024×768 GetData CPU miss. It changed no source and does not retire the published cohort.

## Frozen scope

- Workload: `pipeline-chain.terminal-read.getdata.hsv-band2-1024x768`
- Exact parity case: `PIL.Image.Image.getdata.performance.material-hsv-band2-1024x768`
- Boundary: `observed_steps`, `band-data`; returned bytes observed before timing stops
- Warm release; five warmups, 20 iterations × 5 samples, concurrency 1
- Pillow 12.2.0; serial CPU, SIMD, and GPU target profiles

All three target profiles passed the exact performance parity case (3/3). Receipts show each requested backend executed without fallback.

## Matched results

| Profile | Median | Speedup vs Pillow | Actual backend |
| --- | ---: | ---: | --- |
| Pillow | 5.942083 ms | 1.000× | pillow, no fallback |
| Serial CPU | 2.553708 ms | 2.327× | cpu, no fallback |
| SIMD | 2.498730 ms | 2.378× | simd, no fallback |
| GPU | 3.532354 ms | 1.682× | gpu, no fallback |

On this dirty ARM64 source, CPU was 2.327× Pillow; SIMD was 2.378×; GPU latency was 1.41× slower than SIMD. GPU uploaded 2359296 bytes and read back 786432 bytes. Sustained throughput was not measured.

The published GitHub Pages row remains a CPU slowdown (0.765807× Pillow on macOS ARM64), but this current dirty-source run does not reproduce it. The current route directly gathers every third HSV byte into the final 786,432-byte output buffer; a prior scalar-loop unroll probe regressed. Keep the published miss open pending a clean, source/build/input-matched cohort. This local diagnostic does not replace the published matrix.

## Identity and artifacts

- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty)
- Loaded extension SHA-256: `dedbed1799f2f61972af51503d4b965e814c76ac272c1383ac861a4c88b71768`
- Host: macOS-15.7.7-arm64-arm-64bit, Lazys-MacBook-Pro.local
- Benchmark run: `migration-benchmark-5fbb31f64de64950a928921f8e78c545`
- Parity gate: `migration-parity-benchmark-gate-7d638ad06d724a1ba21891838a4fd0ce`
- Benchmark report SHA-256: `7c4e52eada24a5433086b053e54d1f89d68bf4642d360ffc388fa403af661bd5`
- Uncompressed parity report SHA-256: `65cbe2ede64c1c60ad3dd7ef142fbd6a34eaac085a40e6fe41c71e1d954f9405`

Only the exact performance parity case was run; no additional edge guard was run in this diagnostic. Focused benchmark-selection tests passed 38/38. No source edit, full suite, or coverage ran.
