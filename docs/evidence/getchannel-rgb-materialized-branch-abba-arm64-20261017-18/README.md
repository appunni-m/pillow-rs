# GetChannel cached RGB parent branch — ARM64 A–B–B–A

Status: **partial candidate improvement; GetChannel remains open**. The change
branches channel extraction from the pending non-paletted pipeline's resolved
pixels after validation materializes the parent. This avoids replaying a
mode-changing parent graph for every extracted band while preserving the
explicit backend lock and copy-on-write behavior.

## Frozen scope

- Workload: `pipeline-chain.getchannel.seeded-corners-1024x768-all-bands`
- Exact parity case: `PIL.Image.Image.getchannel.nuanced.pipeline-seeded-corners-1024x768-all-bands`
- Input: a 1024×768 RGB result of pending RGBA→RGB conversion with seeded
  corners; all RGB channels are extracted and terminally materialized.
- Boundary: `whole_workflow`; warm cache; 5 warmups; 20 iterations × 5 samples;
  concurrency 1. The timed workflow includes public calls and output
  materialization.
- Host: macOS 15.7.7 ARM64; Pillow 12.2.0; CPython 3.12.13.
- Source revision: `1c9203b11e789059b139bbd4772bc3184cf37511` (dirty tree).
  Controls loaded `_core` SHA-256
  `f57829c34996eb451f855314c459eec3cb2aba04fefb57a3a6cc02c6aa352529`;
  candidates loaded `_core` SHA-256
  `d0f0eba64191ef3dc0310085e9ae37e241cb406c465031a274729c5b5e47b807`.
- Full-workflow parity passed for the CPU and SIMD profiles in all four legs;
  B2 reused B1's identity-matched proof. A2 ran fresh parity after strict
  identity checks refused a proof reuse. No parity mismatch occurred.

## End-to-end results

Median latency, milliseconds. Each speed ratio is calculated against Pillow in
the same run.

| Leg | Pillow | Serial CPU | SIMD |
| --- | ---: | ---: | ---: |
| A1 control | 1.099 | 1.658 | 16.177 |
| B1 candidate | 1.099 | 0.898 | 4.467 |
| B2 candidate | 1.074 | 0.721 | 4.615 |
| A2 control | 1.058 | 1.727 | 16.569 |

The two CPU control medians average 1.692 ms; the candidate medians average
0.809 ms, **52.2% lower latency (2.09× over control)**. Both candidate CPU
runs beat Pillow, by 1.23× and 1.49×. SIMD control medians average 16.373 ms;
candidate medians average 4.541 ms, **72.3% lower latency (3.61× over
control)**. Candidate SIMD is 0.233–0.246× Pillow speed in these runs, so it
remains roughly four times slower than Pillow and misses both the requested
half-Pillow target and the repository's 5× SIMD target. This result does not
close GetChannel across modes, sizes, and backends.

The receipts show actual CPU/SIMD execution, terminal completion, and no
fallback. Candidate SIMD used the NEON deinterleave path with 49,152 vector
blocks. GPU was not part of the performance cohort. The separate GPU parity
guard proves GPU execution on a tiny case, not GPU latency or sustained
throughput.

One execution resource receipt per run reported candidate host-buffer bytes
falling from 9,437,184 to 3,145,728, peak live host bytes from 6,291,456 to
3,145,728, and mode conversions from 2 to 1. It reported one 786,432-byte host
allocation in both variants. These are one-receipt counters, not timing-sample
means.

## Copy-on-write and backend guard

`PIL.Image.Image.getchannel.nuanced.pipeline-parent-mutation-after-channel-branch`
passed strict parity on CPU, SIMD, and GPU. It converts a 3×2 RGBA parent to
RGB, branches its green channel, mutates the parent, and observes both results.
A focused Rust test also checks that an explicit CPU backend lock survives the
materialized branch. The tiny SIMD extraction correctly reports `scalar-gather`;
that case is a semantic guard, not vector-acceleration evidence.

## A–B–B–A runs

- A1: `migration-benchmark-d187ee97593141b8a9963dcb0fe3463b`
- B1: `migration-benchmark-7bb0186cade7469a81348f09a4978458`
- B2: `migration-benchmark-79306742e76e484fb8c7c0f1e5a942ad`
- A2 retry control: `migration-benchmark-78361677639c42fdb7c0c5c3d17151f0`

B1/B2 used matching candidate extension hashes; A1/A2 used matching control
hashes. The initially rejected A2 proof-reuse attempt is excluded; A2's fresh
exact parity report is retained. B2's proof points to B1 because its source,
build, input, producer, oracle, and profile identities matched.

Reports, raw samples, execution and producer attestations, producer bundles,
exact parity proofs, and the focused three-backend copy-on-write receipts are
retained in this directory. Large parity reports are gzip-compressed and remain
bound by the retained parity attestations. `SHA256SUMS.txt` covers evidence
files except itself and this README.
