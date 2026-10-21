# RGB 5x5 BLUR ring SIMD candidate on x86_64

This experiment replaces scalar RGB gather work for Pillow's exact 5x5 BLUR
ring with vertical rolling byte-column sums and vectorized interleaved RGB
taps. The family orchestrator admits RGB8, the exact BLUR kernel, scale 16,
offset 0, and images above the small-image cutoff. Other modes, parameters,
and smaller inputs keep their existing routes. The ring kernel is excluded
from the opt-in Parallel CPU profile.

Source follows the single repository layout standard in
[`IMPLEMENTATION_STRUCTURE.md`](../../../IMPLEMENTATION_STRUCTURE.md): the
SIMD family orchestrator is
`pillow-rs/src/compute/pool_simd/ops/adapters/filter.rs`; its focused kernel is
`pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/rgb_blur_ring.rs`.
No generic pixel dispatch was introduced.

## Noise workload

The measured boundary is the complete public Filter5x5 workflow, including
setup, allocation, execution, and materialization. The pinned policy is warm
cache, concurrency 1, one warmup, three iterations, and two samples per
iteration. Fresh exact Pillow parity passed for CPU and SIMD before timing.

| Profile | Median latency | Speedup vs Pillow | Actual route and receipt |
| --- | ---: | ---: | --- |
| Pillow 12.2.0 | 35.631 ms | 1.00x | Oracle |
| Serial CPU | 6.329 ms | 5.63x | CPU, terminal complete, no fallback |
| SIMD | 5.812 ms | 6.13x | SIMD, terminal complete, 291,848 vector blocks and 3,056 scalar-tail components |

The resource receipt reports 12,288 allocated bytes for CPU and 2,371,584
allocated bytes for SIMD, with 4,718,592 host-buffer bytes in either profile.
These allocations are inside the timed public call; there are no mode
conversions or device transfers.

The first run is retained under
[`noise/initial-machine-id/`](noise/initial-machine-id/) as diagnostic only.
It had no `runner_id`, so its row cannot form a stable matched-host comparison
in `pillow-perfdb`. Its medians were Pillow 34.874 ms, CPU 6.137 ms, and SIMD
5.568 ms. Do not use those values for cross-run claims. The stable-identity
run is under [`noise/identified-runner/`](noise/identified-runner/):

- Benchmark run: `migration-benchmark-a8bdc7ac4016428d9f233e78de723c63`.
- Benchmark JSON SHA-256:
  `c38e7c63e69959c7772ceabfdd91b67ce58ece144f90402b57d4e00074c5c81b`.
- Fresh parity gate: `migration-parity-benchmark-gate-f139ada288584d4e908a7e26ae26e9a6`.
- Parity report remains on the native runner to avoid copying a 25 MB file.
  Its JSON SHA-256 is
  `1b55e033af797e8608ffbe1cb5d574fcb63d496e761bb65495fd1fb7360176e3`; its
  deterministic gzip SHA-256 is
  `39e8e757473ac0fb2f1bcae9e17bedaeb2d0e0939db4a128d1ddcd1d3a7e52a9`.
- Runner path:
  `/home/appunni/work/pillow-rs-ycbcr-20261018/build/migration-parity/filter5x5-rgb-blur-noise-identified-20261019/parity.json`.

An earlier no-ring B1 SIMD median was 53.409 ms. The 9.19x ratio against the
stable-identity run is diagnostic only: the B1 record has a per-run runner ID,
so `pillow-perfdb` does not classify the rows as a matched cohort. The same
applies to the earlier B1 CPU median (6.117 ms versus 6.329 ms now). The
current same-run Pillow ratios are valid for this workload, but this one
single-thread latency result does not establish sustained throughput or
complete `PIL.Image.Image.filter` across its modes, kernels, sizes, or
architectures.

## Uniform-image regression guard

This whole-workflow guard measures the existing uniform RGB Filter5x5 followed
by Pad fast path. Its benchmark gate is `successful_execution`; exact Pillow
parity was run separately against the current source and passed 1/1 for CPU and
1/1 for SIMD.

| Profile | Current median | Previous B2 median | Change |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 30.256 ms | 29.432 ms | +2.8% |
| Serial CPU | 5.946 ms | 5.869 ms | +1.3% |
| SIMD | 2.448 ms | 2.315 ms | +5.7% |

The run-to-run difference is inconclusive; it is not evidence for either a
regression or no regression. Keep this guard open and compare a controlled
repeat before accepting a layout or dispatch change that affects this path.
The current report is under
[`uniform/identified-runner/`](uniform/identified-runner/), run
`migration-benchmark-710cfeeacb254d898249590e493a7be7`, JSON SHA-256
`c6286ad2ca1f51a2beac344313d3630d289cdb0ebd8d034c2e1d29793359fb87`.
The separate CPU and SIMD parity reports, attestations, rebaseline manifest,
and earlier unlabelled diagnostics are under [`uniform/`](uniform/).

## Validation and source identity

- Native x86_64 release extension build passed with `make build-parity`.
- Focused Rust ring-path tests passed 2/2 on the native runner.
- Fresh noise parity passed for both CPU and SIMD profiles; uniform exact
  parity passed 1/1 for each profile.
- `cargo check -p pillow-rs --features parallel` passed locally; the serial
  SIMD ring kernel and its tests are cfg-excluded from Parallel CPU.
- Focused ARM64 family tests passed 4/4 before the x86 run.

The native runner is an AMD Ryzen 5 2600X, x86_64 Linux, CPython 3.12.3,
Pillow 12.2.0, Rust 1.96.1. The source commit was
`418d5f0cde51ca64a97cbd007ea731da72d68abf`, with a dirty working tree. The
ring kernel SHA-256 is
`eb5bf84a14d9efd07133a981d7f1dd72486d67233a4451931415a20f5ff9f8a9`; the
Filter5x5 dispatch function SHA-256 matched between runner and workspace at
`6431d987572c51093ab60ab4f3ff6470f2786a2a8f6ab0a29ff2c8c0fa71e1ac`. Other
unrelated edits in the large dispatch facade differ between dirty trees; the
family modules and measured Filter5x5 function match.

Benchmark reports are imported into the generated `pillow-perfdb` DuckDB. No
baseline was promoted: repository admission still requires a validated
workflow-parity link binding the benchmark, parity proof, producer, and exact
source/build identities. No genuine test defect was found in this experiment.
The filter operation remains open for other kernel parameters, modes, sizes,
architectures, Pillow-SIMD comparison, and GPU measurements.
