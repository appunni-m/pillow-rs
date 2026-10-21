# Uniform RGB Filter5x5 SIMD identity scan

Status: measured candidate; the Filter5x5 operation remains open.

## Scope and source

This x86_64 experiment admits one narrow case: a large native RGB image, an
all-ones 5×5 kernel, scale 25, and offset 0. A focused SIMD scan first samples
the image, then proves the entire buffer is uniform before returning its
identity result. Other inputs continue through the existing Filter5x5 path.

The candidate lives in the canonical family layout:

```text
pillow-rs/src/compute/pool_simd/ops/adapters/filter.rs
pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/uniform_rgb.rs
```

The family adapter owns exact admission and receipt accounting; the kernel
owns the RGB scan. This follows `docs/IMPLEMENTATION_STRUCTURE.md`.

Runner: AMD Ryzen 5 2600X, Linux x86_64, CPython 3.12.3, Pillow 12.2.0.
The checkout was dirty at `418d5f0cde51ca64a97cbd007ea731da72d68abf`; the
experiment was built on the native runner's existing checkout. No target tree,
build directory, or binary was copied between machines.

## Matched end-to-end measurements

The primary workload is
`pipeline-chain.convolution.native.rgb-5x5-pad.1024x768`: warm
`whole_workflow`, RGB 1024×768, Filter5x5 → Pad, one warmup, three iterations ×
two samples, concurrency one. The timer includes the full public workflow,
including output cloning and terminal materialization.

| Cohort | Pillow median | CPU median | SIMD median | SIMD receipt |
| --- | ---: | ---: | ---: | --- |
| A1 before candidate | 29.023 ms | 5.696 ms | 51.634 ms | Filter5x5: vector, 293,376 blocks; Pad: vector, 78 blocks, 12 scalar-tail pixels |
| B1 candidate | 29.442 ms | 5.852 ms | 2.328 ms | Filter5x5: vector, 147,456 blocks; Pad: vector, 78 blocks, 12 scalar-tail pixels |
| B2 candidate | 29.432 ms | 5.869 ms | 2.315 ms | Filter5x5: vector, 147,456 blocks; Pad: vector, 78 blocks, 12 scalar-tail pixels |

Both candidate SIMD runs report the requested and actual backend as SIMD,
terminal completion, and no fallback or execution errors. The repeated median
is about 22.3× faster than the A1 SIMD result and 12.7× faster than Pillow.
The CPU route was unchanged and remained about 5.0× faster than Pillow.

The uniform workload retains its original `successful_execution` gate. Exact
parity was run separately, not relabelled into the benchmark gate:

- CPU: `convolution-uniform-color-trial-20261014.workflow.native-rgb-pad-1024`,
  run `migration-parity-bc1e23f2c541473dbbfd48c950407681`, 1/1 passed.
- SIMD: `convolution-uniform-color-trial-20261014.guard.simd-rgb-1024`,
  run `migration-parity-ca82a37f65724e01869b5fadb1344929`, 1/1 passed.

Therefore this measurement is not eligible for baseline promotion until a
validated workflow-parity link binds the exact benchmark and parity inputs,
build, producer, and runtime identities.

## Noise guard and remaining gap

The unchanged full-size guard is
`pipeline-matrix.expanded.filter5x5.1024x768`, RGB noise, with one warmup,
three iterations × two samples, and a passing exact parity gate (2/2 profiles):

| Cohort | Pillow median | CPU median | SIMD median |
| --- | ---: | ---: | ---: |
| A1 before candidate | 34.531 ms | 6.091 ms | 53.132 ms |
| B1 candidate | 34.948 ms | 6.117 ms | 53.409 ms |

The noise SIMD result changed by +0.52% between these cohorts and still runs
about 0.65× Pillow. The guard executes the existing vector convolution
(293,376 Filter5x5 blocks); the uniform scan rejects the sampled noise without
claiming an identity shortcut. The still-slow noise path is the next Filter5x5
target. Pillow-SIMD and GPU were not measured here.

## Evidence and limits

Benchmark report IDs:

- A1 uniform: `migration-benchmark-3f93e45ca6a84ddaaa35a1950d7d960c`
- B1 uniform: `migration-benchmark-dbd989eaab0442638b137dd1deadfc8c`
- B2 uniform: `migration-benchmark-5a82236a192e4216b6ef3207b3ea93fc`
- A1 noise: `migration-benchmark-110432e511d847babe5628dcd94f9a55`
- B1 noise: `migration-benchmark-f177b3a278244b5da5f1dff40b2008e0`
- B1 noise parity gate: `migration-parity-benchmark-gate-99f4007b5770483398f6e3ab3b54071b`

Benchmark reports, raw sample sidecars, and gzip-compressed uniform parity
reports are retained beside this README. SHA-256 values below refer to the
original, uncompressed JSON. Attestations, producer bundles, and the large
noise-gate report remain on the native runner at
`/home/appunni/work/pillow-rs-ycbcr-20261018/build/migration-parity/filter-rgb-5x5-rebaseline-20261019/`.
The 18 MB compressed noise-gate report was not copied. Report hashes:

- A1 uniform: `4d89f91fb908a9b1fd3c57be99b81a980f44c350a3fe693a512253d52dadda61`
- B1 uniform: `a4a3bff30ac63ab0f96f21f88e2e528c51d22e4644e264ff3891c374dc5b1791`
- B2 uniform: `f5e048286134a9fe0312e2817e9600afee6be379d2dc4ee852326964ae47a6bc`
- A1 noise: `0bf1aee1cea1ecf127e8f16fe536e140a20df8c8f5bfd05e3fe48118bd278a7f`
- B1 noise: `583fea079431564f54a1b1516cc257c62310bcca7ed508cda71ca4e30f4c2d20`
- CPU parity report: `6032ebed3c904cbb2020e2c859002db1e0f827e689e4f24227f63e9e2645c575`
- SIMD parity report: `72be4bfd340ac155285c8f898d2001f7fe8a78f5e0f9fc7968f7cb17ba074c9f`
- Noise parity-gate report (retained on runner): `c316720bd389ac63d38ca4c9b4c04f20df50141286c8a1d6e0b6c525e5630756`

The runner reports show
`host_allocated_bytes = 0` for the SIMD sample, so that counter is not evidence
that the cloned image allocation disappeared; the complete workflow latency
does include the clone and Pad materialization.

No benchmark or parity workload was weakened. No coverage ran. No genuine test
defect was found; the initial candidate build exposed an adapter visibility
error, which was corrected through the family kernel facade before measurement.

The five benchmark results and two compressed uniform parity reports were
imported into the generated DuckDB at
`/Users/lazytrot/work/pillow-perfdb/performance-results.duckdb`; the local
history now retains A1, B1, and B2 per workload/profile. No baseline channel
was promoted: the uniform workload still needs a validated workflow link, and
the noisy RGB SIMD workload remains slower than Pillow.
