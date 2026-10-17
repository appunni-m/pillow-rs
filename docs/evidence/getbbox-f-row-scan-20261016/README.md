# F-mode GetBBox row scan — exploratory CPU candidate, 2026-10-16

Status: **retained partial CPU improvement; the 2× prioritization target is
still open.** This is one dense F-mode workload on a dirty Apple ARM64
checkout. It does not establish GetBBox across modes, patterns, sizes, SIMD,
Parallel CPU, or GPU.

## Frozen workload and parity

- Workload: `pil-image-image.getbbox.materialized.f-filled-1024x768`
- Parity case: `PIL.Image.Image.getbbox.nuanced.performance-f-filled-1024x768`
- Input: F 1024×768, all samples 1.0; setup is outside the measured call.
- Boundary: public `getbbox` call; five warmups, 20 iterations × five samples,
  warm cache, concurrency one.
- Oracle: Pillow 12.2.0 on CPython 3.12.
- Each report records a passing exact parity gate, actual serial CPU execution,
  terminal completion, and no fallback.

The workload input is in
[`benchmark input`](../../../pillow-rs/tests/fixtures/inputs/benchmark/getbbox-f-filled-1024x768.json)
and [`parity input`](../../../pillow-rs/tests/fixtures/inputs/parity/getbbox-f-filled-1024x768.json).
The copied manifest and fixture builder preserve how this one-off workload was
constructed. Existing `nonzero-f`, `blank-f`, and negative-zero semantics are
also relevant guards; the prior signed-zero correction is documented in
[`getbbox-f-signed-zero-20261015`](../getbbox-f-signed-zero-20261015/README.md).

## Candidate progression

All latency values are medians in milliseconds. The first two readings are
pre-change controls; candidate runs were not bracketed by a full A–B–B–A
sequence, so the large before/after movement is diagnostic rather than a
source-matched causal claim.

| Variant | Run | Pillow | Serial CPU | Pillow / CPU | Decision |
|---|---|---:|---:|---:|---|
| Original F scan | `479f681c586b40c1934d8f132839778a` | 0.002625 | 0.878563 | 0.003× | Control |
| Original F scan repeat | `7ede444297c34298a1e52b45a27070eb` | 0.002833 | 0.986271 | 0.003× | Control |
| Two-ended row scan | `a45fe28c27294b3e80ee1a942e0f2299` | 0.002291 | 0.002792 | 0.821× | Rejected; slower than Pillow |
| Row scan plus full-width early stop | `644b1ae0870f4b109f582e6db7bebc82` | 0.002209 | 0.001417 | 1.559× | Retained partial candidate |
| Four-corner early return | `034f67c5dee649a2a16f4deabb1fc934` | 0.002292 | 0.001541 | 1.487× | Rejected; did not improve on row scan |
| Borrowed mode lookup | `fb746f047564446fa822f0bc2b905dd3` | 0.002292 | 0.001500 | 1.528× | No gain established; helper removed |
| Borrowed mode lookup repeat | `094b2c2e809c454dac41426fc54b508a` | 0.002250 | 0.001625 | 1.385× | No gain established; helper removed |

The retained implementation checks each F row inward from both ends. When the
global horizontal bounds reach the full width, it stops the forward scan and
reverse-scans to locate the final nonzero row. Its predicate tests the raw
32-bit sample word, preserving Pillow's behavior that `-0.0` is data while
positive zero is empty. The I-mode implementation is unchanged.

The best measured candidate was 1.559× Pillow; the last two measurements were
1.528× and 1.385×. The ≥2× priority is therefore unmet. No SIMD, Parallel CPU,
or GPU timing was collected, and no sustained throughput is claimed. Receipt
resource counters report no temporary host allocation for the operation, but
they do not count the Python tuple returned by the public call; latency includes
the complete call boundary.

## Identities and retained artifacts

The source revision was `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` with a dirty
worktree. The workload input SHA-256 is
`051e267fe7038601231ae7908e1dec9af09acb788d15d400d6ed7a392d76003a`; the parity
input is `e75468a6cfebceaffb9b00161dc011297e27d989a6d9babc61ebdf88db81348d`.
The candidate run IDs are:

- `migration-benchmark-644b1ae0870f4b109f582e6db7bebc82` — retained row scan.
- `migration-benchmark-034f67c5dee649a2a16f4deabb1fc934` — rejected corner test.
- `migration-benchmark-fb746f047564446fa822f0bc2b905dd3` and
  `migration-benchmark-094b2c2e809c454dac41426fc54b508a` — borrowed lookup
  probe, removed due to no established gain.

Benchmark reports, raw samples, producer/build receipts, parity reports,
attestations, and the one-off manifest are retained in this directory. Kata
tracks the operation in `pillow-rs#pd3w`. The operation remains open.
