# L BoxBlur current-source ARM64 rebaseline

This diagnostic rechecked the published L-mode BoxBlur GPU/SIMD latency gap on
the current dirty Apple ARM64 source. It did not change the kernel or close the
BoxBlur operation.

## Frozen scope

- Workload: `pipeline-op.boxblur.material-l-noise-1024x768-radius-1`
- Measured parity case: `PIL.ImageFilter.BoxBlur.nuanced.performance-material-l-noise-1024x768-radius-1`
- Edge guard: `PIL.ImageFilter.BoxBlur.nuanced.backend-noise-l-65x47-radius-1`
- Boundary: `observed_steps`, `apply-filter` + `observe-filter-result`
- Warm release; five warmups, 20 iterations × 5 samples, concurrency 1
- Pillow 12.2.0; CPU, SIMD, and GPU target profiles

The benchmark preflight passed the material case on all three target profiles
(3/3). The 65×47 edge guard separately passed strict parity on CPU, SIMD, and
GPU (1/1 each).

## Matched results

| Profile | Median | Speedup vs Pillow | Execution evidence |
| --- | ---: | ---: | --- |
| Pillow | 1.8680415 ms | 1.000× | Pillow 12.2.0 |
| Serial CPU | 0.7271250 ms | 2.569× | Actual CPU, terminal complete, no fallback |
| SIMD | 0.8128750 ms | 2.298× | Actual SIMD, `path=vector`, 98,304 vector blocks, no fallback |
| GPU | 0.6633955 ms | 2.816× | Actual GPU, one dispatch, no fallback |

GPU latency was 1.225× faster than SIMD. The GPU call uploaded and read back
786,432 bytes each and used a 256-byte parameter buffer. Its concurrency-one
rate was 1,507 operations/s versus 1,230 operations/s for SIMD; this is not
sustained-throughput evidence.

The published ARM64 row had reported GPU latency slower than SIMD, but lacked a
positive SIMD vector receipt and a loaded-extension identity. This matched
current-source run does not reproduce that latency gap on this Mac, so the
proposed tiled shader trial is not justified here. Keep the published cohort
separate; this local result does not update the clean matrix or establish x86
behavior. Explicit GPU-batch throughput, other modes/radii/sizes, and composed
pipelines remain open.

## Identity and artifacts

- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`
- Loaded `_core.abi3.so` SHA-256: `dedbed1799f2f61972af51503d4b965e814c76ac272c1383ac861a4c88b71768`
- Host: macOS 15.7.7, ARM64, `Lazys-MacBook-Pro.local`
- Benchmark run: `migration-benchmark-eb524eeeccb34b5b98ee5402bb7ac2cb`
- Parity gate: `migration-parity-benchmark-gate-cc07c528e53f43849ec2080a72ecd6d4`
- Benchmark report SHA-256: `9a62bcdb227ac502ec18317474f9c2e73a3a6766b30196a49da4b4f80139e747`
- Input SHA-256: `b4673500e3590f96044b5e9aec0fa2a2a2f903e91ad0b3f1fb5c7f0208b4b18c`

The parity summary is stored as `benchmark-parity.json.gz`; its uncompressed
SHA-256 is recorded in `benchmark-parity.sha256`. The report, benchmark and
parity attestations, producer attestation, raw samples, and three edge-guard
reports are retained beside this README. The local-run ledger has four new
subject rows. Kata `pillow-rs#7gmy` records the decision and full receipt
details. The focused benchmark-selection tests passed 38/38; no full suite or
coverage ran.
