# Native-L MedianFilter 3×3 SIMD selection network — 2026-10-16

## Change

For the SIMD `MedianFilter` path on native `L`, size 3 and rank 4 now use the
fixed 19-comparator median-of-nine selection network, operating directly on the
nine `u8x16` sample vectors. The generic nine-value odd-even sort used 36
compare-exchange pairs. Other modes, sizes, ranks, tails, edge handling,
allocation, and the CPU/GPU paths retain their existing implementation.

## Frozen verification

- Workload: `pipeline-op.medianfilter.material-l-noise-1024x768`.
- Material parity case: `PIL.ImageFilter.MedianFilter.nuanced.performance-material-l-noise-1024x768-size-3`.
- Edge/tail parity case: `PIL.ImageFilter.MedianFilter.nuanced.backend-noise-l-33x35-size-3`.
- Boundary: `observed_steps`, `apply-filter` + `observe-filter-result`; full
  public call and output observation are timed.
- Policy: warm release, 5 warmups, 20 iterations × 5 samples (100), concurrency
  1. Each A/B result is from a separate benchmark process.
- The SIMD network unit contract passed for 4,096 deterministic inputs across
  16 lanes, including ties, repeated values, ascending/descending values, and
  replicated edge windows.
- Strict candidate parity passed 2/2 on `python-simd`. The final candidate
  receipt says requested/actual SIMD, `path=vector`, 49,152 vector blocks, zero
  scalar tail, zero mode conversions, terminal complete, and no fallback.
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7 ARM64, CPython 3.12.13, Pillow
  12.2.0. Source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty
  worktree. Default Cargo features; `parallel` disabled.

## A/B/B/A results

| Run | Extension SHA-256 | Pillow median | SIMD median | Pillow / SIMD | SIMD p95 |
| --- | --- | ---: | ---: | ---: | ---: |
| A1 control | `4a879cd9…` | 41.598146 ms | 7.882917 ms | 5.277× | 8.111875 ms |
| B1 candidate | `315a2f32…` | 40.866292 ms | 5.853480 ms | 6.981× | 6.068750 ms |
| B2 candidate | `315a2f32…` | 41.571750 ms | 5.898688 ms | 7.047× | 6.360959 ms |
| A2 control | `4a879cd9…` | 40.956521 ms | 7.813959 ms | 5.242× | 8.103500 ms |

The mean of the two SIMD medians fell from 7.848438 ms in the controls to
5.876084 ms in the candidate runs (25.1% lower full-call latency). Both
candidate runs improved over both controls, with Pillow medians remaining
stable. Candidate p95 latency also fell in both B runs. These ARM64 results
clear 5× Pillow for this workload, but do not establish the x86 target or
Pillow-SIMD comparison.

The clean x86 Pillow-SIMD snapshot at
`docs/evidence/pillow-simd-benchmark.json` measured this workload at 4.344×
Pillow and 4.445× Pillow-SIMD before this change. That snapshot is an old
selection signal, not candidate evidence. The native x86 runner was unavailable
for this experiment, so keep the x86/Pillow-SIMD claim open.

The first A2 attempt to reuse A1 parity was rejected because the runtime
identity did not match. A fresh A2 material-only parity proof passed against
the restored control binary before timing. This is the identity guard working
as designed, not a test defect. Candidate material and edge/tail parity passed
in a single strict run. No genuine test defect was found and no coverage ran.

## Disposition

Retain the focused candidate as a local ARM64 SIMD improvement. Keep
`pillow-rs#07jq` and its parent operation issue open until a native x86 build,
parity run, and same-host Pillow/Pillow-SIMD benchmark prove the result there.
This dirty-tree run does not update the published clean matrix. No commit or
push ran; the user's batch is still below 33 completed changes.

The benchmark reports, raw samples, parity reports, producer artifacts, and
execution attestations are retained beside this file. `SHA256SUMS.txt` covers
the evidence files other than itself.
