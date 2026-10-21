# Grayscale CPU/SIMD ownership baseline

This focused rebaseline checked the grayscale family move and separated
architecture-specific vector kernels from serial CPU execution. It does not
claim a Grayscale optimization item complete. The family follows the single
repository-wide path standard in
[`IMPLEMENTATION_STRUCTURE.md`](../../../IMPLEMENTATION_STRUCTURE.md); legacy
areas are aligned incrementally when their ownership or execution path changes.

## Family layout

The common Pillow-compatible scalar conversion remains in
`pillow-rs/src/color.rs`. CPU mode handling and native YCbCr scalar extraction
now live in `pillow-rs/src/compute/pool_cpu/ops/grayscale.rs`. x86 AVX2 RGB and
SSSE3 YCbCr loops live in
`pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/`; ARM NEON
remains in the corresponding grayscale family kernels. CPU registration points
to the CPU family module. The SIMD adapter and its mode-specific kernels remain
separate, with no new generic dispatch in pixel loops.

The move corrected hidden execution: before it, the shared converter invoked
AVX2 in the serial CPU profile and the CPU YCbCr path reused SSSE3. Receipts
called those paths CPU with zero vector blocks. Now CPU receipts are scalar and
SIMD receipts name actual vector execution.

## Frozen workload evidence

Both workloads used 1024×768 noise inputs, warm release execution, five
warmups, 20×5 measured samples, concurrency one, and the
`call;observe-result` boundary. Each report ran fresh parity on the RGB and
YCbCr material cases for both CPU and SIMD; all four comparisons passed on each
host. Ratios are same-report Pillow median divided by the profile median.

| Host | Mode | Pillow | CPU | SIMD | SIMD execution receipt |
| --- | --- | ---: | ---: | ---: | --- |
| ARM64 | RGB | 0.245750 ms | 0.101875 ms (2.41×) | 0.104292 ms (2.36×) | vector, 49,152 blocks |
| ARM64 | YCbCr | 0.171959 ms | 0.0619585 ms (2.78×) | 0.0544375 ms (3.16×) | NEON deinterleave, 49,152 blocks |
| x86_64 | RGB | 1.507462 ms | 1.462812 ms (1.03×) | 1.012146 ms (1.49×) | vector, 49,152 blocks |
| x86_64 | YCbCr | 1.156526 ms | 0.845376 ms (1.37×) | 0.819206 ms (1.41×) | SSSE3 deinterleave, 49,152 blocks |

Exact workload IDs:

- `pipeline-op.grayscale.material-rgb-noise-1024x768`
- `pipeline-op.grayscale.material-ycbcr-noise-1024x768`

Exact parity cases:

- `PIL.ImageOps.grayscale.nuanced.performance-large-rgb-noise-1024x768`
- `PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`

## Validation

- `cargo check -p pillow-rs --lib` passed.
- The moved CPU grayscale contract tests passed 4/4 on ARM64 and x86_64.
- The x86 AVX2 kernel test passed exhaustive RGB and tail checks (2²⁴ inputs).
- `make operation-source-crosswalk-test` passed 11/11; the generated report
  contains a CPU registry edge to `pool_cpu/ops/grayscale.rs` and no unresolved
  Grayscale registration finding.
- The ARM64 and x86_64 focused benchmark reports each passed 4/4 parity checks
  before timing.
- No coverage ran.

Run IDs: ARM64 `migration-benchmark-c90c923d70e240639b9906b1a138c0fb`; x86_64
`migration-benchmark-c50d14afd82444fcb8bb01536f5f4092`. Both sources were
dirty at revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`; these are local
diagnostics, not clean published-snapshot rows. Per-subject rows are in
[`performance-optimization-local-runs.csv`](../../performance-optimization-local-runs.csv).
Ignored benchmark reports and raw samples remain under each host's
`build/migration-parity/grayscale-family-...-20261019/` directory.

## Crosswalk contract repair

The first regenerated source crosswalk flagged the new CPU registration as
unresolved. The parser recognized grouped imports from `ops::imageops::{...}`
but missed direct leaf imports such as `ops::grayscale::op_grayscale`. The
parser now resolves the final segment, with a focused regression case. The
generated inventory maps `registry-key:Grayscale` to
`pool_cpu/ops/grayscale.rs`; the focused crosswalk suite passed 11/11.

## Rejected direct-store probe

An x86 AVX2 probe wrote complete vectors directly into the final output buffer.
Its RGB median was 0.996757 ms versus 1.012146 ms for the control (1.5% lower),
with exact parity and positive vector receipts. This was one unbracketed cohort,
so the change was reverted and recorded as inconclusive. Its rows are labeled
`inconclusive_reverted` in the local run ledger. The exhaustive AVX2 unit test
still checks all 2²⁴ RGB values and bounded/tail behavior.

The x86 RGB CPU margin is narrow; both x86 SIMD rows remain below the 2× Pillow
target. GPU, Parallel CPU, sustained throughput, other modes, and other sizes
remain open. No coverage, commit, or push ran; the commit/push batch remains
0/33.

## Rejected eight-pixel YCbCr CPU block probe

The serial CPU helper in `pool_cpu/ops/grayscale.rs` was tested with three
bounded 64-bit loads per eight YCbCr pixels, replacing the current four-pixel
12-byte extraction block. This kept the existing family path and did not touch
the SSSE3 SIMD kernel. The focused Rust contract test covered 1–17 pixels and
passed. The candidate source SHA-256 was
`c2a5a004fa6c0bcc32a20353143e7f7c17fb712e145b41d9e5a734cc48342281`; the
restored control source SHA-256 is
`9ef2e0694fa43c8cf33a824e06b9c5205378ff8aee2385c439f6a5a6a603f48d`.

All four cohorts used the same 1024×768 YCbCr workload, warm release public
call plus observed-result boundary, 5 warmups, 20×5 samples, and concurrency
one. Each report selected and passed both CPU and SIMD parity comparisons.
Every target receipt completed without a fallback; CPU reported the serial CPU
path, while SIMD reported `ssse3-deinterleave` with 49,152 vector blocks.

| Cohort | Pillow | CPU | CPU speedup | SIMD | SIMD speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| Control A1 | 1.168509 ms | 0.843903 ms | 1.385× | 0.828162 ms | 1.411× |
| Candidate B1 | 1.179919 ms | 0.883424 ms | 1.336× | 0.836424 ms | 1.411× |
| Candidate B2 | 1.189694 ms | 0.891045 ms | 1.335× | 0.861470 ms | 1.381× |
| Restored control A2 | 1.220060 ms | 0.910985 ms | 1.339× | 0.878161 ms | 1.389× |

The raw medians drifted between A1 and A2, so compare the candidate to the
nearest restored control and normalize each run to its own Pillow median. The
candidate averaged 1.335× CPU speedup; restored A2 measured 1.339×, a 0.3%
candidate regression. The candidate also averaged 0.887234 ms CPU latency versus
0.877444 ms for the two controls. This is not a demonstrated gain, so the
eight-pixel implementation was rejected and the four-pixel control restored.
The SSSE3 source was unchanged; its measured variation followed the raw cohort
drift and is not attributed to the CPU candidate.

Run IDs: control A1 `migration-benchmark-039ce0d795a3469fa70eda973ce0c73f`;
candidate B1 `migration-benchmark-2d7d367f640644409b8c3be5dd59ad4c`;
candidate B2 `migration-benchmark-6a02be6e80e94c1c98648041645f88e9`;
restored control A2 `migration-benchmark-096b3525d4c948a4a8f7f2b4fc2cc1a0`.
All four gate IDs and per-profile result rows are recorded in
[`performance-optimization-local-runs.csv`](../../performance-optimization-local-runs.csv).
