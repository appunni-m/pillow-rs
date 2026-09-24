# Performance campaign

Status: active. This report records the operation-wide baseline and focused
optimization visits. It is a dated performance snapshot, not a support declaration.
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

## Font variant checkpoint

The next ranked workload, `pil-imagefont-freetypefont.font-variant.standard`,
includes constructing the original font and its variant. A fresh unchanged-policy
baseline measured Pillow at 0.040458 ms and the CPU profile at 3.930730 ms.
The SIMD/GPU profiles call the same font loader without accelerated execution
receipts; their timings cannot establish SIMD/GPU acceleration.

The behavior audit found that the Python wrapper reused the native handle's
original settings and bytes after public attributes or the source file changed.
The correction resolves the current public size/index/encoding/layout settings,
reopens path-backed sources, and preserves `font_bytes` for memory-backed sources.
Unreadable font paths now produce Pillow's `OSError("cannot open resource")`.
All 462 comparisons across the 154 existing ImageFont/FreeTypeFont cases pass.

The added live-reference check, `scripts/test_font_variant_parity.py`, exercises
14 source/state scenarios. Thirteen pass; the malformed-source scenario retains
a real unresolved parser defect: `b"invalid font data"` yields
`OSError("broken file")` in Pillow/FreeType 2.14.3 and `OSError("unknown file format")` through
the pinned fontdone dependency. This is an implementation failure, not a test
defect; the assertion remains failing. The reference BDF reader reaches EOF
without a complete first line and reports `Invalid_File_Format`. Correcting
driver probing/error precedence belongs in fontdone, not a special case in the
Pillow wrapper. Do not push this checkpoint as passing until that failure is
resolved and affected checks run.

A release-mode phase diagnostic against the pinned fontdone library measured
100 font opens after five warmups. For the benchmark's DejaVu Sans font, median
face opening took 1.949666 ms; library initialization took 0.000042 ms, requesting
the size 0.001125 ms, and destruction 0.001125 ms. The workflow opens two faces,
explaining almost all its latency. The smaller render-coverage font still spent
0.207541 ms opening its face. Receipt:
`font-variant-load-phases-20260924.jsonl`. These are component diagnostics, not
public latency or throughput claims.

After the wrapper correction, the maintained workflow measured CPU at
3.865917 ms versus Pillow at 0.037959 ms (receipt
`migration-benchmark-6ce93ba54ccf42c3aa3457a4571b3ffb`). The change addresses
parity; the large performance gap remains.

The remaining work is in fontdone's face-opening path: correct malformed-input
error precedence and reduce repeated parsing while keeping each face's mutable
state independent. A shallow `FT_Face` clone shares `Rc<RefCell<...>>` state and
cannot replace a newly opened variant. Any immutable parsing reuse must still
honor changed path contents, selected face, encoding, and variation reset rules.
No font-variant performance target is complete. Related font metadata workloads
also time font construction, so their shared loading bottleneck stays pending
while the campaign gives the next image operation its bounded optimization visit.

## Inversion visit

The next visit covers `PIL.ImageOps.invert` and its mapped pipelines. All 46
maintained workloads were measured without changing their policies (receipt
`migration-benchmark-f0162ac92e9440378daba035278bb815`). For RGB 256 × 256,
Pillow/CPU/SIMD/GPU medians were 0.118937/0.024230/0.102604/0.584146 ms.
For RGB 1024 × 768 they were 1.269625/0.647584/0.613250/1.942063 ms.
Single-sample long-chain rows remain noisy diagnostics; their declared policies
were not changed. No inversion kernel optimization has been applied yet.

Exact output comparison covered 85 public cases plus 43 mapped workflow cases
on all three backends. It passed 381 of 384 comparisons. The failing workflow
was `pipeline-chain.loaded-10.rgb-jpeg-512x384` on CPU, SIMD, and GPU. CPU/SIMD
pixels were exact, but crop dropped all four JFIF metadata fields. On GPU the
cropped result also differed in 8,539 bytes. The existing benchmark's successful
execution gate did not detect these differences.

Observing every intermediate step located metadata loss in crop's pipeline
materialization boundary, where retained image information was explicitly set
to `None`. The GPU pixel divergence begins at autocontrast, after inversion
itself has matched Pillow. Its integer LUT mapping is not equivalent to Pillow's
floating-point calculation: for a channel range of 164–255, input 255 becomes
254 in Pillow and 255 in the shader. The exact rational result cannot substitute
for the reference's floating-point rounding and truncation.

Evidence is retained in `perf-invert-20260924-initial-parity.json`,
`invert-loaded-jpeg-first-divergence-20260924.json`,
`invert-loaded-jpeg-gpu-divergence-20260924.json`, and
`invert-autocontrast-rounding-20260924.json`. Resolve these pipeline parity
failures before accepting inversion speedups. The SIMD loop's repeated padding
and copying of full vectors remains an untested performance hypothesis.

Retaining source header information when materializing a pipeline fixes the
crop metadata loss. The unchanged cohort then passes 383/384 comparisons:
all CPU/SIMD cases pass, and GPU retains only the autocontrast pixel mismatch.
The post-fix receipt is `perf-invert-20260924-crop-info-fix-parity.json`.
A Rust regression covers JPEG metadata across invert followed by crop; it is
added for the pre-push test run.

The GPU autocontrast shader now reproduces the reference's binary64 division,
separate rounded products, subtraction, and integer truncation using pairs of
32-bit integers. The bounded domain needs at most a 61-bit product. Each channel
finds its histogram bounds and scale once; all 256 lanes then build the LUT.
This requires no shader f64 support, host LUT computation, or extra GPU passes.

An isolated hardware diagnostic exercised the production arithmetic helpers on
all 16,777,216 byte/low/high combinations and matched independent scalar f64
evaluation. All 8,355,840 valid-range values also matched LUT bytes generated
by live Pillow's public masked autocontrast. The old integer formula differed
on 12,094 values. Receipt:
`autocontrast-gpu-exhaustive-pillow-20260924.jsonl`.

Twenty permanent input-only L/RGB cases cover interior rounding, endpoint
rounding, and masked values outside the selected range. The complete public
autocontrast shader path passes 585 focused comparisons, and the inversion
pipeline cohort passes all 384 comparisons. Another 60 comparisons exercise
the new cases under strict backend selection; all 20 GPU receipts record
completed hardware execution, four dispatches, and no fallback. Receipts:
`perf-autocontrast-20260924-binary64-parity.json`,
`perf-autocontrast-20260924-binary64-strict-parity.json`, and
`perf-invert-20260924-autocontrast-fix-parity.json`.

The first inversion speed change replaces padding/copying every vector with
the existing helper that loads complete vectors directly and pads only the
tail. XOR with the active-channel mask performs inversion and preserves alpha.
A paired release diagnostic compared the old and new loops over 32,768
channel/alpha/length combinations with identical output. The isolated loop
improved about 20–24× for buffers of 768 bytes through 2.25 MiB. This component
result, retained in `invert-simd-loop-20260924.jsonl`, does not establish public
latency or throughput targets.

### Inversion checkpoint and remaining gaps

This visit retained three changes: crop metadata preservation, exact GPU
autocontrast arithmetic for the failing composed pipeline, and direct full-block
SIMD inversion. It is checkpointed before taking on another bottleneck. Both
ImageOps and ImageChops inversion plus mapped workflows pass 477 comparisons;
another 252 strict-backend comparisons pass across vector tails, row boundaries,
alpha, and CMYK. The artifacts are
`perf-invert-20260924-simd-full-blocks-parity.json` and
`perf-invert-20260924-simd-full-blocks-tails-parity.json`.

All 46 maintained workloads retain their original policies in receipt
`migration-benchmark-798d49766c95438fb2f662193b33bcf1`. Selected median
milliseconds are:

| Invert workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB 1 × 1 | 0.033500 | 0.011396 | 0.011542 | 0.269042 |
| RGB 32 × 32 | 0.035959 | 0.011938 | 0.012021 | 0.263500 |
| RGB 256 × 256 | 0.118876 | 0.022604 | 0.023542 | 0.381938 |
| RGB 1024 × 768 | 1.226604 | 0.507917 | 0.583584 | 1.615667 |
| Invert → mirror, RGB 1024² | 2.322813 | 2.083501 | 0.732834 | 1.563208 |

The 256² SIMD median improves 4.36× from 0.102604 ms and is 5.05× faster
than Pillow in this run. The larger materialized case improves only 1.05×;
its public SIMD latency is still just 2.10× faster than Pillow. The component
speedup does not remove construction, copying, export, or scheduling costs.

The changing-input throughput run completed 40,320 exact output comparisons,
including 38,400 timed completions, with unchanged source hashes and consistent
runtime binaries. All GPU requests recorded one dispatch and complete transfers.
Receipt: `invert-throughput-20260924-simd-full-blocks.json`. Rates are completed
fresh 1024 × 768 requests per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 2762.1 | 5013.0 | 5027.9 | 526.2 |
| L | 2 | 4106.3 | 8372.6 | 8281.2 | 752.8 |
| L | 4 | 5095.4 | 10270.7 | 10374.2 | 985.4 |
| RGB | 1 | 606.4 | 1903.9 | 1861.5 | 618.4 |
| RGB | 2 | 936.2 | 2636.1 | 2453.0 | 998.7 |
| RGB | 4 | 1082.9 | 2627.6 | 2582.5 | 1214.9 |

At depth one, SIMD request medians are 0.189813 ms for L and 0.484417 ms
for RGB, versus Pillow's 0.347938/1.577541 ms: 1.83×/3.26×, below 5×.
GPU fails both the SIMD latency and throughput goals at every tested depth.
CPU is faster than Pillow for these standalone samples, but some composed
workloads still lose; the loaded JPEG ten-operation pipeline measures
3.875730 ms on CPU versus Pillow's 3.455834 ms.

Remaining investigations are the large-image terminal costs (allocation,
copies/export, and row scheduling), fixed overhead on small inputs, and GPU
format preparation/transfers/completion. The large standard SIMD workload
spends a 0.426542 ms median in the terminal phase; that includes execution and
export and does not identify one cause. The throughput profile also shows RGB
scaling plateauing between depths two and four. Profile those stages before
changing thresholds or adding workers. No full-operation target is complete,
and cross-runtime plus pre-push checks remain pending. The next bounded visit
is `PIL.ImageChops.blend`, ranked by the remaining baseline gaps after excluding
the already checkpointed equalize, inversion, and shared font-loading work.

## Blend visit

The next ranked operation is `PIL.ImageChops.blend`; its wrapper delegates to
module-level `Image.blend`. The four unchanged maintained workloads measured
under receipt `migration-benchmark-df26b330f3694581a5b4b63044f8ada3` include
three materialized pipelines and one deferred construction row. The materialized
16² workload measured Pillow/CPU/SIMD/GPU at
0.015605/0.022250/0.021438/1.123354 ms. The deferred row cannot prove acceleration.

The existing Chops cohort and mapped workflows pass 78 comparisons, but a
complete byte-pair audit exposed rounding defects for seven of thirteen tested
alpha values on every requested profile. CPU and SIMD used weighted f64 terms;
the failing GPU cases were CPU semantic fallback, as their receipts show.
Live Pillow's selected macOS build narrows alpha to float32 and uses fused
`left + alpha * (right - left)` arithmetic. Weighted f64 and separately rounded
float32 both differ on some extrapolating inputs. Evidence is retained in
`blend-arithmetic-20260924.json` and
`perf-blend-20260924-pairs-initial-parity.json`. The formula and float parameter
also appear in [Pillow's Blend.c](https://github.com/python-pillow/Pillow/blob/main/src/libImaging/Blend.c).
The live executable, rather than that moving source link, establishes the
observed fused rounding behavior.

CPU, SIMD, and GPU now use the same float32 fused interpolation. SIMD admission
requires hardware on which wide's vector multiply-add is fused; builds without
that feature remain an explicit acceleration gap instead of using its
non-equivalent two-rounding fallback. Cross-platform reference behavior and
those unsupported SIMD builds still require verification.

The former GPU admission check recomputed 65,536 byte pairs for every call to
compare two incorrect formulations. Matching the kernel arithmetic removes
that scan; admission now checks finite float32 alpha. Twenty-eight permanent
input-only regressions cover both public aliases, L/RGB, interpolation,
extrapolation, and alpha narrowing. No old assertion or workload changed.

After the arithmetic correction, both APIs and mapped workflows pass all 354
comparisons. The complete 65,536 byte pairs at thirteen alpha values pass all
39 backend comparisons, and every receipt records the requested hardware path
without fallback. Receipts: `perf-blend-20260924-fma-shared-parity.json` and
`perf-blend-20260924-fma-parity.json`. These exhaust byte pairs at the selected
alpha values, not the entire float32 alpha domain.

Maintained receipt `migration-benchmark-e1880f4d9d014f9e8bbc51d79103d2ba`
measures the materialized 16² GPU sample at 0.283896 ms, down from 1.123354 ms
(3.96×). The blend/difference/offset/invert pipeline drops from 3.086854 to
1.037084 ms. CPU/SIMD
materialized medians are 0.019855/0.018188 ms versus Pillow's 0.015083 ms;
none of the full-operation targets is complete.

The next change borrows native CPU input buffers, eliminating two input copies
and the L→RGB→L round trip. Its first public check found 34 empty-image failures
from using the ordinary raw-image constructor. The existing constructor that
permits empty images restores the intended boundary. The final build passes
all 354 shared comparisons and all 39 exhaustive byte-pair comparisons again;
the latter record the requested backend without fallback. Artifacts:
`perf-blend-20260924-native-final-shared-parity.json` and
`perf-blend-20260924-native-final-pairs-parity.json`.

### Blend checkpoint and remaining gaps

This visit retains fused interpolation, constant-time GPU admission, and native
CPU buffers, including the empty-image correction. All seven maintained
workloads mapped to the two public aliases were measured with their original
policies in `migration-benchmark-770e6c3c92ed49bda072f2a41654eaa7`. Two rows
observe deferred construction and have no native execution proof. The five
materialized rows have completed receipts for their requested backends.

For the materialized Chops 16² sample, final Pillow/CPU/SIMD/GPU medians are
0.015292/0.017792/0.019438/0.515271 ms. CPU improves 1.25× from its initial
0.022250 ms but remains slower than Pillow. The corresponding GPU sample
improves 2.18× from 1.123354 ms; its earlier 0.283896 ms observation shows
substantial run-to-run variability and is not a stable latency guarantee.

The two-image throughput diagnostic includes constructing both fresh images,
blend at alpha 0.3, transfers, materialization, scheduling, and receipt capture.
It passes 40,320 exact output checks, including 38,400 timed completions, with
unchanged source hashes and consistent binaries. GPU receipts include the
primary upload, auxiliary image, dispatch, and readback. Receipt:
`blend-throughput-20260924-native-final.json`. Completed 1024 × 768 image pairs
per second are:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1850.4 | 3628.1 | 1394.1 | 332.2 |
| L | 2 | 1961.7 | 4037.6 | 1415.9 | 324.8 |
| L | 4 | 1911.2 | 4074.9 | 1395.2 | 321.5 |
| RGB | 1 | 354.7 | 1335.5 | 428.8 | 342.8 |
| RGB | 2 | 394.0 | 1433.4 | 438.3 | 351.2 |
| RGB | 4 | 388.8 | 1459.0 | 441.5 | 355.2 |

Depth-one SIMD request medians are 0.708979/2.272396 ms for L/RGB, against
Pillow's 0.528438/2.790312 ms. SIMD is slower than Pillow for L and only 1.23×
faster for RGB. CPU medians are 0.251271/0.645500 ms; GPU medians are
2.991396/2.781646 ms. GPU misses both SIMD targets at every depth. Increasing
host concurrency barely improves SIMD/GPU throughput, so adding more requests
alone does not solve the gap.

The next visit should profile SIMD widening/packing and its serial pixel loop,
shared host/secondary-image materialization boundaries, and GPU transfer and
completion phases. It must also establish cross-platform fused arithmetic,
restore exact native SIMD execution where hardware FMA is absent, and expand
maintained size/mode workloads beyond the current tiny latency rows. These
remain investigations rather than proven bottlenecks. No blend target is
complete; pre-push and cross-runtime checks remain pending. The campaign moves
to the next ranked operation, `PIL.ImageChops.add`.

## Add visit

This visit uses four bounded attempts: restore affine arithmetic and argument
parity; replace inaccurate device division with a parameter table; restore
unequal-size clipping; then simplify default CPU arithmetic and SIMD block
loads. Subtract shares the affine implementation, and the bytewise helpers also
serve min/max, difference, modulo, and logical Chops operations.

The first exhaustive live-Pillow comparison failed 20 of 102 comparisons.
Eighteen had wrong bytes because the CPU/SIMD formulas used binary64 instead
of Pillow's binary32 scale, division, and offset addition. Two rejected the
zero divisor under strict SIMD selection. Python offsets also need the integer
index protocol and C-int range checks, rather than arbitrary floating offsets.
The binding now preserves the observed exception types and messages, including
C-long overflow before C-int overflow.

Merely matching the shader's float type did not restore GPU parity: four of
108 comparisons still failed after that first repair. Scale 0.3 exposed device
division differences at truncation boundaries; a subnormal scale with offset 1
also changed the zero numerator. Receipts preserve these failed variants in
`perf-add-20260924-initial-pairs-parity.json` and
`perf-add-20260924-f32-parity.json`.

The retained GPU path builds a packed 512-byte table from scale and offset in
Rust core. Although there are 65,536 input byte pairs, only 511 sums or
differences can enter the affine formula. The GPU indexes that table while
processing every pixel; the host neither reads image pixels to construct the
table nor caches output images. This removes the old per-admission exhaustive
pair scan and device floating division. All 36 exhaustive parameter cases
execute natively on each backend: 108 exact comparisons, with one GPU dispatch
and a 768-byte aligned parameter allocation per GPU receipt. The selected
scales include zero, subnormal, negative, and values that narrow to infinity.
Evidence: `perf-add-20260924-final-pairs-parity.json` and the corresponding
`add-pairs-final-pairs-*-20260924.json` execution receipts.

The third attempt corrected a shared public validator: Pillow clips unequal
input dimensions to their overlap; rejecting them was wrong. A focused probe
failed all 288 comparisons before the correction. Ninety-six permanent cases
now cover twelve affected arithmetic operations in L/LA/RGB/RGBA, both empty
overlaps and varied nonempty rows with different source strides. Native
acceleration for unequal shapes still needs separate proof; fallback parity
does not satisfy the accelerated target.

The final attempt specializes default CPU Add/Subtract as saturating byte
arithmetic. SIMD full blocks now load/store directly, padding only the final
partial vector. The isolated Add loop improved 17.9× at 2.25 MiB, but the
three-byte case regressed by about 2 ns; these component timings are not public
latency claims. All 532 strict CPU/SIMD comparisons across ten operations,
vector tails, row boundaries, and supported native byte modes pass. Evidence:
`add-simd-loop-20260924.jsonl` and
`perf-add-20260924-final-tails-parity.json`.

### Argument-test representation limitation

The migration manifest declares numeric offset literals and its validator
rejects `None` and string literals before invoking either library. The first
post-LUT benchmark therefore stopped before timing. No validator, assertion,
threshold, or workload was weakened. The ten original offset inputs moved
intact to `scripts/test_chops_affine_parity.py`, with an additional custom
`__index__` object. All 66 backend/case comparisons pass against isolated live
Pillow, checking exact output or exception type and message. Numeric rounding
cases remain in the maintained corpus. This is an input-representation limit,
not evidence that invalid offsets are supported.

### Add checkpoint and remaining gaps

The final shared-operation cohort passes 2,379 comparisons, including the
permanent clipping cases: `perf-add-20260924-final-shared-parity.json`.
All seven maintained Add workloads retain their original policy in receipt
`migration-benchmark-13fceee41f6a4fd3abfc35ecb697fd2f`. The standard deferred
construction row has no native execution proof. The six materialized rows
record completed execution of the requested backend without fallback. Median
milliseconds are:

| Add workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized RGB 16 × 16 | 0.019521 | 0.018104 | 0.018188 | 0.477250 |
| RGB 32 × 24 | 0.016271 | 0.076479 | 0.017542 | 0.331125 |
| RGB 1 × 1 | 0.013438 | 0.017542 | 0.017354 | 0.402751 |
| RGB 32 × 32 | 0.017146 | 0.017875 | 0.017146 | 0.559708 |
| RGB 256 × 256 | 0.196875 | 0.039751 | 0.081667 | 0.850771 |
| RGB 1024 × 768 | 3.128625 | 0.900146 | 0.623250 | 4.019708 |

The six materialized workflows also pass 18 separate strict-backend exact
output comparisons in `perf-add-20260924-final-workloads-parity.json`; their
maintained execution-only timing gates are not used as parity evidence.

At 256², CPU improves 5.20× from its initial 0.206563 ms and SIMD improves
1.63× from 0.133208 ms. The large SIMD sample improves 2.59× from 1.613604 ms
and is 5.02× faster than this run's Pillow median. The smaller SIMD cases still
miss 5×. Tiny CPU cases still lose, and the 32 × 24 CPU result regresses from
0.017500 ms to 0.076479 ms, with variation across setup, planning, and terminal
phases. Keep that observation; a later controlled repeat must resolve its cause.

GPU improves 2.26× on the materialized 16² case and 3.15× at 32 × 24. The large
GPU sample changes from 3.961792 to 4.019708 ms and remains 6.45× slower than
SIMD. Its terminal phase has a 3.733230 ms median, including execution and
export; this is not a device-kernel measurement. Remaining investigations are
fixed public-call overhead, input/output copying, CPU row scheduling, and GPU
preparation, transfer, lookup, and completion costs. A default-parameter integer
shader could avoid table lookups, but the later Subtract trial found no reliable
public benefit from that change. Measure device time before another arithmetic
rewrite. No full-operation target is complete.

The fresh-image throughput run passes 40,320 exact output comparisons,
including 38,400 timed completions. Source hashes remain unchanged and runtime
binaries agree across processes. Every GPU request includes both input images,
one dispatch, and output readback. Receipt:
`add-throughput-20260924-final.json`. Completed 1024 × 768 image pairs per second
are:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1378.9 | 2987.4 | 5790.2 | 258.3 |
| L | 2 | 1414.4 | 4576.4 | 6659.3 | 403.8 |
| L | 4 | 1397.4 | 5291.4 | 7012.4 | 582.1 |
| RGB | 1 | 258.9 | 1020.6 | 1359.6 | 263.5 |
| RGB | 2 | 301.6 | 1344.7 | 1733.8 | 401.2 |
| RGB | 4 | 324.1 | 1407.8 | 1603.1 | 538.3 |

At depth one, SIMD request medians are 0.142833 ms for L and 0.588958 ms for
RGB, versus Pillow's 0.668812/3.740542 ms: 4.68×/6.35×. The L latency target
still misses, and GPU loses to SIMD on both latency and throughput at every
tested depth. SIMD RGB throughput also falls between depths two and four; more
workers alone do not solve that limit. These are completed fresh requests,
not reciprocal-latency estimates. Cross-runtime and pre-push checks remain
pending. The next bounded visit is `PIL.ImageChops.subtract`, which shares the
retained fixes but still needs its own measurements and remaining-gap review.

## Subtract visit

The three-workload starting cohort passes 198 focused parity comparisons, but
its materialized 16² and 32 × 24 rows still miss the targets. Four size-matrix
workloads are added at 1², 32², 256², and 1024 × 768 using the existing workload
policy. All six materialized workflows pass 18 separate exact comparisons
before timing, since the maintained workflow gate records successful execution
rather than output parity. No existing workload or assertion changes.

The seven-row baseline is
`migration-benchmark-6867c98db6144b1d87e4411bf4f4042b`. At 1024 × 768,
Pillow/CPU/SIMD/GPU medians are 3.122313/0.968917/0.593250/4.136271 ms. The
changing-input baseline passes 40,320 exact outputs, including 38,400 timed
completions, with consistent source/runtime identity. At queue depth one, its
L rates are 1253.3/3001.1/5443.4/258.0 fresh pairs per second and RGB rates are
260.8/875.0/1152.0/274.3. Receipt:
`subtract-throughput-20260924-baseline.json`.

Three bounded optimization attempts address separate costs:

1. CPU and SIMD Chops now hold the existing immutable secondary pixel storage.
   Previously, `materialize()` returned a deep copy before the kernel borrowed
   its byte slice. Palette indices remain native samples. The SIMD helper also
   serves blend and fused multiply/screen, so the verification cohort includes
   those callers and their composed workloads.
2. The GPU trial used exact unsigned `left - min(left, right)` for default
   subtraction. General scale/offset values retained the exact parameter-only table. The first
   shader variant was rejected for implicit unsigned-to-signed vector conversion;
   the unsigned expression resolves the validation failure without float math.
   Failed receipts remain in `perf-subtract-20260924-shared-fast-gpu-parity.json`
   and `perf-subtract-20260924-shared-fast-gpu-pairs-parity.json`. The corrected
   candidate passed parity but did not show a reliable public win: large GPU
   latency changed from 4.136271 to 4.057604 ms, small-case changes were mixed,
   and fresh RGB throughput at queue depth four fell from 543.0 to 421.3 pairs
   per second. L throughput rose only 6–8% while Pillow's L baseline also varied
   substantially between runs. The shader and parameter changes are rejected;
   the checkpoint retains the prior exact table implementation. Candidate
   receipts: `perf-subtract-20260924-scheduling.json` and
   `subtract-throughput-20260924-scheduling.json`.
3. Cheap CPU subtraction stays serial below 4 MiB of output and groups up to
   32 complete rows per parallel task above it. General arithmetic retains its
   existing scheduling policy. The paired 12-thread component diagnostic compares
   serial rows, individual parallel rows, and grouped rows with identical data.
   At 1024 × 768 L, their medians are 15.45/57.92/45.90 µs; at RGB, they are
   52.15/70.48/55.55 µs. At 2048² RGB, grouping reduces 373.38 to 315.33 µs.
   These component results select the policy; they do not establish public
   latency. Receipt: `subtract-row-scheduling-20260924.jsonl`.

All 108 exhaustive byte-pair comparisons pass after the shader correction.
Six live-Pillow CPU comparisons around the scheduling crossover also pass,
including unequal source strides and partial final row groups. Receipts:
`perf-subtract-20260924-scheduling-pairs-parity.json` and
`perf-subtract-20260924-scheduling-strides-parity.json`.

### Subtract checkpoint and remaining gaps

The retained changes are shared secondary buffers and CPU scheduling for the
default byte operation. The shared cohort passes 2,943 comparisons, including
blend and composed Chops workflows. After restoring the original GPU path,
the final build passes 210 focused Subtract comparisons, 108 exhaustive affine
comparisons, and 18 materialized-workflow comparisons. Artifacts are
`perf-subtract-20260924-scheduling-shared-parity.json`,
`perf-subtract-20260924-final-parity.json`,
`perf-subtract-20260924-final-pairs-parity.json`, and
`perf-subtract-20260924-final-workloads-parity.json`.

Final receipt `migration-benchmark-7abb03f24fa640f5af3e9ed645cb1939` retains the
seven-workload policy. All six materialized rows have completed native receipts
without fallback; the deferred standard row has no execution proof. Median
milliseconds are:

| Subtract workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized RGB 16 × 16 | 0.015563 | 0.016542 | 0.018938 | 0.486230 |
| RGB 32 × 24 | 0.015708 | 0.016354 | 0.017750 | 0.386667 |
| RGB 1 × 1 | 0.014125 | 0.015833 | 0.017750 | 0.420083 |
| RGB 32 × 32 | 0.016417 | 0.016354 | 0.016855 | 0.358875 |
| RGB 256 × 256 | 0.181396 | 0.031271 | 0.033813 | 0.704583 |
| RGB 1024 × 768 | 2.859647 | 0.479021 | 0.419480 | 3.740146 |

Large CPU latency improves 2.02× and SIMD improves 1.41× against the expanded
baseline. SIMD is 5.36×/6.82× faster than Pillow at 256²/1024 × 768 in this
run, but the small rows still miss. CPU remains slower than Pillow on three
materialized small rows. The unchanged GPU implementation remains much slower
than SIMD; its timing differences between runs are not an implementation gain.
The next visit must profile the remaining public-call, copying/export, and GPU
transfer/completion costs rather than repeat an unproven arithmetic change.

Final changing-input receipt `subtract-throughput-20260924-final.json` passes
40,320 exact output comparisons, including 38,400 timed completions, with
unchanged source hashes and consistent runtime binaries. Every GPU request
records its primary and auxiliary images, one dispatch, and readback. Completed
1024 × 768 pairs per second are:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1411.9 | 8268.2 | 8095.7 | 282.5 |
| L | 2 | 1467.8 | 9893.7 | 9747.1 | 422.0 |
| L | 4 | 1499.7 | 10673.4 | 10471.8 | 625.6 |
| RGB | 1 | 277.1 | 1984.9 | 1979.6 | 293.0 |
| RGB | 2 | 313.5 | 2656.8 | 2565.9 | 415.2 |
| RGB | 4 | 339.4 | 2546.2 | 2420.7 | 601.0 |

At depth one, SIMD request medians are 0.105146 ms for L and 0.412104 ms for
RGB, versus Pillow's 0.646771/3.506312 ms: 6.15×/8.51×. CPU request medians
improve from 0.287250/0.894875 to 0.103688/0.414166 ms. These larger fresh-image
results meet the CPU and SIMD latency ratios, but the operation remains pending
because small cases still lose and GPU misses both targets at every queue depth.
RGB CPU/SIMD throughput also falls between depths two and four, so increasing
concurrency alone does not resolve the remaining limit. Cross-runtime and
pre-push checks remain pending. After these three attempts, the next ranked
visit is `PIL.ImageOps.grayscale`; the shared font-loading blockers remain
recorded separately above. No coverage collection was run.

## Grayscale visit

The seven-workload baseline is
`migration-benchmark-0678c64154da4ed3b3d6910d8de66401`. At 1024 × 768,
Pillow/CPU/SIMD/GPU medians are 0.382959/0.929271/1.843626/2.985209 ms.
The ordinary standard row only queues the operation and has no native execution
proof; six materialized workflows provide the meaningful latency boundary.

Four bounded attempts address distinct mechanisms:

1. The original focused cohort passes 42 comparisons, but an 18-mode audit
   finds 102 failures among 972 comparisons. `RGBa` needs integer
   unpremultiplication before grayscale; zero alpha preserves stored channels,
   and samples above alpha clip. `La` must construct successfully and then
   reject conversion to L, including empty images. Core constructors now retain
   its logical tag, and public Grayscale/conversion report the exact error.
   CPU conversion to L shares the source-aware grayscale executor. SIMD's
   conversion admission rejects its unsupported RGBa-to-L arithmetic instead
   of interpreting premultiplied bytes as ordinary RGBA. These premultiplied
   cases use the exact CPU path and do not prove native SIMD/GPU acceleration.
   Thirty-six permanent input-only cases cover both entry points. The first
   repair leaves eight SIMD conversion failures; the admission fix clears them.
   Receipts: `perf-grayscale-20260925-initial-modes-parity.json` and
   `perf-grayscale-20260925-parity-fix-parity.json`.
2. CPU grayscale borrows native L/LA/RGB/RGBA storage. Previously, `to_rgb8()`
   allocated a complete intermediate even for RGB inputs. Layout-specific loops
   remove that copy/expansion while keeping the rounded L and truncated bilevel
   formulas distinct. Receipt `migration-benchmark-7e49cdb9768547ea84deb0413c334a6d`
   reduces large CPU latency to 0.201313 ms, 4.62× faster than its initial value;
   Pillow measures 0.338479 ms in that run. Small public calls still miss.
3. SIMD loads complete sixteen-pixel blocks and deinterleaves with byte shuffles;
   only the tail is padded. L is a native copy and LA drops alpha by shuffle.
   RGB arithmetic retains all coefficient bits in sixteen u16 lanes. Write
   `19595 = 77*256-117`, `38470 = 150*256+70`, and `7471 = 29*256+47`.
   With `base = 77R+150G+29B` and `residual = 70G+47B-117R+32768`, the exact
   result is `(base + (residual >> 8)) >> 8`. Residual lies in 2933..62603;
   unsigned wrapping intermediates recover that positive result, and the final
   pre-shift sum is at most 65524. All 16,777,216 RGB triples match live Pillow
   on CPU/SIMD/GPU before and after this rewrite. The prototype still takes
   1.041396 ms on large SIMD input: assembly reveals seven `memset_pattern16`
   calls setting up vector constants for each sixteen-pixel block. Receipt:
   `migration-benchmark-e1ae0db70c864ec88a130693f87036d8`; assembly:
   `grayscale-simd-blocks-20260925.asm`.
4. The final code-generation change forces compile-time vector constants and
   inlines the block kernel so the loop can retain them. It changes no arithmetic
   or input contract.

Before the final constant-setup change, the shared conversion/Color/Contrast/
Grayscale cohort passes 2,709 comparisons, the 18-mode audit passes 972, and
all native-layout widths 0..33 pass 510 comparisons. Their receipts are
`perf-grayscale-20260925-final-shared-parity.json`,
`perf-grayscale-20260925-final-modes-parity.json`, and
`perf-grayscale-20260925-final-tails-parity.json`. The exhaustive RGB diagnostic
retains actual concatenated output bytes, backend receipts, and binary identities
in `grayscale-rgb-domain-20260925-initial/` and
`grayscale-rgb-domain-20260925-final/`; the latter records attempt three.

The new exhaustive diagnostic initially assumed a `status` key in raw core
telemetry. That key belongs to adapter receipts, not raw telemetry. The corrected
harness requires a successful terminal byte export, the requested and actual
backend, one operation without fallback, and complete GPU upload/readback plus
one dispatch. It compares actual bytes with isolated live Pillow; it never uses
the proposed formula to construct backend expected outputs. This was a harness
schema error, not a production parity failure. No assertion, workload, or
performance threshold is weakened.

### Grayscale checkpoint and remaining gaps

The final SIMD assembly has no function calls in the complete-pixel hot loop;
shuffle masks and arithmetic constants are initialized before it. Receipt:
`grayscale-simd-constants-20260925.asm`. After that last change, all RGB triples
again pass exact CPU/SIMD/GPU comparison, 510 tail/layout comparisons pass, and
96 maintained Grayscale/workflow comparisons pass. Final receipts:
`grayscale-rgb-domain-20260925-constants/report.json`,
`perf-grayscale-20260925-constants-tails-parity.json`, and
`perf-grayscale-20260925-constants-final-parity.json`.

The checkpoint latency receipt is
`migration-benchmark-c5c873b4ac384bd79199df4bfcc26f29`. All six materialized
rows have completed native CPU/SIMD/GPU receipts without fallback. The separate
workflow parity checks above establish exact output. No workload or repeat
policy changes. An earlier timing run overlapped documentation generation;
`perf-grayscale-20260925-constants-final.json` is retained, while this checkpoint
run starts after that background work completes. Median milliseconds are:

| Grayscale workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized RGB 16 × 16 | 0.012375 | 0.011979 | 0.012979 | 0.561979 |
| RGB 32 × 24 | 0.010167 | 0.010750 | 0.011125 | 0.425396 |
| RGB 1 × 1 | 0.009771 | 0.010084 | 0.010604 | 0.534646 |
| RGB 32 × 32 | 0.010146 | 0.010479 | 0.011104 | 0.418063 |
| RGB 256 × 256 | 0.027021 | 0.023625 | 0.031875 | 0.536834 |
| RGB 1024 × 768 | 0.292855 | 0.205959 | 0.261521 | 2.226750 |

Large CPU and SIMD latency improve 4.51× and 7.05× against their starting
values. SIMD is only 1.12× faster than Pillow there and misses the 5× target
at every materialized size. CPU still loses on three small rows. GPU still
loses to SIMD on every row; its implementation is unchanged, so run-to-run GPU
timing variation is not an optimization gain.

The next Grayscale visit needs to address public wrapper/materialization/export
costs for small images and compare native interleaved loads with the portable
shuffle sequence. The CPU loop currently beats explicit SIMD on the largest
row. GPU RGB 1024 × 768 still uploads and reads back 3,145,728 bytes each for a
786,432-byte L output. A native input/output layout would remove RGB expansion
and reduce readback, but requires an exact dedicated dispatch/result path.
GPU synchronization and queue behavior also remain unresolved. Premultiplied
modes still lack native acceleration; cross-runtime verification remains pending.

The first changing-input run completes its L cohort, then the new Grayscale
selector rejects Pillow's RGB reference inventory because one remaining check
assumes output mode equals input mode. The shared expected-result-mode helper
now requires L for grayscale at both inventory and returned-output checks;
all previous mode-preserving selectors keep their existing requirement.
`grayscale-throughput-20260925-checkpoint.json` retains this failed harness run.
The fixed harness continues checking every output byte and the complete input
upload independently of the smaller output readback.

Final changing-input receipt `grayscale-throughput-20260925-final.json` passes
40,320 exact outputs, including 38,400 timed completions, with consistent source
and binary identities. Completed 1024 × 768 images per second are:

| Input mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 8120.5 | 13910.2 | 12344.5 | 500.1 |
| L | 2 | 8014.5 | 14602.3 | 14267.5 | 745.2 |
| L | 4 | 7403.3 | 15118.7 | 14256.3 | 989.4 |
| RGB | 1 | 1715.6 | 3966.0 | 3238.0 | 523.5 |
| RGB | 2 | 2778.9 | 6535.9 | 5429.9 | 829.1 |
| RGB | 4 | 3540.4 | 8173.9 | 7426.8 | 1220.6 |

At depth one, SIMD request medians are 0.072042 ms for L and 0.284459 ms for
RGB, versus Pillow's 0.105042/0.558459 ms: 1.46×/1.96×. CPU medians are
0.063167/0.226812 ms. GPU medians are 1.962875/1.844958 ms, and GPU throughput
loses to SIMD at every queue depth. These completed-request measurements do not
meet the remaining goals.

Four attempts are checkpointed. The reusable optimization skill now includes
semantic-versus-physical layout decisions, exact coefficient decomposition with
bounded carries, and diagnosing runtime vector-constant setup. No coverage was
run; input regeneration updates only declared plans and documentation counts.
Pre-push checks and cross-runtime evidence remain pending. No operation is
newly declared complete. The next bounded visit is `PIL.Image.Image.resize`,
including its mapped pipeline workloads; Grayscale remains pending with the
specific blockers above.

## Transpose verified behavior

On 2026-09-24, the release extension passed the focused 368-case transpose
cohort on CPU, SIMD, and GPU: 1,104 exact Pillow comparisons. Strict GPU receipts
recorded 366 applicable hardware executions, one exact host semantic control,
and one case outside pipeline telemetry. The initial sandbox run had no
enumerated adapters and returned adapter-unavailable errors; rerunning with
host GPU access passed without changing source, cases, or assertions.

The wider input corpus currently contains 11,236 parity cases, 24 coverage
plans, and 773 benchmark workloads. Those counts describe indexed inputs, not
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
