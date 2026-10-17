# Native-L BoxBlur radius-one SIMD ring trial — 2026-10-16

## Change

For `ImageFilter.BoxBlur` on native `L`, one pass, radius 1 in both axes, the SIMD adapter now bypasses the full-frame input clone and full-frame horizontal scratch image. The focused `box_blur_luma` kernel keeps horizontally rounded rows in a three-row ring, then writes the vertical pass into the final output. Horizontal and vertical Pillow fixed-point rounding remain separate. Other modes, radii, pass counts, CPU, and GPU routes are unchanged.

## Frozen verification

- Workload: `pipeline-op.boxblur.material-l-noise-1024x768-radius-1`.
- Measurement boundary: `observed_steps`, `apply-filter` + `observe-filter-result`; full public-call work, output materialization, and observation remain inside timing.
- Policy: warm release, 5 warmups, 20 iterations × 5 samples (100), concurrency 1.
- Exact Pillow parity on the candidate: material `1024×768` and edge/tail `65×47` cases passed 2/2, strict `python-simd` profile. The unchanged L radius-one SIMD-vs-CPU edge/tail unit passed for 10 sizes from `1×1` through `1024×3`.
- Candidate receipt: requested/actual SIMD, `path=vector`, 98,304 vector blocks, zero scalar tail, terminal complete, no fallback. The default build has the opt-in `parallel` feature disabled.
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0. HEAD `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`; dirty tree.

## A/B/B/A results

| Run | Extension SHA-256 | Pillow median | SIMD median | Pillow / SIMD | SIMD p95 |
| --- | --- | ---: | ---: | ---: | ---: |
| A1 control | `d9cb7176…` | 1.783334 ms | 0.672187 ms | 2.653× | 0.737791 ms |
| B1 candidate | `4a879cd9…` | 1.781729 ms | 0.599125 ms | 2.974× | 0.637416 ms |
| B2 candidate | `4a879cd9…` | 1.736000 ms | 0.605687 ms | 2.866× | 0.728792 ms |
| A2 control | `d9cb7176…` | 1.756959 ms | 0.650021 ms | 2.703× | 0.686333 ms |

The mean of the two SIMD medians fell from 0.661104 ms in the controls to 0.602406 ms in the candidate runs (8.9% lower full-call latency). Both candidate runs stayed below Pillow latency and were repeatable at the median. The p95 varies by run; the result is a local median improvement, not a tail-latency claim. SIMD remains about 2.92× Pillow here and does not meet the repository's 5× final SIMD target.

The harness's host-allocation counters do not account for these kernel-local Rust `Vec` buffers. The measured call still includes their real allocation costs; the allocation reduction is established from the old and new code paths, not from those counters. The candidate retains one final output allocation plus three row buffers instead of two full-frame working buffers before materialization.

The A2 parity reuse attempt was rejected because its runtime identity did not match A1; a fresh focused A2 material parity run passed before timing. This is the intended identity guard, not a test defect. No genuine test defect was found and no coverage ran.

## Disposition

Retain the focused candidate as a partial SIMD improvement and keep BoxBlur open. Record it in the local run ledger only: this dirty-tree, ARM64 cohort does not change the clean published matrix and does not establish other modes, radii, or sizes. The 5× gap is tracked in Kata `pillow-rs#jexq`; continue with another higher-priority owned gap rather than treating this as operation completion. No commit or push ran; batch remains 0/33.

Raw reports, samples, parity receipts, producer attestations, and runtime identities are retained beside this file. `SHA256SUMS.txt` covers every file in this directory except itself.
