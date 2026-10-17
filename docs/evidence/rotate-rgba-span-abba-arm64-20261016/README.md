# Serial RGBA nearest Rotate row-span clipping — ARM64, 2026-10-16

Status: **partial CPU candidate retained**. The candidate improves the measured
RGBA call by 3.70× over the matched serial control and beats same-run Pillow in
both candidate legs. It reaches only 1.48–1.81× Pillow, so it has not met the
user's 2× prioritization target. Rotate remains open across modes and backends.

## Measurement contract

The frozen workload is
`pipeline-chain.geometry-material.rotate-rgba-1024x768`: RGBA 1024×768,
nearest Rotate at 37° with expansion, followed by `tobytes`. Input/setup stays
outside the timed boundary; output materialization is included. The run used a
release build, warm cache, concurrency 1, 1 warmup, and 3 iterations × 2
samples per subject. Pillow 12.2.0 and pillow-rs ran in the same process cohort.

## Source-matched A–B–B–A

Each report has a completed CPU receipt, terminal materialization, and no
fallback. The control source/core hashes are `340f8919cb8596c1dfd4775c549488bb07e53e3ee5d0e7dac0bf8a19cc29ed20` /
`8224662a6214686fe35e6dff29ecadb106dc1f4046d1f11620d790937695f7f2`. The
candidate hashes are `d02910803282d6db6ca38a2afff249cdc4a25dd2577d852637b3f72a15ae1bbd` /
`7e6fced7ae0a16ea94847861e99c91a3d8be8c5a791613a91f2de525fffbd57c`.

| Leg | Run ID | Pillow median (ms) | CPU median (ms) | Pillow / CPU |
|---|---|---:|---:|---:|
| A1 control | `migration-benchmark-dcf6609a28b646848a961317cece9633` | 2.776584 | 6.725375 | 0.413× |
| B1 candidate | `migration-benchmark-150084b599be41c58a4f8b565befca66` | 3.055459 | 1.685250 | 1.814× |
| B2 candidate | `migration-benchmark-1bc1e2a7cc9542b1a3998e4b7021b74f` | 2.792938 | 1.886563 | 1.481× |
| A2 control | `migration-benchmark-f99db593c18340b8b1746905124e4f4c` | 3.020604 | 6.491083 | 0.465× |

Control mean CPU median was 6.608229 ms; candidate mean was 1.785906 ms, a
3.700× reduction against control. The matched Pillow baseline moved between
legs, so the two per-leg ratios are retained rather than collapsed into one
ratio.

## Exact parity and coverage boundary

The final current-candidate parity run is
`migration-parity-7cc906454a5b4a3fab5eb4451adf04e4`: six of six CPU
comparisons passed against Pillow, including:

- 1024×768 nonuniform RGBA noise, 37° nearest, expanded output, and custom
  four-channel fill `[17, 31, 47, 63]`;
- 1024×768 RGB nearest rotation at 1° on deterministic noise;
- the large materialized RGBA pipeline and the existing fractional-nearest,
  alpha-fill, and expanded-red-fill guards.

The nonuniform RGBA case validates the new span's source-byte copies and fill
channels, but is a parity guard only: this A–B–B–A measured the existing solid
RGBA pipeline input. Its input is not represented as a random/noise timing
workload. The core loaded for parity has the same SHA-256 as both candidate
benchmark builds. SIMD, Parallel CPU, and GPU were not measured.

The RGB fixture previously used by the RGB trial was missing from the source
generator. It has been restored as a versioned case and workload because the
original bytes could not be recovered. The current candidate's recovered RGB
workload passed exact parity and measured 1.252105 ms CPU versus 1.287292 ms
Pillow (1.028×), with actual CPU execution, terminal completion, and no
fallback. This is a new input identity and is not substituted for the old
RGB A–B–B–A evidence. Its current-source run is
`migration-benchmark-cf13f726d19b4cefbf3195ae2b9efe07` and it reused the
matching six-case parity proof above.

## Implementation

`pillow-rs/src/compute/pool_cpu/ops/geometry.rs` extends the checked positive
fixed-point in-bounds span to the serial four-channel path. It copies native
RGBA bytes in the span and fills the outside spans with the original four fill
bytes. Other channel counts, nonpositive steps, and arithmetic that cannot
prove a safe span keep the scalar path. The change does not touch the SIMD,
Parallel CPU, or GPU implementations.

Full reports, exact samples, producer receipts, and execution attestations are
retained under the ignored `build/migration-parity/rotate-rgba-span-*` and
`rotate-rowspan-large-nonuniform-*` paths. The sampled results are also in
[`../performance-optimization-local-runs.csv`](../performance-optimization-local-runs.csv).
The five raw benchmark-result artifacts are imported into the local
`pillow-perfdb` DuckDB; `history --source-kind benchmark-result` returns the
four RGBA bracket legs and the separate recovered RGB row.
Kata trial `pillow-rs#s2wm` tracks the bounded RGBA follow-up; parent operation
matrix `pillow-rs#0h0b` remains open. No coverage, full campaign, commit, or
push was run.
