# Pending RGBA→RGB GetChannel fusion trial — ARM64, 2026-10-20

## Scope

This trial tested whether `getchannel(0..2)` could skip a pending plain
RGBA→RGB conversion and extract the selected byte directly from RGBA storage.
It used only these workloads, with the standard warm whole-workflow policy
(5 warmups, 20 iterations × 5 samples, concurrency 1):

- `pipeline-chain.getchannel.seeded-corners-1024x768-all-bands`
- `pipeline-chain.getchannel.seeded-corners-16x16-all-bands` (small-call guard)

The candidate report is under
`build/migration-parity/getchannel-rgba-rgb-fusion-abba/a-candidate-1.json`;
the selected no-fusion control is
`build/migration-parity/getchannel-rgba-rgb-fusion-abba/b-control.json`.
Both reports include same-run Pillow and execution receipts. Candidate parity
passed 5/5 frozen cases on CPU and 5/5 on SIMD. Candidate benchmark preflight
passed both selected workload pairs; the control benchmark's selected
preflight also passed both pairs.

## Result

| Workload | Variant | pillow-rs SIMD median | Pillow median | SIMD / Pillow speedup |
| --- | --- | ---: | ---: | ---: |
| 1024×768 | fusion candidate | 1.0513 ms | 1.4577 ms | 1.39× |
| 1024×768 | no-fusion control | 0.8448 ms | 1.2318 ms | 1.46× |
| 16×16 | fusion candidate | 0.1144 ms | 0.1133 ms | 0.99× |
| 16×16 | no-fusion control | 0.1153 ms | 0.1133 ms | 0.98× |

The large workflow was about 24% slower in the candidate by median. The small
guard was effectively unchanged. These are one candidate run and one control
run taken three minutes apart on the same ARM64 host (candidate run
`migration-benchmark-116794d6b0664a7ea01aecae4a7e262a`, control run
`migration-benchmark-29b63ab160a64636bb14aa675d00dc4a`), not an interleaved
A–B–B–A cohort; the result is a rejection signal, not a precise causal
estimate. Both runs nevertheless completed the same selected workload
contracts, and no additional campaign was run to chase the result.

Both variants reported actual SIMD, terminal completion, and no fallback. The
candidate receipt showed the fused Convert path followed by NEON channel
deinterleave (49,152 blocks at 1024×768). The control used NEON channel
deinterleave (also 49,152 blocks) after ordinary materialization. Reducing an
intermediate frame therefore did not make this public workflow faster; the
RGBA-stride extraction path cost more than it saved here.

## Decision

Reject the fusion and retain the ordinary materialize-then-extract path. Do not
reuse these timings as a baseline. A future attempt must beat a source-matched
control on the same full workflow and preserve the 16×16 guard. No tests,
thresholds, workloads, or parity assertions were weakened.
