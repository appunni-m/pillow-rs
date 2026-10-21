# GetChannel RGBA→RGB SSSE3 path — native x86_64

This experiment targets the composed `RGBA → RGB → GetChannel(all bands)`
workflow, which was slower than Pillow on the current x86 SIMD baseline. It
adds a direct 16-pixel SSSE3 RGBA-to-RGB kernel and places the Convert
orchestrator in the repository's standard family adapter tree.

## Frozen scope

- Workloads:
  - `pipeline-chain.getchannel.seeded-corners-1024x768-all-bands`
  - `pipeline-chain.getchannel.seeded-corners-16x16-all-bands` (small guard)
- Exact parity cases:
  - `PIL.Image.Image.getchannel.nuanced.pipeline-seeded-corners-1024x768-all-bands`
  - `PIL.Image.Image.getchannel.nuanced.pipeline-seeded-corners-16x16-all-bands`
- The workflow seeds four RGBA corner pixels, converts to RGB, extracts all
  three channels, and observes each result as bytes.
- Boundary: complete public workflow, including allocations and result
  materialization; warm release build, 5 warmups, 20 iterations × 5 samples,
  concurrency 1.
- Host: native x86_64 Linux, CPython 3.12.3, Pillow 12.2.0. The benchmark
  attestation contains the full runner and source identities.
- All reports used source revision
  `418d5f0cde51ca64a97cbd007ea731da72d68abf`; the worktree was dirty.

## Matched A–B–B–A results

Each ratio is Pillow median / SIMD median from the same report. A is the
baseline without the RGBA→RGB SSSE3 route; B uses the candidate route.

| Leg | Benchmark run | Pillow ms | SIMD ms | Pillow / SIMD | 16×16 Pillow / SIMD |
| --- | --- | ---: | ---: | ---: | ---: |
| A1 control before | `migration-benchmark-aba2b174c844485080b2b37a1b6c15fb` | 7.860424 | 10.551509 | 0.745× | 1.016× |
| B1 candidate | `migration-benchmark-0ab70398ce4c46779fef80ed21da01c2` | 7.746986 | 6.034997 | 1.284× | 1.007× |
| B2 candidate repeat | `migration-benchmark-3712f8816b47436aad09c81863ba30be` | 7.544739 | 5.787406 | 1.304× | 1.008× |
| A2 control after | `migration-benchmark-a766072ee9fe402cb362c8068a3bc192` | 7.640827 | 10.991992 | 0.695× | 1.003× |

The candidate moved this composed x86 SIMD workflow from slower than Pillow to
about 1.28–1.30× Pillow. The small guard remained near parity. This is a
repeatable partial gain, below the 2× optimization checkpoint and the 5× SIMD
completion target; GetChannel remains open.

## Family-layout revalidation

Moving the full `simd_convert` orchestrator into `convert.rs` while registering
that module at the top of the large facade measured 1.250× and 1.241× Pillow
in B3/B4. The final arrangement keeps the family adapter and focused kernels,
but registers and re-exports the family at the facade position formerly
occupied by `simd_convert`; B5 returned to 1.299× Pillow:

| Layout leg | Benchmark run | Pillow ms | SIMD ms | Pillow / SIMD |
| --- | --- | ---: | ---: | ---: |
| B3 family declaration at facade top | `migration-benchmark-0e4d687871c24e18bf76fa86ae96787c` | 7.777517 | 6.220480 | 1.250× |
| B4 repeated top declaration | `migration-benchmark-ebe37d825f184d92b2c3955d5a01f457` | 7.708296 | 6.212290 | 1.241× |
| B5 final family layout/order | `migration-benchmark-0586209e0e944bc39b0f867553d69de8` | 7.897316 | 6.077509 | 1.299× |

B3/B4 are retained as measured layout trials, not accepted final performance.
The B5 arrangement follows the stable family tree while preserving the
original dispatch position's measured performance. Keep registration order
under the full-call performance gate when changing code layout in this crate.

The final source layout is:

```text
pillow-rs/src/compute/pool_simd/ops/adapters.rs
  # Convert family declaration and re-export at its existing dispatch position
pillow-rs/src/compute/pool_simd/ops/adapters/convert.rs
pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/mod.rs
pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_neon.rs
pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_ssse3.rs
```

The family adapter owns validation, mode/backend selection, execution receipt,
and output materialization. The SSSE3 kernel reads sixteen RGBA pixels per
iteration (four 16-byte loads), shuffles the RGB lanes, and writes 48 bytes in
three stores; only a final partial block uses the scalar tail. The existing
NEON path remains a separate 16-pixel `vld4q_u8`/`vst3q_u8` kernel.

## Execution and parity proof

The selected GetChannel benchmark reports actual SIMD execution, no fallback,
and a final `ExtractBand` receipt (`ssse3-gather`, 49,152 vector blocks, zero
tail on 1024×768). Its operation receipt does not include the preceding lazy
Convert receipt. The retained public Convert probe
[`getchannel-rgba-rgb-ssse3-B5-convert-dispatch.json`](getchannel-rgba-rgb-ssse3-B5-convert-dispatch.json)
uses the same 1024×768 seeded-corner source and the exact B5 extension SHA;
it proves actual SIMD, no fallback, `ssse3-rgba-to-rgb`, 49,152 vector blocks,
zero tail, and a 2,359,296-byte RGB result. Do not read the final ExtractBand
receipt as a complete per-operation trace of a composed workflow.

All seven reports passed the selected two-case exact parity gate. B2 reused
B1's report-bound proof; B4 reused B3's. The remaining reports have fresh
parity artifacts. B1/B2 shared extension SHA-256
`a4748c07cd951db39ea5088a84f48ee45476c11a32120aeb6dfd318f6dd4545d`;
B3/B4 shared
`7f270089a9bee576e095a55d53bca5d5882ae26d11f38d2097fd0c12a7bd4b69`; B5
used
`cc6ab230ec82914e0435cf5ddd46a941cbf44c6d99c4103e2c4de02454a01d1a`.
Every target row records `actual_backend=simd` and an empty fallback map.

The runner's Pillow-SIMD environment did not provide an installed
Pillow-SIMD oracle, so this work makes no Pillow-SIMD speed claim. CPU, other
modes/sizes, ARM64, GPU latency, and sustained GPU throughput were not changed
or demonstrated here. Next, keep GetChannel in the under-2× queue and profile
its repeated ExtractBand/materialization costs against the proven Convert
candidate.

The directory retains all A1/B1/B2/A2 and B3/B4/B5 benchmark reports, sample
data, parity artifacts, attestations, producer receipts, and child receipts.
