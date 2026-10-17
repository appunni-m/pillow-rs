# RGB Color CPU conversion-fusion trial — not retained — 2026-10-16

This focused experiment tried fusing native RGB `Grayscale` followed by
`Convert(RGB)` inside `CpuPool` for the eager `ImageEnhance.Color` constructor.
The code was removed after one whole-call run failed to establish a repeatable
improvement. It earns no operation completion credit and does not update the
published per-operation matrix.

## Frozen scope and identity

- Workload: `pil-imageenhance-color.enhance.material-rgb-520x512-factor-zero`
- Full public boundary: `whole_workflow`, including image setup, `Color(image)`,
  `enhance(0.0)`, and `tobytes` materialization.
- Parity cases, serial CPU: `PIL.ImageEnhance.Color.enhance.nuanced.audit-RGB-varied-0.3` and `PIL.ImageEnhance.Color.enhance.nuanced.coverage-batch-enhance-large-color-rgb-0`.
- Exact parity: 2/2 selected cases passed. The 17×11 varied RGB case observed
  `Grayscale` and `Convert` during construction and `BlendModule` at terminal
  materialization on CPU; the large case recorded the constructor conversion
  at `setup-color-3`.
- Measurement: warm cache, 5 warmups, 20 iterations × 5 samples (100 samples),
  concurrency 1; Pillow 12.2.0, CPython 3.12.13, macOS 15.7.7 ARM64.
- Source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty worktree);
  experimental native extension SHA-256 `30d191dc64cf543c7368ac377b2453314aad41d8423f3c05080173cd545c6de7`.
- Benchmark run `migration-benchmark-26b63217fc124e70b515e87ff8cc30bd`; reused exact parity run `migration-parity-b426c9aaf2594dcb9b3313a9142df58a`.

## Result

| Subject | Median latency | p95 latency | Median concurrency-one throughput |
| --- | ---: | ---: | ---: |
| Pillow | 0.524021 ms | 1.041834 ms | 1908.4 op/s |
| Serial CPU candidate | 0.507499 ms | 0.548208 ms | 1970.4 op/s |

The candidate measured 1.033× Pillow by median latency. Concurrency-one
throughput is reciprocal latency, not sustained throughput. The CPU receipt
reported actual backend `cpu`, terminal completion, and no fallback. At
`setup-color-3`, the receipt included `Grayscale` and `Convert`, reported two
fused logical operations, and recorded one 798,720-byte
output allocation. This confirms the CPU fusion path executed for the measured
constructor; it does not demonstrate a performance win.

The earlier control A1/A2 measurements for this same workload were 0.360813
and 0.981667 ms CPU while Pillow medians ranged from 0.328521 to 0.346876 ms.
That large drift, plus the new candidate's small 1.03× Pillow margin, makes a
causal gain unproven. The planned threshold is at least 2× Pillow before credit;
this trial is rejected, and no further A/B/B/A build cycle is justified here.

## Receipt nuance

The original whole-workflow receipt only named setup `PutPixel`, which made
constructor attribution ambiguous. A targeted `observed_steps` parity run with
`--timing-step setup-color-3 --timing-step observe-result` proved the constructor
pair. Step-scoped receipt evidence should be used when eager constructor work
is otherwise collapsed into a later workflow receipt. The large factor-zero
terminal receipt retained a `PutPixel` event; no `BlendModule` attribution is
claimed for that case.

Compressed reports, raw samples, parity result, backend receipts, and identity
attestations are stored beside this README. Verify artifact bytes with
`shasum -a 256 -c SHA256SUMS.txt`.
