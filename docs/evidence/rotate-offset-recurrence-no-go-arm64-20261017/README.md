# RGBA Rotate source-offset recurrence — rejected — ARM64, 2026-10-17

Status: **candidate rejected and removed**. The candidate preserved exact output
for the selected cases but made the measured public CPU call slower. The exact
baseline source and extension were restored. Rotate remains open; this trial
earns no operation-completion credit.

## Frozen workload and proof

The workload was
`pipeline-chain.geometry-material.rotate-rgba-1024x768`: RGBA 1024×768,
nearest Rotate at 37° with expanded output, followed by terminal `tobytes`.
Input setup was outside the timed boundary. The public workflow included output
materialization. Runs used release builds on the same macOS ARM64 host, warm
cache, concurrency one, one warmup, and 3 iterations × 2 samples per subject.

The CPU receipt reports actual `cpu`, one Rotate operation, terminal completion,
and no fallback on all four legs. Exact Pillow parity passed 10/10 for each
source state: A1 `migration-parity-8666dce9416e4f6dacad28661c990767`, B1
`migration-parity-b7c0277fbbbb463eb2ef151417143dbb`, and A2
`migration-parity-a2ea44395fa048e5865201b95ea0daa5`. The B2 parity proof reuses
B1 because source, extension, inputs, producer, oracle, and CPU profile match.
The selected guards cover RGBA nearest arbitrary rotation and fractional
coordinates, RGB nearest, CMYK, premultiplied `RGBa`, expanded output, fill
bytes, and the materialized RGBA pipeline. Each proof selected and executed
10/10 cases with zero failures or infrastructure errors.

## Source-matched A–B–B–A

| Leg | Benchmark run | Core SHA-256 | Pillow median (ms) | CPU median (ms) | Pillow / CPU |
|---|---|---|---:|---:|---:|
| A1 control | `migration-benchmark-6b4b21f3e9324f72a9c67da0ef4f2d6d` | `bdafdc95d73debbd2cb9c8969bccdc2b079ede28bb1c8a0e2c1c6e2d80a62360` | 2.705480 | 1.644229 | 1.645× |
| B1 candidate | `migration-benchmark-0c2a9530e4564c31a67ed74e9e73288a` | `4fb1ec17ac12c693b1b3778f35ab757075c477f15718d05201fd44b4fec18287` | 2.486563 | 2.054105 | 1.211× |
| B2 candidate | `migration-benchmark-e830428af8f6483aa8d15f0464edf089` | `4fb1ec17ac12c693b1b3778f35ab757075c477f15718d05201fd44b4fec18287` | 2.664959 | 2.057125 | 1.295× |
| A2 control | `migration-benchmark-0ec68fb386a0486b9192cb737145ff89` | `bdafdc95d73debbd2cb9c8969bccdc2b079ede28bb1c8a0e2c1c6e2d80a62360` | 2.670437 | 1.783895 | 1.497× |

The control CPU medians average 1.714062 ms; candidate medians average
2.055615 ms, a 19.93% regression. Both candidate medians are slower than both
control medians. Candidate runs also reduce the Pillow speedup from the
controls' 1.497–1.645× to 1.211–1.295×, still short of the 2× prioritization
checkpoint.

## Candidate and disposition

The trial initialized a source byte offset per in-bounds span, retained the
16-bit x/y fractions, and advanced the offset using x/y carries when both
positive fixed-point steps were less than one pixel. Other step classes kept
the existing per-pixel sampler. This removed per-pixel row-major index
recalculation in the candidate branch, but the measured call regressed; the
trial does not establish which added operations caused the loss. The candidate
was removed without changing the SIMD or GPU kernels.

The exact control source SHA-256 is
`24645acf442d8356fe021f2baf861dd34275ac3e85ffda6b2913600e624696d9`; the
candidate source SHA-256 is
`5ea562c528c22150d9108caee1ec41398eef33cf61a39267aeee76821f3f5295`. The
restored core SHA matches A1/A2. Full reports, raw samples, parity reports,
attestations, and producer receipts remain under the ignored
`build/migration-parity/rotate-offset-recurrence-*` paths. Their eight
Pillow/CPU measurement rows are in
[`../performance-optimization-local-runs.csv`](../performance-optimization-local-runs.csv).
No coverage, commit, or push ran; the batch remains 0/33.
