# GetProjection I/F row-scan candidate — 2026-10-15

## Scope and status

This is dirty-tree ARM64 diagnostic evidence for the I/F scalar row scan in
`pillow-rs/src/image.rs`. It is not a clean published matrix result, a matched
A/B/A attribution, or completion of the full GetProjection optimization target.
The source revision was `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` with
uncommitted changes. The current `image.rs` SHA-256 after operation-specific
receipt instrumentation is
`e8670886bbf370900487407a782d6bdbd0a262dfec55dedf6289108bcd2ba009`.

The scan processes scalar rows directly, sets each horizontal projection bit
only once, and after all columns are covered scans later rows only until finding
their first nonzero word. F samples use raw `u32` words so negative zero is
nonzero to match Pillow.

## Focused correctness

`make build-parity` completed before the focused measurements. The selected CPU
parity run passed 4/4 cases: F negative zero from a raw-byte input, I/F negative
nonzero controls, and the large scalar analysis workflow. After a static audit
found that no case reached the all-columns-covered early-out, two cases were
added and passed 2/2 CPU parity:

- `PIL.Image.Image.getprojection.parity.i-row-scan-after-horizontal-complete`
- `PIL.Image.Image.getprojection.parity.f-row-scan-after-horizontal-complete`

The added inputs make the first row nonzero in all columns, then include an
empty and sparse later row. No full parity or coverage campaign ran. The
current parity fixture SHA-256 is
`1fbb7a1e14e32a42a710153d63e1810c369685b04363b5f5ec8eb2a65026a331`.

The receipt-v2 benchmark preflight selected the exact scalar analysis comparison
`PIL.Image.Image.getcolors.nuanced.analysis-scalar-if-terminal-1024x768` and
passed 1/1 on CPU.

## Focused latency and execution evidence

The original row-scan cohort measured Pillow 12.2.0 and the target on the same
macOS 15.7.7 ARM64 host, warm cache, with 100 samples for each direct terminal
read. The boundary was the selected public `getprojection` call after input
materialization.

| Mode | Pillow median | Pre-candidate CPU | Original row-scan CPU | Receipt-v2 CPU | Receipt-v2 Pillow / CPU |
|---|---:|---:|---:|---:|---:|
| I | 0.3306665 ms | 0.453958 ms | 0.0112705 ms | 0.0130625 ms | 25.71× |
| F | 0.3715835 ms | 0.4939165 ms | 0.0124995 ms | 0.014396 ms | 25.60× |

Receipt-v2 selected only the I and F terminal-read workloads, with warm cache,
5 warmups, 20 iterations × 5 samples, and concurrency 1. The public
GetProjection call is eager on both subjects. Its target receipt records one
`GetProjection` operation on actual CPU, terminal complete, and no fallback.
Resource telemetry records three host buffers totaling 3,152,896 bytes and two
output allocations totaling 7,168 bytes. The strict benchmark preflight passed
the exact scalar-analysis parity comparison (1/1).

The earlier generic-receipt experiment had no GetProjection-specific operation
receipt or output allocation accounting; it is superseded for execution and
resource claims by receipt-v2. The original row-scan report's operation
telemetry listed Resize and Convert only, so its timing alone did not prove the
terminal operation. Receipt-v2 closes that operation-identity gap.

These remain dirty-tree local diagnostic timings, not clean published matrix
credit or matched A/B/A causal attribution. The original target-versus-control
shift and receipt-v2's observed Pillow ratios are measurements, not an isolated
row-scan speedup claim. The earlier whole scalar-analysis workflow was only a
six-sample guard: Pillow 6.9253955 ms, CPU 6.5939795 ms; treat it as
inconclusive workflow evidence.

The F signed-zero case also passed when SIMD and GPU were requested. Both
execution receipts show CPU fallback because I/F GetProjection is unsupported
on those profiles; these runs receive no SIMD or GPU credit. No SIMD speed or
GPU latency/throughput benchmark was run for I/F.

## Artifacts and tracking

This directory retains the pre-candidate control report, candidate reports and
samples, parity results, execution and producer attestations, and the strict
SIMD/GPU fallback receipts. The local-run ledger retains the earlier ten rows
and adds four receipt-v2 Pillow/CPU rows in
`../performance-optimization-local-runs.csv`. Kata tracking is
`pillow-rs#e1p3`; the row-scan/backend audits are comments 2531 and 2533, and
the improved operation receipt is documented in comment 2558. The operation
matrix remains pinned to its clean snapshot.
