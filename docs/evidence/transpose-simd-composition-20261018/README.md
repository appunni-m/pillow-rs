# Consecutive transpose SIMD composition

## Change

The automatic CPU/SIMD segment planner ended a SIMD segment after every
transpose because transpose changes the image shape. The SIMD batch executor
already composes consecutive transpose operations, but the planner passed each
operation in a separate batch and prevented that path from running.

The planner now extends a segment across adjacent transpose operations only.
It checks every operation against the segment's original input layout before
dispatch. Crop, conversion, extraction, and other shape or mode changes remain
segment boundaries. The executor composes the transforms, allocates one output,
and runs one native transpose kernel.

## Frozen workload and checks

- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`
- Parity case: `PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`
- Guard: `PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0`
- Input: RGBA, 772×768; two transpose calls followed by materialization.
- Boundary: `observed_steps`, `transpose-0;transpose-1;materialize`.
- Warm cache, five warmups, twenty iterations × five samples, concurrency one.
- Candidate benchmark preflight passed exact Pillow parity for CPU and SIMD on
  both ARM64 and x86_64. The odd-size guard passed strict SIMD parity on both.
- Focused Rust test passed:
  `cargo test -p pillow-rs --lib automatic_simd_composes_consecutive_transposes`.
- No full parity or benchmark campaign, coverage, or GPU run was performed.

## Latency results

Medians are milliseconds. Each after-ratio uses Pillow from the same report.
The pre-change ARM64 measurement is the exact local worktree before the planner
edit. The pre-change x86 measurements are diagnostic reports from the native
runner. The current x86 candidate was built on that runner with its existing
dirty files preserved, but the earlier diagnostic did not retain a complete
source hash manifest. Treat the raw 8.811-to-3.016 ms change as indicative, not
as a source-attested A/B result.

| Host | Profile | Before | After | Pillow in after report | After / Pillow | Interpretation |
|---|---:|---:|---:|---:|---:|---|
| ARM64 | CPU | 0.674 | 0.697 | 1.428 | 2.05× | Meets CPU ≤ Pillow for this case |
| ARM64 | SIMD | 0.874 | 0.700 | 1.428 | 2.04× | Meets interim SIMD ≤ Pillow/2, below 5× goal |
| x86_64 | CPU | 2.934 | 2.986 | 1.998 | 0.67× | Still 1.49× slower than Pillow |
| x86_64 | SIMD | 8.811 | 3.016 | 1.998 | 0.66× | 2.92× lower latency than prior SIMD run, still 1.51× slower than Pillow |

Pillow's ARM64 median changed from 1.326 ms in the pre-change run to 1.428 ms in
the candidate report. The two x86 baseline profiles used separate reports with
Pillow medians of 2.582 ms and 2.602 ms; the after report's Pillow median was
1.998 ms. Therefore, use the within-report after ratios for Pillow comparisons
and the raw target medians only for before/after screening. This is not an
interleaved A–B–B–A result.

## Execution receipts

The after reports completed the full public workflow and report actual backend
identity, no fallback, two transposes fused, and one output allocation of
2,371,584 bytes. Both profiles report two host-buffer boundaries totaling
4,743,168 bytes.

- ARM64 SIMD: 148,224 vector blocks, `native-copy`, actual `simd`.
- x86_64 SIMD: 148,224 vector blocks, `native-copy`, actual `simd`.
- CPU controls: actual `cpu`, no vector blocks.
- The old SIMD receipts reported two output allocations and four buffer
  boundaries. The old x86 SIMD run reported zero fused operations; the old
  ARM64 run did likewise.

## Evidence identities

The small benchmark reports, raw samples, and execution attestations are kept
in this directory. Full parity outputs contain serialized image bytes and are
not copied here; their SHA-256 values and report-bound attestations are kept.

| Host | Benchmark run | Benchmark SHA-256 | Parity report | Odd guard report | Python extension SHA-256 |
|---|---|---|---|---|---|
| ARM64 | `migration-benchmark-09df9568df59490f95474969adcc5db4` | `5bacf75850f7421cde721136a0149f6ad4ea803b2ba0dd261e9cbd88b994e8b2` | `migration-parity-benchmark-gate-105a1a9782f4436abd777f129cfb1235`, `8c47e08cf372b478a2c704f1bbd666758a697818d8a1a7970285350cd4a6f007` | `migration-parity-83597f78f4af4b66bca6e9c52d34d789`, `55251ea42cc6eab2ba6fe475b4a5da0270016fc26eda50f18bfb3281d4ec4075` | `a2882b8d6aff0c1c9cd63671f49a3838d02c834a0b1b8e70ecffbaa74c3d42cc` |
| x86_64 | `migration-benchmark-ca7d3dbe1b6341fcb4839056dec20d03` | `6a1332c7c00d756f0733c95310681aa8322f661ab5c399c89b45ba193f100be6` | `migration-parity-benchmark-gate-934d75e25b734656ae83a4bf8a34b44d`, `f235fd4fada363e9708ecb1a64103f403ad1645b9638fa6a0f27f8cc47dd501a` | `migration-parity-a35f8e03c907453aa545cdc3520bb018`, `73f2a954ad8375a3e5bff4d7e8575c8f5cf7d8cb8d7e5ce3a036ecb622d965b5` | `10d45be2b16f3f19834129b7ae226091f1b56da86d7457d1d6d3c6d0a81d0594` |

The source revision was `418d5f0cde51ca64a97cbd007ea731da72d68abf` with dirty
worktrees. The local planner source hash after the edit was
`5e1f10745d4de5ab4b54d7ed33045b4ea37065d867b5942c34219b37f99cbd2a`; the x86
runner's corresponding file hash was
`681c49a3dc79ad04442fa47a1b6e1522bf21d805074decd1907fedd173540d74`. The
runner's existing dirty changes were preserved. At candidate time its SIMD
pool and adapter hashes were `e3f6ad8a5903a1896b620921cb8c8b75bf7453af124fa3397bcbd7e6f0ced915`
and `8d8b1e465617c095ad96f8d41db8fd4c3427c49f550055e54e9a51086686e92a`,
respectively. Those whole-file hashes differ from the local current tree due
to unrelated dirty work; their old-run hashes were not retained, so do not
claim a strict cross-run source match.

## Decision and remaining work

Keep the narrow composition fix as partial progress: the ARM64 exact-source
candidate removes an extra full-frame SIMD transpose pass. The x86 current
candidate also runs the composition path, but its before/after timing is only
diagnostic without a complete baseline source manifest. It does not optimize
the serial CPU kernel, and the current x86 CPU and SIMD rows remain slower than
Pillow. The 5× SIMD goal, other modes and shapes, Pillow-SIMD comparison, GPU
latency, and sustained GPU throughput remain open.

Kata parent: `pillow-rs#m2w4`. Route diagnosis: `pillow-rs#5xwb` (closed).
Next read-only kernel investigation: `pillow-rs#t4c8`.
