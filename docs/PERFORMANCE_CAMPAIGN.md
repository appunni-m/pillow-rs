# Performance campaign

Status: active. This report records the operation-wide baseline and the focused
equalize and transpose work. It is a dated performance snapshot, not a support declaration.
The selected contract lives in the [parity manifest](../pillow-rs/tests/fixtures/manifest.yaml).

The goals are per operation: CPU latency at or below Pillow, SIMD latency at
least 5× faster than Pillow, and GPU latency no slower than SIMD with higher
throughput. Every comparison requires equal output, the same timing boundary,
and a receipt naming the backend that actually ran. Pipeline latency,
materialized-operation latency, and changing-input throughput are separate
measures. A pass at one boundary does not substitute for another.

Spend at most three or four optimization attempts on one operation per visit.
Then commit a checkpoint with the retained changes, parity evidence, measured
gaps, and concrete remaining investigations, and move to the next ranked
operation. A checkpoint is not completion: unmet targets stay in the matrix
and can be revisited after other operations receive their first pass.

## Operation-wide baseline

On 2026-09-24, the unchanged standard benchmark completed all 768 indexed
workloads across Pillow, CPU, SIMD, and GPU. Its receipt is
`migration-benchmark-3690266aab224ccfb59681959893a003`, retained locally as
`build/migration-parity/perf-all-20260924-baseline.json`. The existing parity
preflight passed 238 comparisons. Of the workload gates, 230 require matching
output and 538 require only successful execution. The latter remain diagnostic
timings until exact output is compared on each requested backend.

The manifest contains 208 operations plus the `MAX_STRING_LENGTH` constant;
206 operations have benchmark mappings. `toqimage` and `toqpixmap` have no
workload. Public exports and aliases outside this selected manifest still need
classification; this count does not establish an exhaustive public API audit.

| Diagnostic latency comparison | Workloads missing the target | Timed workloads |
| --- | ---: | ---: |
| CPU ≤ Pillow | 421 | 768 |
| SIMD ≤ Pillow / 5 | 740 | 768 |
| GPU ≤ SIMD | 609 | 768 |

These are observed workload ratios, including rows without native-backend
proof. They are not counts of completed operations. Backend receipts, exact
parity on the measured inputs, and build provenance must also agree. The run
records a dirty worktree; its source revision alone cannot identify all local
changes. The benchmark's reciprocal-latency throughput does not prove sustained
concurrent throughput.

`scripts/report_optimization_goals.py` generates the complete manifest matrix
and an investigation order without dropping unmeasured or unproven rows. The
current local outputs are `build/migration-parity/optimization-goals.json` and
`build/migration-parity/optimization-goals.md`. See
[the reporting command](BENCHMARKING.md#correctness-gate-and-budget-gate).

## Equalize optimization

The baseline ranked tiny GPU equalize workloads first: RGB 32 × 32 took
17.575250 ms on GPU versus 0.0130625 ms on SIMD. Inspection found a quadratic
LUT shader: one invocation rescanned the 256-bin histogram and recomputed a
prefix sum for every LUT entry. Computing channel steps once and advancing an
exclusive prefix sum reduced that workload to 0.396667 ms (44.3× faster).
RGB 1024 × 768 improved from 25.372584 to 6.769792 ms (3.75× faster).

The linear-pass receipt is `migration-benchmark-84c7c5adad5a422fb2d932fa5a687d51`.
All 160 equalize parity cases plus eight equalize benchmark workflows passed on
CPU, SIMD, and GPU before and after the change: 504 comparisons in each run.
The accelerated timing rows record completed GPU execution without fallback.
GPU latency still exceeds SIMD. Further changes accumulate histograms within
workgroups, combine repeated samples before atomic updates, and construct the
LUT with a parallel integer prefix scan. Synchronized diagnostic batches show
that the serial LUT pass still dominated after removing its quadratic work.
These diagnostic batches do not establish public latency or sustained request
throughput. Reused native timestamp queries returned inconsistent durations in
the scratch diagnostic, so those timestamp values were discarded.

The mask audit exposed an independent correctness bug: `equalize_with_mask`
validated the mask and then dropped it. Random L/RGB inputs failed 18 of 24
strict comparisons, while full-mask controls passed. The fix retains a masked
equalize descriptor and uses nonzero mask samples to build each backend's
histogram. The GPU derives the selected population from the histogram prefix,
not image dimensions. The resulting LUT applies to every output pixel.

CPU equalize now shares the native L/RGB LUT builder with SIMD, removing the
grayscale-to-RGB conversion and the duplicate scalar LUT implementation.
Twenty-four permanent input-only cases exercise L/RGB/palette images with
empty, full, partial, and one-bit masks. Existing assertions and workloads are
unchanged; one masked benchmark is added. The expanded focused cohort passes
582 live Pillow comparisons. Another 156 strict-backend comparisons pass on
mask cases and random/repeated data around dispatch-size boundaries, including
images above the workgroup cap. These results cover equalize, not the whole API.

The next unchanged-policy run is
`migration-benchmark-782af844abc44a638dbe21de80601c44`, retained as
`build/migration-parity/perf-equalize-20260924-mask-fix.json`. It measures the
same nine workloads plus the added masked case. Median milliseconds for the
nine materialized native-execution rows are:

| Equalize workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB 16 × 16 | 0.056125 | 0.012480 | 0.013167 | 0.331084 |
| Masked L 64 × 64 | 0.060917 | 0.033125 | 0.039834 | 0.558396 |
| RGB 32 × 24 | 0.048521 | 0.011980 | 0.011959 | 0.526063 |
| RGB 1 × 1 | 0.045688 | 0.010813 | 0.011021 | 0.219000 |
| RGB 32 × 32 | 0.049750 | 0.011626 | 0.011938 | 0.315230 |
| RGB 256 × 256 | 0.187313 | 0.060209 | 0.056730 | 0.427271 |
| RGB 1024 × 768 | 2.065146 | 1.640354 | 1.212292 | 1.732188 |
| `pipeline-chain.matrix-012` | 0.066271 | 0.019417 | 0.021334 | 0.539917 |
| Equalize → autocontrast → invert | 0.065271 | 0.019917 | 0.020812 | 0.476584 |

The remaining standard row observes deferred construction without native
backend execution and cannot establish an acceleration target. All timed native
rows record their requested backend without fallback. The latest small RGB
32 × 32 and large RGB 1024 × 768 GPU observations are respectively 55.8× and
14.6× faster than the initial GPU baseline. CPU meets the latency target on
these samples; SIMD's 5× goal and GPU's latency goal still fail. No sustained
equalize throughput claim is established. Paired reruns varied materially, so
these individual medians are diagnostic observations, not stable guarantees.

The CPU/SIMD histogram was then changed to combine blocks of 16 identical
pixels and alternate four independent counter banks for larger inputs. Small
inputs keep one bank to avoid extra initialization and reduction. Diagnostic
histograms compared uniform, block-repeated, random, and skewed distributions;
every bin matched the scalar counter. The public equalize cohort again passed
582 comparisons, plus 204 strict CPU/SIMD comparisons covering block tails,
internal changes within otherwise repeated blocks, and the bank-size boundary.

Receipt `migration-benchmark-e325fe378054470db26e7e010966e9c7` retains all ten
equalize workloads and their policies. On the uniform RGB 256 × 256 workload,
CPU/SIMD medians became 0.028167/0.027584 ms against Pillow's 0.192396 ms.
At 1024 × 768 they became 0.387563/0.440666 ms against 2.831771 ms. SIMD exceeds
5× on those two samples, but smaller and masked workloads still miss the goal.
GPU remains slower than SIMD. Results on these identity-LUT inputs do not prove
performance on varied inputs or sustained throughput.

### Equalize checkpoint and remaining gaps

This visit retained four performance changes: remove repeated GPU prefix
rescans, parallelize the GPU scan and local histogram, combine CPU histogram
updates, and fill SIMD lookup lanes. The separate mask correction is required
parity work. Equalize is checkpointed as incomplete; the next ranked operation
is `PIL.ImageFont.FreeTypeFont.font_variant`.

The last SIMD change handles L with one vector lookup and RGB in blocks of
sixteen complete pixels with three full lookup vectors. The previous loop
performed four lookups per sixteen input bytes, including unused channels.
All 698 selected equalize/autocontrast/eval/point comparisons passed against
live Pillow. Another 204 strict CPU/SIMD comparisons passed on masks, vector
tails, row boundaries, and histogram bank boundaries. The retained artifacts
are `perf-lut-20260924-simd-lanes-parity.json` and
`perf-equalize-20260924-simd-lanes-tails-parity.json`.

The unchanged changing-input diagnostic then completed 40,320 exact output
checks, including 38,400 measured completions. Source hashes and runtime
binaries remained consistent during the run. The receipt is
`equalize-throughput-20260924-simd-lanes.json`; its predecessor is
`equalize-throughput-20260924-cpu-banks.json`. Rates below are completed fresh
1024 × 768 requests per second, including construction, transfers,
synchronization, materialization, scheduling, and receipt capture.

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1594.2 | 2596.0 | 2549.3 | 481.7 |
| L | 2 | 2596.5 | 4564.7 | 4140.6 | 693.3 |
| L | 4 | 3771.7 | 7325.0 | 5407.8 | 952.7 |
| RGB | 1 | 407.5 | 688.7 | 845.7 | 525.4 |
| RGB | 2 | 682.5 | 1311.8 | 1178.6 | 765.1 |
| RGB | 4 | 964.7 | 1995.3 | 1526.8 | 1052.0 |

SIMD throughput increased 39–87% over the preceding implementation across these
six cases. At queue depth one, median SIMD request latencies were 0.3771 ms
for L and 1.1085 ms for RGB, versus Pillow's 0.6115 and 2.3771 ms. This is
1.62× and 2.14×, still below 5×. CPU was faster than Pillow on these samples.
GPU was slower than the improved SIMD path in both request latency and
throughput at every tested depth.

Remaining blockers to completing equalize are the SIMD latency gap on varied,
small, and masked inputs; GPU dispatch/transfer/completion overhead relative to
the improved SIMD path; and unproven scope outside these cohorts. A later visit
should profile histogram versus lookup versus allocation cost, inspect row
scheduling under concurrent requests, and measure GPU host/device phase costs.
These are investigations, not demonstrated hardware impossibility. Cross-runtime
checks and the repository's pre-push verification remain pending for this
checkpoint. No coverage collection was run.

## Transpose verified behavior

On 2026-09-24, the release extension passed the focused 368-case transpose
cohort on CPU, SIMD, and GPU: 1,104 exact Pillow comparisons. Strict GPU receipts
recorded 366 applicable hardware executions, one exact host semantic control,
and one case outside pipeline telemetry. The initial sandbox run had no
enumerated adapters and returned adapter-unavailable errors; rerunning with
host GPU access passed without changing source, cases, or assertions.

The wider input corpus currently contains 11,060 parity cases, 24 coverage
plans, and 769 benchmark workloads. Those counts describe indexed inputs, not
fresh full-corpus evidence. No coverage collection was run for this campaign.

The WASM metadata adapter needed a correction for retained `Image.info`
references. Pillow 12.2.0 omits WebP `timestamp` and `duration` immediately
after open, then adds them to the same dictionary during load. A workflow
that captures that dictionary before transpose observes the added fields
when it serializes after transpose. An early adapter snapshot loses those
updates. Keep the mutable mapping alive and synchronize actual decoder
metadata; adding the fields to the Rust image before load changes the public
behavior and is not a valid fix. The existing parity inputs remain unchanged.

## Current latency results

The maintained RGB 1024 × 1024 quick pipeline has these median end-to-end
latencies in milliseconds, including the same public input and output boundary
for every subject. Its five warmups, 20 iterations, and five samples are
unchanged. Receipt: `migration-benchmark-ade95333bc8b4b1cbb165c625c1b4a12`.

| Subject | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Median latency (ms) | 2.204562 | 0.607376 | 0.515584 | 0.865333 |

SIMD is 4.276× faster than Pillow on this pipeline, short of 5×. GPU latency
is 1.68× SIMD latency. This pipeline does not prove the per-operation goal.

The 12-case materialized-operation cohort retains the same measurement policy
and exact correctness gate. Receipt:
`migration-benchmark-935f72d9e020466bbca3b7699f7c120a`.

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| rgb-513x515-rotate90 | 0.261250 | 0.173395 | 0.167500 | 0.541563 |
| rgba-515x513-rotate90 | 0.247208 | 0.175042 | 0.184417 | 0.705062 |
| rgba-9x31-transpose-mirror | 0.007042 | 0.007583 | 0.007917 | 0.192959 |
| rgb-768x772-rotate90 | 0.617938 | 0.340542 | 0.275875 | 0.889833 |
| rgba-772x768-transpose-mirror | 0.986750 | 0.406542 | 0.355187 | 0.942895 |
| rgb-768x772-rotate-cancel-mirror | 1.357229 | 0.308813 | 0.204417 | 0.908208 |
| rgb-513x515-mirror | 0.253687 | 0.163042 | 0.079563 | 0.516437 |
| rgb-513x515-flip | 0.200458 | 0.161271 | 0.082666 | 0.513437 |
| rgb-513x515-rotate180 | 0.244750 | 0.165542 | 0.071667 | 0.509563 |
| rgba-515x513-rotate180 | 0.253146 | 0.174375 | 0.149375 | 0.657500 |
| rgba-768x769-rotate90 | 0.636854 | 0.376312 | 0.342041 | 0.989750 |
| rgba-1025x1023-transpose-mirror | 1.940292 | 0.700625 | 0.635520 | 1.018125 |

All 24 paired workloads passed their correctness gates. The small materialized
RGBA case still misses the CPU target: 0.007583 ms versus Pillow at 0.007042 ms.
Its whole-workflow measurement is CPU 0.014750 ms versus Pillow 0.015146 ms
(receipt `migration-benchmark-8867e909c2ca4db183374c414b379249`). These results
show why construction, dispatch, and terminal materialization costs must be
measured separately from kernel work.

## Changing-input throughput

The maintained public diagnostic creates a fresh image and graph for every
request, uses 16 changing inputs per window, and checks every output against
live Pillow bytes outside the measured window. It ran queue depths 1, 2, and 4
with five warmup windows and five samples of 20 measured windows. It checks
40,320 outputs, including 38,400 measured completions. Rates are completed
images per second. Receipt: `perf-20260922-coalesced-write-throughput.json`.

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| RGB | 1 | 442.5 | 1434.3 | 1663.5 | 785.8 |
| RGB | 2 | 607.7 | 1838.1 | 1996.1 | 1140.0 |
| RGB | 4 | 744.2 | 1807.1 | 1892.0 | 1222.6 |
| RGBA | 1 | 526.6 | 1042.6 | 1181.6 | 740.7 |
| RGBA | 2 | 722.6 | 1298.7 | 1287.8 | 946.9 |
| RGBA | 4 | 846.8 | 1152.0 | 1443.2 | 1059.9 |

GPU throughput remains below SIMD at every tested depth. These measurements
are host request throughput; they do not claim simultaneous GPU kernels.

## Optimization findings

The public path includes input decoding or construction, lazy operation
planning, backend dispatch, storage conversion, and terminal output. Measure
those boundaries before changing an inner loop. The tiny materialized case is
near the fixed dispatch and allocation floor, while large GPU cases still pay
substantial device synchronization and readback costs.

- **CPU:** Avoiding an unnecessary owned copy at `frombytes` and using
  orientation-aware row segments reduced memory movement. A simple loop reorder
  did not help the odd-sized RGBA case; hoisting stride and orientation
  arithmetic out of per-pixel work did. The remaining tiny-case miss points to
  fixed call, allocation, and materialization overhead rather than a large
  pixel kernel.
- **SIMD:** The transpose path uses native pixel tiles and a 16-pixel
  collection callback over four-row scratch. Tile admission, arbitrary cache
  access, payload bits, edge tails, and scalar fallback are correctness
  boundaries. The measured 4.276× quick result shows callback and memory-layout
  work helped, but more of the public path must be removed or fused to reach
  the 5× target consistently.
- **GPU:** Packed RGB and tiled shaders coalesce output writes; fusion reduces
  two public transposes to one dispatch; bounded pool reuse reduces resource
  churn. Direct-mapping experiments did not show a consistent public latency
  win. A phase diagnostic measured output mapping at 245.771 µs and host output
  copying at 122.209 µs for RGB, with both higher for RGBA. Device completion,
  mapping, and readback remain the first large-image costs to attack. Small
  cases cannot amortize those fixed costs.
- **Parity:** Compose transform geometry using the dimensions produced by each
  preceding operation, not the original dimensions. Preserve bit patterns,
  native modes, output shape, alpha rounding, metadata behavior, and public
  materialization boundaries when fusing or returning identity operations.
  Unaligned tails and unsupported layouts must stay on the exact fallback.

Transpose remains pending: lower GPU wait/map/readback time while keeping
transfer and dispatch receipts truthful, continue SIMD gains along the measured
public path, and remove the CPU tiny-case overhead. The campaign has moved on
to give other operations bounded optimization attempts. Current measurements
do not meet the complete performance goals.
