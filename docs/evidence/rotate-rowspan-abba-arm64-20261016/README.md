# Serial RGB nearest Rotate row-span clipping — ARM64, 2026-10-16

Status: **keep as a scoped serial-CPU candidate** for RGB nearest-neighbor Rotate at 1024×768. This improves the measured CPU path beyond the matched control and just below Pillow latency. It does not establish the 2×-Pillow target, cover other modes or transforms, or complete Rotate across SIMD, Parallel CPU, and GPU.

## Frozen measurement

Primary workload: `pipeline-op.rotate.material-rgb-nearest-noise-1024x768`, exact case `PIL.Image.Image.rotate.nuanced.rgb-nearest-1deg-1024x768`. RGB 1024×768, angle 1°, nearest, `expand=False`; the public call and `tobytes` materialization are timed together, with input setup outside the timer. Warm release; 5 warmups; 20 iterations × 5 samples; concurrency 1. The separate 32×32 expanded-Rotate workflow is only a successful-execution guard, not an exact-parity claim.

## Source-matched A–B–B–A

Medians are per run. Every CPU receipt says actual `cpu`, terminal completion, and no fallback. The controls use geometry source SHA-256 `05d2bffe112a0b1870d667045d35a87cc5f2ace4135f72e3f85f1dbf58203060` and `_core.abi3.so` SHA-256 `88b86723994eb2ac0f15cea0424215bc47e5d5a9f8fe201e677c339e49c1132d`. Candidate runs use geometry SHA-256 `340f8919cb8596c1dfd4775c549488bb07e53e3ee5d0e7dac0bf8a19cc29ed20` and core SHA-256 `8224662a6214686fe35e6dff29ecadb106dc1f4046d1f11620d790937695f7f2`. Both binaries were rebuilt from this worktree; the candidate rebuild reproduced the measured candidate hash.

| Leg | Primary Pillow (ms) | Primary CPU (ms) | Pillow / CPU | 32×32 Pillow (ms) | 32×32 CPU (ms) |
|---|---:|---:|---:|---:|---:|
| A1 | 1.867604 | 3.427917 | 0.545× | 0.030875 | 0.025750 |
| B1 | 1.486479 | 1.364188 | 1.090× | 0.032084 | 0.024937 |
| B2 | 1.612709 | 1.428167 | 1.129× | 0.032188 | 0.029375 |
| A2 | 1.540271 | 3.450375 | 0.446× | 0.032959 | 0.030146 |

The mean primary CPU median fell from 3.439146 ms on control to 1.396177 ms on candidate: **2.463× faster than the matched control (59.4% lower latency)**. Candidate CPU was 1.090× and 1.129× faster than its same-run Pillow samples. The 32×32 guard did not regress; its candidate CPU mean is 0.027156 ms versus 0.027948 ms on controls.

Earlier Rotate controls with another `_core` hash were excluded from this comparison. The four runs above share the same workload/input hashes, host, revision, release profile, timing boundary, warm policy, sample counts, and default serial-CPU feature set.

## Parity and implementation boundary

The candidate is gated to serial RGB (`channels == 3`) nearest affine rotation when both per-output-x fixed-point steps are positive. It computes the in-bounds destination span with checked integer arithmetic, fills the two outside spans with the existing native fill bytes, and retains the original per-pixel path for other channel counts, nonpositive steps, and unprovable arithmetic. The implementation is `cfg(not(feature = "parallel"))`.

Exact candidate parity passed 3/3 for the large RGB nearest case, expanded red fill, and fractional RGBA nearest. The final rebuilt candidate parity run is `migration-parity-e68c7e878c1447dba0ac8971661c06ff`; the benchmark B1/B2 parity proof is `migration-parity-2bb3c38f613541c1b3e550730b779a3e`. Both benchmark controls passed the primary exact case (1/1 each). All four benchmark execution and producer attestations are complete. The focused Rust test `rotate_positive_step_span_matches_signed_fixed_point_bounds` passed 1/1 (596 filtered).

No SIMD, Parallel CPU, or GPU lane was measured. No full Rotate claim is made. The per-run benchmark reports, execution/producer attestations, raw samples, and parity reports remain in ignored `build/migration-parity/` under the `rotate-rowspan-control-*` and `rotate-rowspan-controlled-*` names. Sixteen Pillow/CPU subject rows are in [`../performance-optimization-local-runs.csv`](../performance-optimization-local-runs.csv). Kata trial `pillow-rs#gknj` records this scoped experiment; the parent operation remains open.

No genuine test defect was found. No full parity/benchmark campaign, coverage, commit, or push was run.
