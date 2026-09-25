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

## Resize visit

This visit uses four bounded attempts: repair source-box/reduction semantics,
repair typed and indexed sample handling, remove a CPU output copy, and simplify
SIMD vertical sampling. The unchanged baseline contains 25 Resize workloads,
including alpha, typed, repeated-geometry and loaded ten-operation pipelines.
Its receipt is `migration-benchmark-e8cdb155b05547228c965d908223c062`, retained
as `perf-resize-20260925-initial.json`. The initial 317 maintained Resize cases
and 24 mapped workflows pass 1,023 CPU/SIMD/GPU comparisons, but that cohort
misses several public argument behaviors.

### Parity repairs

A varied-pixel audit of floating/integer boxes, invalid bounds and reduction gaps
fails 342 of 432 comparisons. The Python wrapper discarded `reducing_gap`, the
binding accepted only integer boxes, and core cropped the source before
resampling. An interior crop discards surrounding filter taps. The repair keeps
floating bounds on a ResizeBoxed operation, reuses the existing boxed kernels,
and validates bounds after Pillow's float32 narrowing. Integer pre-reduction
retains the filter halo and transforms the original coordinates into the reduced
image's coordinate system. LA/RGBA filtered resizing intentionally ignores a
valid gap, matching Pillow's premultiplied recursive call. Direct encoded sources
are decoded at resize() so malformed-image failures occur at the public call.

The broader 16-mode/tall-source audit initially passes 1,179 of 1,224 comparisons.
All 45 failures are PA: Pillow filters raw index/alpha bands, while pillow-rs
forced nearest sampling. PA now retains its filter and palette. Reduction must
also average those bands independently; the CPU alpha predicate and GPU's
per-operation packed-mode word now preserve that rule. The targeted PA rerun,
including retained palettes, passes all 72 comparisons.

I;16 variants use the existing native two-byte boxed resampler and preserve its
byte-order and clipping rules. Pillow rejects those modes only when reducing_gap
actually selects integer reduction. La and RGBa are already premultiplied and
must not be multiplied again. Very tall images follow Pillow's vertical-first
pass order, retaining premultiplied samples across both passes. Intermediate
rounding makes pass order and repeated alpha conversions observable.

The generator adds 558 input-only regressions. No expected outputs, assertions,
thresholds or existing workloads are changed. The floating-box/tall descriptor
currently falls back to CPU; those comparisons establish output parity, not
native SIMD/GPU support. The first argument and maintained scratch runs used the
same receipt filename and one overwrote the other. Subsequent runs use distinct
names; the initial failure receipt is retained and the cases are rerun through
the permanent fixture cohort.

### Performance changes

CPU resampling previously constructed an owned output Vec, copied it into a
second Vec inside the image constructor, then discarded the original. Moving
the completed Vec into the existing owned constructor removes that allocation
and full output copy, including boxed and typed-F results. The intermediate
quantization and sampling loops are unchanged. The intermediate measurement is
`migration-benchmark-a49c0a482d1a4efda71189cef8327380`, retained as
`perf-resize-20260925-owned-output.json`.

The fourth attempt removes repeated scalar address and mask construction from
SIMD vertical taps. Adjacent pixels occupy one contiguous span of an intermediate
row; validate that span once, then gather its channel samples. Short vectors
retain zero padding and unchanged scalar tails. Fixed-point accumulation,
rounding, clipping and alpha restoration are unchanged.

### Resize checkpoint and remaining gaps

All final selected comparisons pass: 2,697 Resize/workflow, 2,898 shared-kernel,
and 300 strict native-backend tail cases. Receipts are
`perf-resize-20260925-checkpoint-parity.json`,
`perf-resize-20260925-checkpoint-shared-parity.json`, and
`perf-resize-20260925-checkpoint-tails-parity.json`. The shared cohort covers
reduce, thumbnail, fit, contain, cover, pad and scale. The strict tail cohort
uses varied L/LA/RGB/RGBA inputs, five convolution filters, and output widths
7, 8, 9, 16 and 17. Ordinary fallback cases remain separate from native proof.

After all diagnostics and documentation generation finish, the unchanged 25-row
benchmark completes as `migration-benchmark-010b26a78b4c441d8a88c3e8bf8c22e0`,
retained in `perf-resize-20260925-checkpoint.json`. All 24 materialized rows have
completed native CPU/SIMD/GPU receipts without fallback. The remaining standard
row queues work without terminal materialization and has no native execution
proof. Representative median milliseconds, including poor composed cases, are:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB 16 × 16 → 16 × 16 | 0.011833 | 0.013104 | 0.013167 | 0.321833 |
| RGB 32 × 24 → 16 × 16 | 0.013688 | 0.012000 | 0.015938 | 0.296167 |
| RGB 1 × 1 → 16 × 16 | 0.010042 | 0.011229 | 0.011437 | 0.306562 |
| RGB 32 × 32 → 16 × 16 | 0.014417 | 0.011855 | 0.017792 | 0.302792 |
| RGB 256 × 256 → 16 × 16 | 0.230791 | 0.064521 | 0.209187 | 0.560042 |
| RGB 1024 × 768 → 16 × 16 | 2.591354 | 0.720958 | 1.035375 | 2.423188 |
| Resize → Rotate → Crop | 0.024334 | 0.164000 | 0.213230 | 0.738458 |
| RGBA Lanczos 256 × 256 | 0.668166 | 0.387271 | 0.478313 | 0.298084 |
| LA Bicubic 256 × 256 | 0.340249 | 0.177792 | 0.231792 | 0.381667 |
| RGBA Lanczos 1024 × 768 | 8.033646 | 3.624542 | 4.482855 | 1.842000 |
| LA Bicubic 1024 × 768 | 4.583562 | 1.520666 | 1.764187 | 2.259875 |
| F repeated geometry pipeline | 0.308729 | 0.083125 | 1.262042 | 12.662896 |
| Loaded RGB JPEG ten-operation pipeline | 3.495854 | 3.928834 | 3.770625 | 3.144667 |
| Loaded RGBA PNG ten-operation pipeline | 3.523499 | 3.883458 | 3.901354 | 5.718375 |

The SIMD vertical change lowers the measured LA 1024 × 768 row from 1.940479 to
1.764187 ms relative to the immediately preceding build, and RGBA Lanczos
256 × 256 from 0.551083 to 0.478313 ms. Some tiny rows regress in the same run;
these short samples do not establish statistical significance. CPU output
ownership removes a proven copy, but timing changes also include environment
variation: Pillow and unchanged GPU paths move between runs. GPU timing movement
on these workloads is not a claimed kernel improvement.

All 25 diagnostic rows still miss the SIMD 5× goal; nine miss CPU ≤ Pillow and
20 miss GPU ≤ SIMD. Excluding the unmaterialized standard row leaves 24/24 SIMD,
8/24 CPU and 19/24 GPU misses. Several basic benchmark inputs are uniform zero images. The varied-input audits
prove their own exact outputs but do not establish varied-input speed. Real
changing-input sustained throughput has not been measured for Resize, so there
is no throughput claim or completed-operation claim. Small images, composed
pipelines and typed resampling remain visible.

The next Resize visit should address:

- Native boxed/tall execution: reuse the existing accelerated boxed kernels with
  complete bounds, typed-mode and pass-order admission. Current fallback timings
  cannot satisfy a native acceleration requirement.
- SIMD horizontal setup and gathers: the horizontal plan is rebuilt per call;
  rows always enter the parallel scheduler, and channel gathers plus widened
  accumulators remain expensive. Measure setup, scheduling and actual tap work
  separately before choosing plan caching, row grouping or a different lane layout.
- Typed and composed costs: repeated F geometry takes 1.262042 ms on SIMD versus
  0.083125 ms on CPU. Preserve f64 accumulation/f32 storage and reference FMA
  behavior while investigating the typed kernel. The Resize/Rotate/Crop chain
  remains much slower than Pillow and motivates the next operation visit.
- GPU preparation, transfers and completion: packed transport, coefficient
  preparation, multiple passes and synchronous readback still dominate small
  requests; the F repeated-geometry pipeline remains especially slow. A throughput
  improvement needs fresh completed requests, exact outputs and transfer receipts.

Four attempts are checkpointed; the campaign moves to Rotate. The reusable skill
now includes dependency halos, coordinate changes after coarse reduction,
quantized pass ordering and logical-mode propagation through packed transport.
Generated contract counts are refreshed without collecting coverage. Pre-push
Rust/documentation checks, cross-runtime verification and the global evidence
matrix refresh remain pending; these focused receipts do not replace them.

## Rotate four-attempt checkpoint — 2026-09-25

This visit spent its four attempts on parity before performance. The original
109 maintained Rotate cases plus nine mapped materialized workflows passed all
354 backend comparisons, but a new seeded varied-pixel audit found 240 failures
among 576 comparisons. Uniform or ordinary-mode examples had hidden these
failures. The 192 input-only regressions now live in the deterministic fixture
generator: 16 modes, three filters, and arbitrary, expanded, non-square right-angle,
and custom center/translation/fill cases. They compare both image metadata and
materialized bytes against the live isolated Pillow oracle.

The retained changes are:

1. Reuse Transform's native fill normalization, preserving LA/La/PA alpha,
   RGBa/RGBX/CMYK's fourth byte and typed scalar bytes. Already-premultiplied La
   bypasses another alpha round trip.
2. Correct CPU filtered geometry: an unexpanded non-square 90/270-degree
   rotation uses the affine sampler, PA uses the ordinary half-pixel offset,
   and I;16 follows the reference's sample ABI. Nearest copies complete u16
   samples using floating-point center coordinates; filtered I;16 samples the
   first width bytes of each two-byte row and writes only the first byte of
   each output sample, retaining the other fill byte. This surprising behavior
   is present in Pillow's Geometry.c and is not treated as a test defect.
3. Correct SIMD center mapping, bounds and ordered bilinear arithmetic; reuse
   GPU's exact binary64 projective sampler by lowering filtered rotation to an
   affine map with denominator one. Geometry preparation remains host work;
   pixel interpolation stays on the GPU. Native backend proof is separate from
   fallback parity.
4. Preserve each typed interpolation intermediate: F subtraction rounds in f32
   before widening, while I uses wrapping i32 subtraction and separate
   multiply/add. Preserve fused byte/F arithmetic on targets where wide's
   mul_add would otherwise split it. Make the lowered GPU default fill explicitly
   zero in every band, including RGBX padding and empty-source fallback.

The source contract was checked against Pillow 12.2.0's Image.rotate wrapper and
[Geometry.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Geometry.c).

The final Rotate cohort passes **930/930** exact comparisons. Strict-backend
bilinear runs pass **72/72** comparisons across SIMD/GPU, and strict-backend GPU
bicubic passes **36/36**, covering nine logical modes. Backend locking alone
does not exclude an exact host semantic fallback. A subsequent Transform
checkpoint rerun records actual execution for all 108 comparisons: 36 SIMD
and 72 GPU executions, completed on their requested backend without fallback.
That receipt is `perf-rotate-20260925-transform-shared-parity.json`. The
shared affine Transform cohort passes **2,241/2,241** comparisons; that receipt
precedes the final Rotate-only typed helper and fill-lowering changes. Receipts:

- `build/migration-parity/perf-rotate-20260925-checkpoint-parity.json`
- `build/migration-parity/perf-rotate-20260925-checkpoint-strict-parity.json`
- `build/migration-parity/perf-rotate-20260925-checkpoint-cubic-strict-parity.json`
- `build/migration-parity/perf-transform-20260925-rotate-shared-parity.json`

The unchanged ten-workload checkpoint is
`migration-benchmark-e3a27269fe3940ccbebea447d91acfa6`, retained as
`build/migration-parity/perf-rotate-20260925-checkpoint.json`. All 27 backend
receipts for the nine materialized workloads report completed execution on the
requested backend without fallback. The standard row only builds a lazy result.
Selected medians in milliseconds on the Apple M3 Pro:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB 16 × 16 materialized | 0.015917 | 0.013938 | 0.015417 | 0.329521 |
| RGB 32 × 24 | 0.013626 | 0.013855 | 0.016146 | 0.298208 |
| RGB 32 × 32 | 0.013646 | 0.014605 | 0.018917 | 0.293751 |
| RGB 256 × 256 | 0.083604 | 0.251209 | 0.345729 | 0.450146 |
| RGB 1024 × 768 | 1.253251 | 1.042833 | 4.048084 | 1.769626 |
| RGBA 1024 × 768 | 2.237209 | 1.549688 | 7.340833 | 2.627646 |
| Loaded RGB JPEG ten-operation pipeline | 3.412229 | 3.868229 | 3.724000 | 3.259062 |
| Loaded RGBA PNG ten-operation pipeline | 3.424500 | 3.833980 | 3.759709 | 5.721646 |

Five of nine materialized rows miss CPU ≤ Pillow, all nine miss the SIMD 5×
goal, and six miss GPU ≤ SIMD. Including the lazy row gives 5/10, 10/10 and
6/10 misses respectively. These short diagnostic timings do not establish
significant speed changes. The basic workloads mostly use nearest rotation and
uniform pixels, so they do not measure the repaired varied-pixel filtered
kernels. No real changing-input sustained throughput run was made for Rotate;
the benchmark's reciprocal-latency field does not satisfy that requirement.
Rotate remains incomplete, and the campaign still has no operation with every
parity/performance/throughput target demonstrated.

Concrete blockers and next-visit decisions:

- **SIMD nearest work:** the current loop divides/modulos a flattened output
  index for every pixel, multiplies fixed-point coordinates per lane, performs
  scalar gathers, then constructs a byte vector that is immediately unpacked.
  Investigate row traversal with coordinate increments and useful packed
  coordinate arithmetic; inspect assembly before crediting vector execution.
  Preserve the distinct floating-coordinate contract for I;16 nearest. Its
  current accelerated admission still needs boundary-focused proof.
- **CPU overhead:** ordinary rotation clones the source before a read-only
  sampler when no premultiplication is needed. Borrow that buffer, then measure
  row scheduling versus useful pixel work, especially the 256 × 256 crossover.
  Do not attribute small timing movements from the parity repairs to this
  unimplemented optimization.
- **Filtered GPU cost:** the reused exact kernel has 13 geometry words per
  output pixel (52 bytes before image traffic), host table construction and
  software binary64 arithmetic. Measure the complete filtered path; replace
  per-pixel geometry uploads with a compact exact coordinate plan only when
  source selection and evaluation order remain identical. Native support for
  La, I and I;16 filtered rotation remains incomplete.
- **Small requests and pipelines:** approximately 0.3 ms GPU completion cost
  dominates tiny requests. Measure allocation, transfers, mapping and host
  output creation, then fresh concurrent completion throughput. Keep all of
  these costs inside the comparison boundary.
- **Proof still missing:** varied-input performance, real throughput, broader
  typed/extreme/tail cases, other hardware and bindings, and the complete
  operation matrix refresh. The focused passing cohorts are not universal
  parity or acceleration declarations.

The skill records the reusable decisions about intermediate types, geometric
fast-path predicates, storage ABI, exact kernel reuse and parameter traffic.
The fixture index now contains 12,022 cases, 24 static plans, 773 benchmark
workloads and 54 suites. No coverage was collected. Pre-push Rust, docs and
cross-runtime checks remain pending. Four attempts are checkpointed; the next
operation is Transform, starting with its unchanged benchmark and the existing
shared-parity receipt.

## Transform four-attempt checkpoint

On 2026-09-25, Transform reached the four-attempt limit and remains incomplete.
The initial maintained cohort passed 2,241 comparisons, but a seeded audit of
16 modes, three filters and five methods found **248 failures in 720
comparisons**. The retained fixes preserve Pillow's typed arithmetic, alpha,
fill and source-coordinate selection:

1. CPU I interpolation now uses wrapping integer source differences and ordered
   binary64 arithmetic; F projective/mesh sampling borrows raw storage and
   writes final bytes directly. La bypasses another premultiplication. I;16
   handles complete-sample nearest and the reference's unusual filtered byte
   ABI across all methods, including overlapping mesh records.
2. SIMD I;16 nearest uses floating pixel-center coordinates; bilinear reuses
   exact byte interpolation with logical sample byte order. Unsupported
   bicubic work reports CPU fallback instead of silently doing nearest.
3. GPU filtered affine maps use the existing exact projective sampler with
   denominator one. The shader samples I;16's logical byte plane directly
   from uploaded storage. Filtered I stays an explicit CPU fallback.
4. Match actual GPU transport packing when locating a logical byte or encoding
   fill: I;16B/I;16N uploads differ from other u16 modes. Larger varied nearest
   affine inputs also exposed a CPU/SIMD mismatch hidden by tiny cases. Reuse
   Pillow's signed 16.16 selector, advance CPU coordinates by integer additions,
   and compute eight SIMD coordinates together with packed integer increments.
   Borrow CPU sources when premultiplication is unnecessary.

The reference contract is Pillow 12.2.0
[Geometry.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Geometry.c).
There are 242 new permanent input-only cases: the 240 mode/method/filter
combinations and two 256 × 256 nearest-affine drift regressions. Existing
assertions and benchmark policies are unchanged. Final receipts record:

- **2,967/2,967** maintained Transform/workflow comparisons:
  `perf-transform-20260925-checkpoint-parity.json`.
- **720/720** varied-mode comparisons with actual execution receipts:
  `perf-transform-20260925-checkpoint-modes-parity.json`. Of 240 cases per
  backend, CPU executes 240 natively, SIMD 91 with 149 CPU fallbacks, and GPU
  224 with 16 CPU fallbacks. Passing fallback output does not meet acceleration
  goals. GPU gaps are filtered I plus nearest affine/extent RGBa, La and RGBX.
- **930/930** shared Rotate regressions:
  `perf-rotate-20260925-transform-shared-parity.json`.
- **40,320/40,320** fresh-request outputs, including 38,400 measured
  completions: `perf-transform-20260925-throughput.json`. Every target request
  executes natively without fallback; GPU receipts record one dispatch and
  complete upload/readback. Source and runtime identities stay consistent.

All receipts are under `build/migration-parity/`. The unchanged ten-workload
benchmark is `migration-benchmark-900a3ce0cbfb4c628c8824e7c32caa57`, saved as
`perf-transform-20260925-checkpoint.json`. The initial receipt is
`migration-benchmark-1b5aa73fef644c2a98d76ce678b3d52e`. Final median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB 16 × 16 materialized | 0.014292 | 0.016229 | 0.017167 | 0.306896 |
| RGB 32 × 24 | 0.011855 | 0.014854 | 0.015542 | 0.247083 |
| `pipeline-chain.matrix-025` | 0.031334 | 0.196250 | 0.185250 | 0.936334 |
| `pipeline-chain.matrix-075` | 0.016146 | 0.021855 | 0.021250 | 0.593958 |
| `pipeline-chain.matrix-081` | 0.024355 | 0.018667 | 0.019563 | 0.524084 |
| `pipeline-chain.matrix-082` | 0.016292 | 0.020709 | 0.021355 | 0.556688 |
| `pipeline-chain.matrix-083` | 0.015876 | 0.018917 | 0.019667 | 0.317958 |
| `pipeline-chain.matrix-084` | 0.017167 | 0.024104 | 0.025604 | 0.574604 |
| `pipeline-chain.matrix-085` | 0.015896 | 0.020917 | 0.022459 | 0.593938 |
| Standard lazy construction | 0.008000 | 0.009458 | 0.009730 | 0.009792 |

All 27 materialized target receipts identify native execution without fallback.
CPU misses its target on 8/9 materialized rows; SIMD and GPU miss on all nine.
The lazy row provides no kernel proof. Short timings do not establish stable
speed changes, and these basic workloads do not measure the repaired filtered
or typed paths. The fresh-input predecessor had incorrect CPU/SIMD pixels, so
its timings would not be a valid optimization baseline.

For fresh 256 × 256 nearest affine requests, queue-depth-one median request
latencies are L: Pillow 0.054459, CPU 0.132917, SIMD 0.044708, GPU 0.252583 ms;
RGB: Pillow 0.118041, CPU 0.208667, SIMD 0.066416, GPU 0.255521 ms. SIMD reaches
1.22×/1.78× Pillow, short of 5×. Each request includes source construction,
materialization and receipt capture. Aggregate completed images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 17,574 | 7,192 | 20,413 | 3,838 |
| L | 2 | 28,150 | 6,971 | 18,216 | 3,712 |
| L | 4 | 34,400 | 6,567 | 15,804 | 3,583 |
| RGB | 1 | 8,206 | 4,637 | 14,188 | 3,762 |
| RGB | 2 | 11,935 | 4,533 | 13,342 | 3,747 |
| RGB | 4 | 13,302 | 4,438 | 12,387 | 3,678 |

GPU misses the throughput goal at every measured queue depth. Host request
concurrency does not prove simultaneous GPU kernels. Concrete next-visit work:

- **CPU/SIMD nearest:** specialize CPU channel copies and expose contiguous
  runs; SIMD still gathers selected pixels scalarly. Measure remaining bounds,
  copies and memory traffic before adding wider coordinate vectors. Check host
  serialization because additional workers reduce target throughput.
- **Coordinate parity:** pure scaling and affine maps outside the fixed-point
  range use repeated floating additions in Pillow. The retained fixed-point
  fix does not prove those recurrence boundaries, extreme coefficients or
  every tail. Preserve the reference's algorithm selector before vectorizing.
- **Native support:** remove the 149 SIMD and 16 GPU mode-audit fallbacks with
  exact kernels. This checkpoint proves parity on the selected cohort, not
  universal support or parity for all public inputs.
- **Filtered GPU cost:** the shared exact projective path prepares 52 geometry
  bytes per output pixel and emulates binary64 arithmetic. Measure host table
  preparation, upload and interpolation separately, then reduce parameter
  traffic without changing evaluation order. Small-request completion and
  output materialization remain an approximately 0.25 ms floor here.
- **Outstanding evidence:** varied filtered/typed performance, other hardware
  and bindings, and ingestion of focused receipts into the complete operation
  matrix. Pre-push checks remain pending. No operation has all goals proven.

The skill now records how to preserve arithmetic-path selection and distinguish
logical byte order from actual transfer packing. The fixture index contains
12,264 cases, 24 static plans, 773 benchmark workloads and 54 suites; no coverage
was collected. The next visit is Multiply, the first unvisited operator in the
highest remaining ranked pipeline (`long-auxiliary.multiply-screen-260`).

## Multiply four-attempt checkpoint

On 2026-09-25, Multiply completed four attempts and was checkpointed with
remaining gaps. The unchanged baseline measured 26 declared workloads
(`migration-benchmark-f74c87efb9e1401b951fd0e9e369b326`) and passed 261 maintained
parity comparisons. A separate 115-case audit found 18 failures: the shared
binary-mode validator rejected La, although Pillow applies Chops arithmetic
to its stored bytes. All 65,536 byte pairs already produced exact products.

The retained changes are:

1. Admit La to the existing two-byte Chops mode family. The six Multiply La
   cases and 33 related Chops cases now pass. Algorithms stay in Rust core;
   accepting the logical mode does not imply native SIMD/GPU execution.
2. Load complete SIMD blocks directly instead of initializing two scratch
   arrays and making variable-length copies for every sixteen samples. Share
   the exact block arithmetic with the owned in-place path; pad only the tail.
3. Force the divide-by-255 helper's constant vector to compile-time storage.
   ARM64 disassembly exposed `memset_pattern16` inside every vector iteration;
   the replacement loop contains packed multiply/add, shifts and narrowing
   without that call. For equal contiguous SIMD layouts, stream across row
   boundaries and use 64 KiB tiles above 4 MiB. CPU Multiply uses the existing
   cheap-byte scheduling policy, retaining separate strides for clipped inputs.
   Partial tiles use their actual slice length rather than a full-row endpoint.
4. Transfer native Multiply bytes on GPU, four independent samples per word,
   instead of expanding every pixel to RGBA. Reuse the existing shader,
   checked buffer pool, mapped Metal input, auxiliary staging and readback
   paths. Preserve all channels and remove only transport alignment padding.
   This path handles a single equal-layout Multiply; composed pipelines retain
   their existing execution contracts. No output cache or CPU pixel computation
   substitutes for the GPU kernel.

There are 148 permanent input-only regressions: 115 Multiply cases covering
19 modes, empty/clipped layouts, vector tails and all byte pairs, plus 33 La
cases for the shared validator. Existing tests and benchmark policies remain
unchanged. Exact Pillow evidence under `build/migration-parity/`:

- **606/606** final Multiply comparisons, including mapped composed workflows:
  `perf-multiply-20260925-checkpoint-parity.json`.
- **12/12** large threshold/tail comparisons across CPU, SIMD and GPU:
  `perf-multiply-20260925-checkpoint-tiles-parity.json`. These exercise buffers
  immediately below/at/above 4 MiB and partial final tiles.
- **579/579** shared Screen comparisons after the CPU/SIMD changes:
  `perf-screen-20260925-multiply-attempt3-parity.json`. Subsequent GPU changes
  only affect Multiply; the final Multiply workflows also check compositions.
- **99/99** related La Chops comparisons after the validator repair:
  `perf-chops-la-20260925-multiply-attempt1-parity.json`.
- **40,320/40,320** fresh-request output checks in both the correct pre-optimization
  baseline and final throughput runs. Receipts are
  `perf-multiply-20260925-attempt1-throughput.json` and
  `perf-multiply-20260925-checkpoint-throughput.json`.

In the 115-case mode audit, 79 cases reach valid arithmetic and 36 check rejected
scalar/16-bit modes. CPU executes all 79 valid cases. SIMD executes 61 natively
and falls back on 18; GPU executes 66 natively and falls back on 13 unequal-size
cases. Empty results include native zero-work paths. These passing fallbacks
do not prove acceleration. No universal parity or support claim follows from
this finite cohort.

The final unchanged benchmark receipt is
`migration-benchmark-015177f1e319480f8e762a7e9d44ed9f`, saved as
`perf-multiply-20260925-checkpoint.json`. Selected median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB 1 × 1 | 0.012167 | 0.014292 | 0.015917 | 0.285729 |
| RGB 16 × 16 | 0.014188 | 0.016042 | 0.017125 | 0.306604 |
| RGB 32 × 32 | 0.014813 | 0.018396 | 0.015750 | 0.290854 |
| RGB 256 × 256 | 0.158979 | 0.032292 | 0.033521 | 0.439521 |
| RGB 1024 × 768 | 2.153000 | 0.433417 | 0.434667 | 1.652208 |
| Multiply→Screen RGB 1024² quick pipeline | 6.493709 | 0.899417 | 1.120479 | 3.684229 |
| 260-operation Multiply/Screen chain | 0.864645 | 1.088083 | 1.132437 | 6.797167 |
| Cold Multiply→Screen RGB 1024² | 11.917500 | 1.350292 | 1.385292 | 17.906791 |

Of 25 declared workloads with native completion receipts, CPU misses on five,
SIMD misses 5× on sixteen, and GPU misses SIMD latency on all 25. The remaining
resident row measures cached materialization and cannot prove fresh execution.
The standard/quick aliases share the same measured pipeline, so these row
counts are not independent operation completions. The RGB 256 × 256 SIMD
median improved from 0.260104 to 0.033521 ms (7.76×); large CPU improved from
0.663042 to 0.433417 ms and GPU from 3.460625 to 1.652208 ms. Short benchmark
samples fluctuate; the fresh-input run supplies a separate longer boundary.

That diagnostic creates two fresh 1024 × 768 images per request. Request timing
includes construction, allocation, transfers, synchronization and terminal bytes;
window throughput also includes worker scheduling and receipt capture. Each of
16 changing input pairs is checked against live
Pillow outside timing. Five warmup windows precede five samples of twenty
windows at each queue depth. All target requests execute on their named backend
without fallback; GPU records exactly one dispatch. Source and runtime binary
identities remain consistent during each run. Queue-one median milliseconds:

| Mode | Pillow | CPU | SIMD | GPU | SIMD speedup over Pillow |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 0.556000 | 0.099708 | 0.104813 | 0.462834 | 5.30× |
| RGB | 2.666104 | 0.406167 | 0.305958 | 1.084979 | 8.71× |

The corresponding baseline GPU medians were 2.896667/2.797625 ms: improvements
of 6.26×/2.58×. The 5.30×/8.71× figures describe request medians, not the complete
window: queue-one throughput ratios are 4.73×/7.72×. Native L transfers now use
786,432 bytes for each primary input,
secondary input and result; RGB uses 2,359,296 bytes each, plus 16 parameter
bytes. Readback still allocates the host result. This is reduced transfer and
conversion work, not a zero-allocation or zero-readback claim. Aggregate fresh
completed images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1,764 | 8,839 | 8,339 | 2,112 |
| L | 2 | 1,976 | 11,776 | 10,816 | 3,461 |
| L | 4 | 1,992 | 13,309 | 13,006 | 4,615 |
| RGB | 1 | 370 | 2,391 | 2,855 | 861 |
| RGB | 2 | 404 | 3,264 | 3,144 | 1,272 |
| RGB | 4 | 427 | 3,665 | 3,618 | 1,387 |

Multiply remains incomplete. Next-visit decisions and blockers:

- **Tiny requests:** host construction, validation and terminal overhead still
  exceed Pillow on several rows. Attribute those stages before changing the
  now-cheap arithmetic loop; wider vectors cannot remove a fixed call floor.
- **SIMD support and bandwidth:** La and unequal dimensions still use CPU
  fallback. Add exact native stride handling and mode admission separately.
  SIMD misses 5× on several uniform/composed workloads even though the two
  fresh-input cases meet it. Measure copies/allocations and the 4 MiB scheduling
  crossover across more shapes and hardware before further instruction tuning.
- **GPU throughput:** queue-one latency remains 4.42× SIMD for L and 3.55× for
  RGB; throughput remains lower at every measured queue depth. Profile auxiliary
  staging, command encoding, mapping and completion. The native-byte path is
  currently little-endian and single-operation; mixed chains still expand to
  packed RGBA. Preserve per-step truncation if extending fusion or residency.
- **Long pipelines:** the 260-operation chain still misses all targets. Count
  graph construction, repeated auxiliary preparation and dispatches before
  extending exact fusion. Existing intermediate observations must remain valid.
- **Outstanding proof:** additional modes, shapes, bindings and hardware;
  ingestion of focused receipts into the complete operation matrix; pre-push
  Rust/docs/cross-runtime checks. No operation has every campaign goal proven.

The optimization skill records when row boundaries are unnecessary and how
packing independent samples removes transport expansion. The fixture index now
contains 12,412 cases, 24 static plans, 773 benchmark workloads and 54 suites.
No coverage was collected. The next independent operation visit is Alpha
Composite; Screen's shared improvements do not mark its own goals complete.

## Alpha Composite four-attempt checkpoint

On 2026-09-25, both `Image.alpha_composite` and the in-place image method
received a bounded optimization visit. The initial maintained cohort passed
318 comparisons. Seventy new input-only cases also passed before changes:
LA/RGBA varied colors, every source/destination alpha pair, transparent payloads,
vector tails, empty images, rejected modes and cropped/out-of-bounds method
arguments. Exhausting the alpha pairs does not exhaust all color combinations.
No parity discrepancy was found in this cohort.

Four attempts were retained:

1. **CPU ownership and shared arithmetic:** borrow the source materialization
   and its native pixels instead of cloning and converting it twice. Compute
   the fixed-point coefficient once per pixel and reuse it across color bands.
   A transparent source still preserves every destination byte.
2. **SIMD coefficient and layout:** load complete LA/RGBA pixels as packed
   words, extract channels with shifts, and replace eight f64 divisions with
   f32 division plus exact integer correction. The numerator is
   `(source_alpha * 65025) * 128`; its significant integer is below 2²⁴, so
   both division operands are exactly representable. The quotient is at most
   32640, and rounding can only raise its truncated value by one. Comparing
   `quotient * denominator` with the original numerator corrects that case;
   every product fits below 2³¹. A focused Rust test checks all 65,536 alpha
   pairs against integer division. Constant vectors stay outside the hot loop.
   Release assembly has packed f32 divides, integer products and masks, with
   no memory-fill calls in either pixel loop. Pixels stream across row
   boundaries; 64 KiB tiles are scheduled only from 1 MiB of output onward.
3. **Full-canvas method:** after the existing coordinate, crop and empty-image
   validation, a composite covering the entire destination queues the composite
   directly. Its following unmasked full-canvas paste would only copy that
   result. Cropped destinations retain their existing crop/composite/paste
   behavior. Immutable graph ownership protects shared source data.
4. **GPU transport:** extend the existing pooled native Multiply transfer path
   to Alpha Composite. An internal shader mode packs two complete LA pixels
   into each storage word, preserving their individual fixed-point arithmetic;
   RGBA retains its ordinary packed pixel. Upload the overlay through the
   mapped-input path where supported and initialize the destination buffer
   directly. Bounds guard the final partial dispatch row, and host output
   excludes alignment padding. This path applies only to one admitted operation
   with matching native inputs on little-endian targets; other batches keep
   the ordinary GPU path. Multiply keeps its existing four-binding shader.

The final maintained cohort passes **528/528 comparisons**; six large varied
inputs immediately below/at/above the SIMD tiling threshold pass another
**18/18**. The shared-transfer change also passes **345/345 Multiply mode
comparisons**. These are local release-extension results, not cross-platform
proof. In the 70 added Alpha Composite cases, 52 reach recorded CPU execution;
GPU executes those 52 natively. SIMD executes 48 natively and uses explicit CPU
semantic fallback for four 1 × 1 cases. The other 18 cases produce no recorded
arithmetic. Passing fallback is not acceleration.

Local receipts under `build/migration-parity/` use the prefix
`perf-alpha-composite-20260925-`: `checkpoint-parity.json`,
`tiles-parity.json`, `initial-throughput.json`, and
`checkpoint-throughput.json`. Related-operation evidence is
`perf-multiply-20260925-alpha-regression-parity.json`. The unchanged ten-workload
benchmark has initial run `migration-benchmark-946e57a65cdf409fa184835970cf0881`
and final run `migration-benchmark-64905846cd40441798e4ba7df75762de`.
Final median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.022229 | 0.036292 | 0.031708 | 0.866667 |
| Operation matrix 32 × 24 | 0.020667 | 0.030063 | 0.031062 | 0.890958 |
| Composed matrix 009 | 0.032771 | 0.033062 | 0.033854 | 1.054604 |
| Composed matrix 021 | 0.023771 | 0.159584 | 0.135062 | 1.141188 |
| LA 256 × 256 | 0.173125 | 0.086771 | 0.060062 | 0.238146 |
| RGBA 256 × 256 | 0.189000 | 0.140729 | 0.086458 | 0.252083 |
| LA 1024 × 768 | 2.281333 | 0.521230 | 0.490354 | 1.164438 |
| RGBA 1024 × 768 | 2.485750 | 0.967604 | 0.710562 | 2.231834 |
| In-place method standard | 0.019209 | 0.028396 | 0.030042 | 0.636479 |

These nine rows have native completion receipts. CPU misses on five, SIMD
misses 5× on all nine, and GPU misses SIMD latency on all nine. The tenth
module-standard row does not materialize pixels and cannot prove backend speed.
The 256² SIMD medians improved 3.62× for LA and 2.19× for RGBA relative to the
initial implementation. Short samples and different stimulus/timing boundaries
must not be mixed with the longer fresh-input diagnostic.

The fresh-input diagnostic creates two new 1024 × 768 images per request,
composites them, and exports terminal bytes. Both initial and final runs pass
**40,320/40,320 exact output checks**, including warmup, with 38,400 measured
completions each. All target requests record the requested native backend and
one operation; GPU records one dispatch. Each run retains unchanged source and
binary identities and consistent binaries across subjects. Request latency
includes construction, allocation, transfer, synchronization and output export;
window throughput additionally includes scheduling and receipt capture.
Queue-one median milliseconds:

| Mode | Pillow | CPU | SIMD | GPU | SIMD speedup over Pillow |
| --- | ---: | ---: | ---: | ---: | ---: |
| LA | 2.768583 | 0.435395 | 0.421208 | 0.789438 | 6.57× |
| RGBA | 2.070625 | 0.809875 | 0.691979 | 1.383562 | 2.99× |

Initial GPU request medians were 3.460145/2.675771 ms: improvements of
4.38×/1.93×. Final GPU primary, secondary and result byte counts are each
1,572,864 for LA and 3,145,728 for RGBA, with 24 parameter bytes. LA previously
used 3,145,728 bytes for each image. Native transport eliminates that expansion;
readback and host result allocation still occur. The zero host-buffer counters
in this GPU receipt are not proof of zero allocations.
Aggregate completed images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| LA | 1 | 354 | 2,116 | 2,151 | 1,236 |
| LA | 2 | 423 | 2,399 | 2,456 | 1,266 |
| LA | 4 | 467 | 2,442 | 2,464 | 1,254 |
| RGBA | 1 | 468 | 1,138 | 1,304 | 703 |
| RGBA | 2 | 460 | 1,251 | 1,448 | 735 |
| RGBA | 4 | 471 | 1,272 | 1,473 | 721 |

Alpha Composite remains incomplete. The next visit should address:

- **Small calls and composed workflows:** CPU still loses to Pillow on five
  rows. Attribute host construction, repeated validation, graph traversal and
  terminal work; widening the pixel loop will not remove their fixed cost.
  Matrix 021 needs its remaining stages attributed separately.
- **SIMD copies and arithmetic:** RGBA misses 5× even on the large fresh-input
  boundary. Measure allocation/copy cost versus coefficient division, packing
  and task scheduling before another instruction change. Four 1 × 1 cases
  still route to CPU; those passes are not native SIMD proof.
- **GPU completion and throughput:** queue-one request latency remains
  1.87×/2.00× SIMD for LA/RGBA. More host concurrency provides little additional
  GPU throughput here. Profile queue writes, mapping, command encoding and
  serialized waits. Composed/cropped pipelines still use ordinary transport;
  a fused regional compositor must preserve crop fill, clipping and every
  intermediate rounding rule.
- **Benchmark attribution defect:** the four
  `pipeline-chain.alpha-composite.{la,rgba}-{256x256,1024x768}` workloads invoke
  module `PIL.Image.alpha_composite`, but their requirement mapping points to
  the in-place method. They are module-composite evidence, not proof of the
  full-canvas method optimization. The workloads, requirements and thresholds
  were left unchanged for this visit; correct the mapping in a separate audit
  and add genuine in-place performance evidence without discarding these rows.
- **Outstanding campaign proof:** additional shapes, platforms and bindings;
  ingestion of focused receipts into the complete matrix; pre-push Rust/docs
  checks. No public operation has every performance goal demonstrated.

The optimization skill now explains when a bounded approximate quotient can
be corrected to an exact integer result, and when complete dependent tuples
can share a transfer word. Static fixture indices now contain 12,482 cases,
24 plans, 773 benchmark workloads and 54 suites. No coverage was collected.
The next independent operation visit was Contrast, recorded below.

## Contrast parity and performance checkpoint

On 2026-09-25, the original 147 maintained comparisons passed, but a varied
19-mode audit found **408 failures in 657 comparisons**. The old arithmetic
used a binary64 weighted sum, while Pillow blends using a narrowed float32
factor and fused multiply-add. Its constructor also retains an observable base
image: later source mutations change the input, not the saved mean or alpha.
The base differs by mode, including `(0, 0, mean)` for HSV,
`(mean, 128, 128)` for YCbCr and `(0, 0, 0, 255-mean)` for CMYK. RGBX blends its
X byte against 255. RGBa rejects conversion from L during construction; typed
integer/float modes can construct an enhancer and reject enhancement later.

This visit retained four areas of work, with corrective iterations for RGBX,
RGBa error timing and the lazy core API before recording final measurements:

1. **Preserve construction and mutation semantics.** The Python Contrast
   object now retains its public `degenerate` image and delegates enhancement
   to the existing Rust module blend. Changes or replacements to `image` and
   `degenerate` remain observable. Algorithms stay in Rust. The one-shot Rust
   API keeps its lazy Contrast descriptor so selecting a backend afterward
   still controls pending source operations.
2. **Remove intermediate frames from base construction.** For native L, LA,
   RGB, RGBA, RGBX and CMYK, reduce the exact quantized luminance directly from
   borrowed native pixels. Allocate the constant base once and copy alpha only
   where required. This avoids a grayscale image and a second conversion of
   that constant image. Other modes retain their conversion/error contract.
3. **Use exact backend arithmetic.** Direct CPU/SIMD Contrast builds per-band
   byte LUTs with the reference float32 expression and reuses existing LUT
   kernels. The GPU Contrast shader uses the same fused expression. Removing
   its old 65,536-pair admission scan eliminates repeated work that checked an
   incorrect arithmetic contract. The host still computes the midpoint.
4. **Borrow the blend source.** The CPU module blend reads a shared
   materialization instead of cloning its second input. This also benefits
   the stateful public Contrast path, whose terminal operation is BlendModule.

The expanded maintained cohort passes **876/876 exact comparisons**, including
243 added input-only cases for varied pixels, constructor observations,
repeated enhancement, source mutation and empty images. Another **120/120**
observations pass for source/base mutation and replacement, and **372/372**
related module-blend/Color comparisons pass. Four focused Rust tests pass:
direct CPU/SIMD/GPU kernels over five modes and eight factors, both pending
PutPixel midpoint paths, and empty CMYK. The kernel test compares against the
already-audited constructor-plus-blend path; it is not an independent oracle.
GPU-unavailable hosts may skip that portion of the test.

Passing parity does not establish native ownership for every case. Among the
243 added cases, 33 record no arithmetic receipt. The other 210 cases record
CPU execution; SIMD records 195 with only SIMD receipts and 15 with CPU
conversion fallback. GPU records 157 with only GPU receipts and 53 with some
CPU fallback. These counts describe recorded operations: native base reduction
and allocation occur on the host and are not separate pipeline receipts.

Local receipts use the prefix `build/migration-parity/perf-contrast-20260925-`:
`varied-initial-parity.json`, `checkpoint-parity.json`, `state-parity.json`,
`checkpoint.json`, and `checkpoint-throughput.json`. Shared-path evidence is
`perf-contrast-related-20260925-checkpoint-parity.json`. The four unchanged
benchmark workloads have initial run
`migration-benchmark-627f51d39b624e5baa010dc25394b4a3` and final run
`migration-benchmark-685846c3349341a690eda3c56fa489dc`. Final median milliseconds:

| Materialized workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Operation | 0.029229 | 0.015542 | 0.016292 | 0.293188 |
| Operation matrix 32 × 24 | 0.027292 | 0.014896 | 0.015230 | 0.312542 |

Both rows complete the final blend on the requested backend without fallback.
GPU improves 3.65×/2.54× from 1.071209/0.794708 ms. CPU beats Pillow on these
rows; SIMD misses 5× and GPU misses SIMD latency on both. The constructor and
unmaterialized enhancement standard rows have no completed backend receipt and
cannot prove backend performance. Their final CPU/SIMD/GPU medians are
0.009375/0.009584/0.009500 ms and 0.008792/0.009167/0.008958 ms, respectively.

The fresh-input diagnostic includes `frombytes`, construction of the Contrast
enhancer, `enhance(0.3)` and terminal bytes. It passes **40,320/40,320 exact
output checks**, including warmup, with 38,400 measured completions. Source and
runtime identities remain unchanged. Each target request's terminal receipt
records the requested backend and one blend; GPU records one dispatch. This
does not make the host base construction a GPU or SIMD kernel. Queue-one
request median milliseconds, including all those host costs:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU | SIMD speedup over Pillow |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 0.683542 | 0.435396 | 0.862021 | 3.102604 | 0.79× |
| RGB | 2.836375 | 0.933938 | 2.481313 | 3.172688 | 1.14× |

Aggregate completed images per second includes worker scheduling and receipt
capture, in addition to each request's work:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1,463 | 2,225 | 1,147 | 320 |
| L | 2 | 1,744 | 3,254 | 1,323 | 327 |
| L | 4 | 1,860 | 3,974 | 1,422 | 332 |
| RGB | 1 | 350 | 1,022 | 399 | 306 |
| RGB | 2 | 402 | 1,410 | 457 | 332 |
| RGB | 4 | 426 | 1,666 | 484 | 342 |

Contrast remains incomplete. Checkpoint blockers and next investigations:

- **Public SIMD blend:** the large stateful path is 1.98×/2.66× slower than
  CPU for L/RGB. Its terminal operation is BlendModule, so changing the direct
  Contrast LUT kernel cannot fix this boundary. Attribute its vector packing,
  widening, task scheduling and full-frame reads/copies first. The small-call
  cost also prevents 5× on the maintained rows.
- **Observable saved base:** a constant-base specialization must survive
  source mutation while invalidating when the public base changes or is
  replaced. Fusing the mean with the final blend would recompute state that
  Pillow intentionally snapshots. Measure base creation separately before
  choosing a specialization or a retained representation.
- **GPU transport and ownership:** each fresh L/RGB request uploads, reads back
  and supplies an auxiliary buffer of 3,145,728 bytes each, with 256 parameter
  bytes. L expands fourfold, and RGB expands to four bytes per pixel. The
  receipt also records a full-frame copy and mode conversion. Reuse the native
  binary transfer path where its shader contract fits, then attribute mapping,
  queue writes and waits. GPU remains 3.60×/1.28× slower than SIMD and has lower
  sustained throughput. Zero host-buffer counters do not mean zero allocation.
- **Direct kernels and remaining proof:** LUT setup and interleaved two/four
  channel gathers still need size-dependent measurement; logical-mode and
  empty-image fallbacks remain. Direct one-shot core kernels need their own
  performance evidence, beyond Python's stateful blend. More shapes, factors,
  bindings and platforms, integration into the complete results matrix, and
  pre-push checks remain outstanding.

The reusable skill now explains constructor snapshots versus live inputs,
conversion fused into exact reductions, and the setup/gather tradeoff for
finite-domain lookup tables. Static input indices contain 12,725 cases,
24 plans, 773 workloads and 54 suites. No coverage was collected. This visit
ends at its checkpoint; the next independent operation is Solarize. No public
operation has every performance target demonstrated.

## Solarize four-attempt checkpoint

On 2026-09-25, Solarize's original 312 comparisons passed, but a varied
threshold/mode audit found **669 failures in 741 comparisons**. The binding
narrowed thresholds to u8 and treated explicit `None` as omission. Pillow instead
compares each byte with the original threshold before checking the image mode.
It accepts fractional and out-of-range numbers, inverts every byte for NaN,
and propagates custom comparison errors. Only L/RGB are supported; P has a
separate NotImplementedError. Other modes must fail at the public call.

Four areas were attempted, then this visit stopped:

1. **Parity repair:** retain numeric range before narrowing, use a ceiling for
   fractional cutoffs, and handle the all/none cases. Exact built-in type guards
   preserve subclass hooks. For custom values, the binding collects all 256
   comparison results in reference order; Rust constructs and applies the LUT.
   Clone the input handle after callbacks so their mutations remain visible.
   The wrapper also retains Pillow's shallow metadata copy. The existing Rust
   u8 API remains, with a host-neutral input variant for the broader contract.
2. **CPU native bytes:** collect transformed L/RGB bytes directly into one
   output. L no longer expands to RGB and converts back; RGB avoids cloning
   before transforming. Other internal layouts retain their existing path.
3. **SIMD trial, reverted:** direct vector writes through reserved Vec appends
   removed a data pass but regressed L/RGB request medians from
   0.065916/0.285292 to 0.120542/0.430500 ms. Release assembly retained a
   capacity branch and a length store for each 16-byte append. A comparison/XOR
   simplification also showed no consistent whole-request gain. The entire
   SIMD trial was removed; no SIMD speedup is claimed from that attempt.
4. **GPU native transport:** reuse the existing Multiply/Alpha Composite
   transfer machinery for one L/RGB Solarize operation. Four independent bytes
   share a shader word, including bytes crossing RGB pixel boundaries. Guard
   the final partial dispatch row and exclude transfer padding from output.
   The unary path has no secondary upload and keeps the three-binding shader.
   Composed pipelines continue through their ordinary fusion/transport path.

The retained release build passes **1,251/1,251 maintained comparisons** over
417 cases, including 313 added input-only cases. The custom comparison and
metadata audit passes **78/78** observations. Shared transport regressions pass
**606/606 Multiply** and **528/528 Alpha Composite** comparisons. In the 313
added cases, 227 record no arithmetic receipt; CPU executes the other 86.
SIMD records 78 native cases and eight empty-image CPU fallbacks. GPU records
74 native cases and 12 empty-image CPU fallbacks. Error parity and fallback
passes are not acceleration proof.

**Benchmark preflight defect:** the manifest's reflected integer annotation
excluded fractional thresholds accepted by live Pillow and prevented the new
negative-type cases from running through the normal validator. The generator
now includes number, boolean, null, string and sequence inputs for this parameter
only. Existing outputs, errors, tolerances, requirements and benchmark workloads
are unchanged; no cases were removed. Regeneration also refreshed two existing
Add/Subtract target signature annotations from the current binding. The final
Solarize parity receipt uses the updated manifest. Static indices now contain
13,038 cases, 24 plans, 773 workloads and 54 suites; no coverage was collected.

Local receipts use `build/migration-parity/perf-solarize-20260925-` with
`varied-initial-parity.json`, `checkpoint-parity.json`, `host-parity.json`,
`repaired-throughput.json`, `native-throughput.json`, and
`checkpoint-throughput.json`. The rejected SIMD trial remains in the native
receipt and the intermediate `xor-gpu-throughput.json`. Related receipts use
`perf-{multiply,alpha-composite}-20260925-solarize-regression-parity.json`.
The unchanged 26-workload benchmark has initial run
`migration-benchmark-7b273776da7b48a19efd8b454d4d465d` and final run
`migration-benchmark-47b2915e03574d119b133f39c3e2b551`. Representative final medians in milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| `pipeline-op.solarize.benchmark-materialized` | 0.039397 | 0.012875 | 0.013479 | 0.280834 |
| `pipeline-op.solarize.matrix-32x24` | 0.040625 | 0.011479 | 0.012103 | 0.279646 |
| `pipeline-chain.simd-lut.l.1024x768` | 1.182916 | 0.317229 | 0.365313 | 2.173355 |
| `pipeline-chain.simd-lut.l.1024x1024` | 1.544125 | 0.366999 | 0.399417 | 2.713604 |
| `pipeline-chain.simd-lut.rgb.1024x768` | 3.488604 | 0.903292 | 0.757750 | 1.821438 |
| `pipeline-chain.simd-lut.rgb.1024x1024` | 4.655126 | 1.175354 | 0.909271 | 2.217084 |

All 25 materialized rows have native completion receipts. CPU misses its
target on 8, SIMD misses 5× on 24, and GPU misses SIMD latency
on 25. The remaining standard row is unmaterialized and cannot prove
backend performance. The selected composed rows include other operations;
changing Solarize alone does not remove their host/LUT preparation costs.

The fresh-input diagnostic includes image construction, Solarize at threshold
128, allocation, transfers, synchronization and output bytes. Both the repaired
baseline and retained checkpoint pass **40,320/40,320 exact output checks**, with
38,400 measured completions each. Source and runtime identities remain unchanged
within each run. Every target request records the requested backend and one
operation; GPU records one dispatch. Queue-one request median milliseconds:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU | Pillow / SIMD |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 0.334917 | 0.058041 | 0.066062 | 0.343063 | 5.07× |
| RGB | 1.397167 | 0.269417 | 0.217854 | 0.571105 | 6.41× |

Relative to the parity-correct baseline, CPU improves
13.80×/1.89× for L/RGB, and GPU improves
4.53×/2.21×. Final GPU upload and readback are each
786,432 bytes for L and 2,359,296 for RGB, down from 3,145,728 each. Parameters
are 20 bytes and auxiliary bytes are zero. Native transport removes the format
expansion; readback allocation and a full-frame copy remain. Zero host-buffer
counters do not establish zero allocations.
Aggregate completed images per second, including scheduling and receipt capture:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 2,911 | 14,971 | 13,434 | 2,975 |
| L | 2 | 4,578 | 17,553 | 16,687 | 4,858 |
| L | 4 | 5,974 | 18,411 | 17,517 | 6,940 |
| RGB | 1 | 739 | 3,380 | 3,918 | 1,657 |
| RGB | 2 | 1,163 | 5,284 | 5,032 | 2,393 |
| RGB | 4 | 1,418 | 5,110 | 4,973 | 2,865 |

Solarize remains incomplete. Checkpoint blockers:

- **Small and composed calls:** CPU still loses on 8 maintained rows.
  Attribute graph construction, per-band LUT composition, wrapper validation
  and terminal export before changing the byte loop again. Native Solarize
  transport does not accelerate a batch already fused into a generic Eval.
- **SIMD output construction:** the retained kernel still copies before
  transforming. Its replacement must avoid both that pass and per-vector
  growth bookkeeping. Check exact-size collection/vectorization or an existing
  owned-buffer path before adding unsafe initialization. A short-input result
  or one near-5× large sample does not demonstrate the goal across all rows.
- **GPU latency and throughput:** transfer reduction helps, but mapping,
  submission, synchronization and host allocation remain. Additional queued
  work improves throughput here but still falls below SIMD. Profile those
  stages and composed native transport before another shader arithmetic change.
- **Unproven scope:** empty-image native paths, additional shapes, bindings and
  platforms, global matrix ingestion, and pre-push Rust/documentation checks
  remain. No operation has every campaign target demonstrated.

The optimization skill records why reserved append loops can lose to a copy
plus a vector loop, and when exact-type specialization must preserve host
comparison order, state mutation and parameter range. The next independent
visit is module `PIL.Image.blend`; the earlier Blend checkpoint covered
`ImageChops.blend`.

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

## Image.blend four-attempt checkpoint

On 2026-09-25, module-level `Image.blend` received three parity repair
iterations and one SIMD optimization attempt. This is separate from the earlier
`ImageChops.blend` checkpoint. The original 80 cases passed 240 comparisons,
but broader modes exposed rejected La/LAB inputs and missing first-image metadata.

1. Admit La with LA and LAB with the three-channel family, preserving the first
   image's output mode and existing error precedence. Copy the first image's
   metadata mapping shallowly, including at alpha 1. Alpha can extrapolate;
   the Rust documentation no longer incorrectly says it is clamped to 0–1.
2. Remove the LAB A/B storage bias independently from each LAB operand before
   blending, then restore it for a LAB result. Keep LAB on exact CPU execution
   when SIMD/GPU cannot implement those semantics. The GPU guard also checks
   a LAB second operand when the first image uses ordinary RGB storage.
3. Preserve a merge's logical output mode through queued operations. Previously
   `merge("LAB", ...)` produced the correct raw storage but advanced the executor
   mode to RGB. A following blend therefore interpreted the A/B samples wrongly.
4. Replace scalar per-lane extraction, clamp/conversion and byte stores after
   SIMD FMA with packed widening, clamp, truncation, narrowing and a sixteen-byte
   store. Stream across rows, retain the eight-byte admission boundary, pad only
   the final 8–15-byte block, and handle a shorter final remainder scalarly.

The final expanded suite passes **876/876 exact comparisons** (292 cases),
including **212 added input-only blend cases**. Related merge, Contrast and
Color paths pass **1,161/1,161 comparisons**. A separate probe compares every
one of the 65,536 byte pairs at 16 factors, including extrapolation, near-one
factors, large finite factors and nonfinite factors. Together with lengths
1–33 it passes **147/147 comparisons** across backend selections. Metadata,
alpha endpoints, nonfinite factors and independent result ownership pass
another **42/42**. Unsupported acceleration still uses the exact fallback;
these counts are not claims that every case executes natively.

Among the 212 added blend cases, 77 record no arithmetic backend (including
rejected inputs). CPU records execution on the other 135. SIMD records 108
cases with SIMD receipts and 27 with CPU receipts; GPU records 52 with GPU
receipts and 83 with CPU receipts. LAB, other logical-mode restrictions, tiny
inputs and setup operations remain part of the ownership gap.

The audit also found independent constructor failures: `Image.new("LAB", (0, 1))`,
`Image.new("LAB", (1, 0))`, and raw `Image.frombytes("LAB", ...)` succeed in
Pillow and are rejected by the target. They are retained as **three permanent
constructor regression inputs**, with their failing receipt under
`perf-lab-constructor-20260925-blockers-parity.json`. LAB blend cases use
`merge("LAB", [L, L, L])`, including zero-size L bands, to reach blend itself.
Those passing blend results do not resolve the constructor defects. The initial
120 failures in 636 varied comparisons included six comparisons stopped by
zero-size LAB construction; the remaining 114 reached a blend discrepancy.

Local evidence uses `build/migration-parity/perf-image-blend-20260925-`:
`varied-initial-parity.json`, `checkpoint-parity.json`, `domain-parity.json`,
`host-parity.json`, `repaired-throughput.json`, `checkpoint-throughput.json`,
and `checkpoint.json`. Shared-path evidence is
`perf-image-blend-related-20260925-checkpoint-parity.json`. An initial sandboxed
throughput run could not access Metal and is retained as
`sandbox-unavailable-throughput.json`; it is excluded from performance results.
The completed throughput and final parity runs used adapter access.

The three maintained benchmark workloads and their thresholds are unchanged.
Initial run `migration-benchmark-e07ab1ef1741430b8599dfcb8d942bd8`; final run
`migration-benchmark-e6e94342840d46998493d94bcd838966`. Final median milliseconds:

| Materialized workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Operation | 0.015229 | 0.018625 | 0.017771 | 0.225417 |
| Operation matrix 32 × 24 | 0.014313 | 0.017729 | 0.017209 | 0.216188 |

Both materialized rows complete on the requested backend without fallback.
CPU misses Pillow, SIMD misses 5×, and GPU misses SIMD on both. The standard
row has no completed backend receipt and cannot prove backend performance;
its Pillow/CPU/SIMD/GPU medians are 0.010042/0.012250/0.012396/0.012333 ms.

The fresh-input diagnostic includes two new `frombytes` images, module blend
at alpha 0.3, and terminal bytes. Both before/after runs pass **40,320 exact
output checks**, including warmup, with 38,400 measured completions each.
Source and runtime identities stay unchanged during each run. Every target
request records its requested native backend; GPU records one dispatch. Final
queue-one request medians include construction, allocation, transfer and waits:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU | Pillow / SIMD |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 0.537771 ms | 0.211271 ms | 0.388729 ms | 2.953438 ms | 1.38× |
| RGB | 2.715104 ms | 0.532313 ms | 1.226521 ms | 2.806729 ms | 2.21× |

SIMD improves **1.68×/1.72×** from its parity-correct baseline of
0.654438/2.108021 ms. CPU is essentially unchanged at this boundary; the
retained CPU change repairs LAB semantics. GPU transport was not optimized
in this visit. Completed images per second, including scheduling and receipts:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1,825 | 4,464 | 2,475 | 336 |
| L | 2 | 1,968 | 5,261 | 2,497 | 334 |
| L | 4 | 1,989 | 5,208 | 2,606 | 325 |
| RGB | 1 | 372 | 1,680 | 805 | 345 |
| RGB | 2 | 410 | 1,911 | 842 | 350 |
| RGB | 4 | 435 | 1,966 | 840 | 347 |

Remaining blockers and concrete next investigations:

- **SIMD instruction overhead:** the old release loop emits per-lane float
  compares, scalar conversions and byte stores after vector FMAs. The retained
  loop emits vector min/max, truncation, narrowing and one packed store, but
  still calls `_memset_pattern16` twice per sixteen-byte block to construct
  repeated float constants and spills around those calls. Hoist or express
  constants so their construction leaves the hot loop, then inspect widening
  shuffles and compare packed-word extraction. SIMD remains 1.84×/2.30× slower
  than CPU for L/RGB. No second SIMD trial was taken after the visit limit.
- **CPU and small-call cost:** the two small materialized rows remain slower
  than Pillow. Attribute Python wrapping, metadata, graph construction and
  terminal export before changing arithmetic. Large CPU blending still
  zero-initializes output and schedules individual rows; compare exact-size
  output construction and coarse independent chunks at measured crossovers.
- **GPU transport:** both L and RGB upload 3,145,728 bytes, provide another
  3,145,728 auxiliary bytes, and read back 3,145,728 bytes, with 256 parameter
  bytes, one full-frame copy and one mode conversion. Reuse the native byte
  transfer helper for supported BlendModule layouts, keeping all-channel FMA,
  word-count tail guards and the shader's 32-byte parameter layout. L currently
  expands fourfold; RGB expands to four bytes per pixel. Then isolate queue
  writes, mapping and waits. GPU is 7.60×/2.29× slower than SIMD and has lower
  sustained throughput. Zero host-buffer counters do not establish no allocations.
- **Remaining parity and scope:** fix the recorded LAB constructor defects in
  their own operation visits; accelerate LAB only with its per-operand bias
  contract. More shapes, factors, composed pipelines, bindings and platforms,
  integration into the complete results matrix, and pre-push verification remain.

The reusable skill records biased representations across queued stages and
packed conversion/store costs, including vector-constant construction that
lowers to library calls. Static indices contain 13,253 parity cases,
24 coverage declarations, 773 workloads and 54 suites. No coverage collection
ran. This operation is checkpointed as incomplete; the next operation is
Color enhancement. No public operation has every performance target demonstrated.

## Color enhancement checkpoint — 2026-09-25

Four areas were addressed before checkpointing: public state and native
arithmetic parity, transparency metadata, packed SIMD module blending, and
native-byte GPU module-blend transport. Color remains incomplete. The next
operation is `Image.convert`: the stage measurements below identify its two
constructor conversions as the dominant remaining RGB cost. This dependency
takes priority over the next baseline-ranked operation, `Image.putpixel`.

The old 48-case selection passed 144 comparisons, but a varied 219-case probe
passed only 255 of 657 comparisons. The implementation had no observable
`degenerate` base or `intermediate_mode`, used different arithmetic, and did not
preserve constructor snapshots. Pillow aliases the original image for L/LA;
other accepted modes retain a converted base. Later calls read the current
`image` and `degenerate`, including mutations and replacement of either object.
Color now retains that state and delegates public enhancement to module blend.
RGBX's converted padding becomes 255. CMYK conversion uses rounded MULDIV255
before quantized grayscale. Direct CPU/SIMD/GPU Color kernels now use the
reference's fused float32 blend rather than float64 or thousandths arithmetic.
The GPU admission path no longer scans 65,536 sample pairs on every call.

Metadata requires conversion too: an RGB transparent color becomes its gray
RGB tuple, while a palette transparency index becomes the converted gray
index. Packed RGB integer ink is unpacked before component coercion. Byte
transparency follows the reference warning/removal behavior. Pixel conversion
remains in Rust; the Python wrapper preserves public object and metadata state.

The shared SIMD blend now extracts four byte positions from packed vector
words, applies fused float32 arithmetic, clamps/truncates, and packs the words
back. Explicit constants remove the two `_memset_pattern16` calls and associated
spills previously emitted per sixteen-byte block. Release assembly shows packed
arithmetic and stores without calls in the main loop. Byte order and scalar
tails remain explicit. This improves public Color through its module-blend
dependency; it does not establish a new standalone Image.blend timing result.

GPU module blend now reuses the existing native-byte transfer helper. Each word
carries four independent samples, with an explicit word bound for a padded
dispatch row. It preserves all stored channels and the 32-byte parameter layout.
Existing mode/precision preflight and exact fallbacks remain. The final blend
receipt records upload/auxiliary/readback of 786,432 bytes each for L and
2,359,296 bytes each for RGB, zero transport mode conversions, one full-frame
copy, and 32 parameter bytes. Constructor transfers are separate; these counters
are not totals for the complete Color request.

### Parity evidence

All receipts below are local under `build/migration-parity/`. There are 255 new
maintained input-only cases, generated from independent live-oracle workflows:
varied modes/factors, valid empty images, mutation, repeated enhancement, and
native-word/dispatch-row tails. Existing tests and thresholds were not weakened.

| Evidence | Exact comparisons passed | Receipt |
| --- | ---: | --- |
| Maintained Color selection, 303 cases | 909/909 | `perf-color-20260925-checkpoint-parity.json` |
| Constructor alias/snapshot and replacement | 144/144 | `perf-color-20260925-state-parity.json` |
| Direct core Color kernels | 252/252 | `perf-color-20260925-kernel-parity.json` |
| Public metadata and packed transparency | 54/54 | `perf-color-20260925-metadata-parity.json` |
| Shared module-blend regression | 876/876 | `perf-image-blend-20260925-color-regression-parity.json` |
| Exhaustive byte-pair/factor and length probe | 147/147 | `perf-color-20260925-blend-domain-parity.json` |

The last probe exercises every byte pair at sixteen factors, plus lengths
1–33; it refreshed the older untagged domain receipt, so the Color-tagged copy
identifies this binary's evidence. Shared Multiply, Alpha Composite and Solarize
transport probes each pass 384 exact outputs at 1025 × 3, with no timing claim;
their receipts use `perf-color-20260925-*-transfer-check.json`. Nonfinite factors
and unsupported modes can use exact fallback. These parity counts do not prove
native acceleration for every mode or parameter. The previously documented LAB
constructor failures remain outside this repair.

### Measured results and blockers

The four maintained workload definitions are unchanged. Final run
`migration-benchmark-31e1fa4c4f774fe5ac6a3b7926323ecd` is retained as
`perf-color-20260925-checkpoint.json`; its two exact benchmark gates pass.
Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015583 | 0.021334 | 0.023876 | 0.757500 |
| Materialized matrix 32 × 24 | 0.015459 | 0.019397 | 0.023604 | 0.769584 |
| Constructor standard | 0.009750 | 0.013396 | 0.016459 | 0.425188 |
| Enhance standard | 0.009834 | 0.013167 | 0.016333 | 0.397833 |

All target rows record the requested native backend without fallback. The first
two complete one terminal blend and miss all three latency targets. The last
two now execute the two constructor conversions; they do not materialize the
subsequent enhancement and cannot establish complete enhancement performance.
The original faster lazy wrapper skipped observable construction work; its
timings are not a parity-correct optimization baseline. Small-call overhead
remains a CPU blocker after the correctness repair.

The fresh-input diagnostic includes new `frombytes`, Color construction,
`enhance(0.3)`, and terminal bytes. The parity-correct baseline and final runs
each pass **40,320 exact output checks**, including warmup, with 38,400 measured
completions. Both retain stable source/runtime identities. Receipts are
`perf-color-20260925-repaired-throughput.json` and
`perf-color-20260925-checkpoint-throughput.json`. Queue-one medians:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU | Pillow / SIMD |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 0.466625 ms | 0.183562 ms | 0.189021 ms | 0.393646 ms | 2.47× |
| RGB | 2.523917 ms | 0.652375 ms | 4.833458 ms | 3.667854 ms | 0.52× |

SIMD improves from 0.361958/5.354396 ms and GPU from 2.889104/5.468334 ms for
L/RGB. CPU is essentially unchanged. CPU meets the large-input latency target;
SIMD still misses 5× in both modes. GPU meets SIMD latency only in RGB, where
SIMD itself is slower than Pillow. Completed images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 2,116 | 5,293 | 5,066 | 2,610 |
| L | 2 | 2,269 | 5,701 | 4,970 | 2,575 |
| L | 4 | 2,210 | 5,738 | 4,899 | 2,588 |
| RGB | 1 | 398 | 1,417 | 206 | 267 |
| RGB | 2 | 432 | 1,704 | 356 | 345 |
| RGB | 4 | 455 | 1,781 | 549 | 390 |

GPU fails the sustained-throughput target: its best observed throughput remains
below SIMD for both modes. These are completed host requests, not a claim about
simultaneous device kernels.

A separate instrumented stage probe passes 216 exact comparisons and retains
individual constructor/final receipts in `perf-color-20260925-stages.json`.
For fresh RGB 1024 × 768, constructor medians are 0.270146 ms for Pillow,
0.221167 ms for CPU, 3.900209 ms for SIMD and 3.028584 ms for GPU. Each target
constructor records two native conversion operations; GPU records two
dispatches. Final blend/export medians are 0.389167/0.516209/0.916230 ms for
CPU/SIMD/GPU. These separately instrumented medians are diagnostic and should
not be substituted for the complete request measurements. They identify
RGB→L→RGB base construction as the next cost to investigate. Inspect individual
conversion layouts, copies, scheduling and transfers before another blend trial.

Other remaining work: reduce small-call and GPU synchronization/readback cost;
replace scalar gathers and repeated per-channel gray construction in the direct
SIMD Color kernel; extend evidence to remaining metadata forms, composed paths,
bindings and platforms. The final-receipt throughput schema omits earlier
constructor receipts, so use the stage probe for that ownership evidence.
Complete matrix integration and pre-push verification remain pending.

The reusable skill now distinguishes conditional aliases from saved snapshots,
uses constructor timing to redirect optimization toward dependencies, and
compares packed-word byte extraction with widening/shuffle costs. Duplicate
constant-setup advice was removed. Static indices contain 13,508 parity cases,
24 coverage declarations, 773 workloads and 54 suites. No coverage collection
ran. Color is checkpointed as incomplete; no public operation has every target
demonstrated.

## Conversion baseline and parity findings — 2026-09-25

This visit starts after Color commit `cd5c14bd6`. No conversion implementation
attempt has been made yet. The first repair will address mode routing and
premultiplied sample interpretation before performance changes. The visit still
has the same three-to-four-attempt limit; unresolved families must remain
documented when the campaign moves on.

The maintained 803 conversion cases plus six workflow benchmarks pass
**2,427/2,427 comparisons**, retained as
`build/migration-parity/perf-convert-20260925-initial-parity.json`.
A separate varied audit crosses 19 source modes with 20 destination modes at
17 × 3, plus zero-width/height cases for L, LA, RGB and RGBA: 540 cases and
1,620 backend comparisons. It passes **1,099** and fails **521**, retained in
`perf-convert-20260925-varied-initial-parity.json` under that same directory.

| Requested backend | Passing comparisons | Failing comparisons |
| --- | ---: | ---: |
| CPU | 392 | 148 |
| SIMD | 317 | 223 |
| GPU | 390 | 150 |

The varied probe explicitly locks terminal pipelines to the requested backend.
Its SIMD failures include 89 `NotImplementedError` outcomes for unsupported
layouts/modes, which are capability or validation-order gaps, not all pixel
arithmetic failures. The CPU failures already establish independent public
parity defects. Principal groups to repair or retain as blockers:

- Destination admission rejects RGBX, RGBa, La and LAB, including conversions
  Pillow accepts and cases with different required error text.
- RGBa conversion often treats premultiplied channels as straight RGB; La→LA
  also returns different pixels. Some La routes accept inputs Pillow rejects.
- Typed I;16→I/F/P/PA, YCbCr→I;16-family, and PA→P have differing sample or
  palette semantics. Empty RGB→P/PA incorrectly rejects zero dimensions.
- Two additional GPU pixel mismatches appear in CMYK→HSV and YCbCr→HSV.
  Unsupported SIMD routing must be distinguished from an exact CPU fallback.

Seven unchanged benchmark workloads ran as
`migration-benchmark-0a136aca3c714381b82dea21b06a21b2`, retained in
`perf-convert-20260925-initial.json`; its one existing exact gate passes.
Inspection shows that **all seven call `convert()` with no destination mode**.
They validly measure the copy/default-mode behavior, but none records native
conversion execution. They cannot establish RGB→L or L→RGB latency, SIMD
speedup, or GPU capability. This is a workload representativeness gap; the
existing definitions and thresholds were left unchanged.

An additional explicit-conversion stage probe uses 72 fresh requests per
subject/direction, with eight warmup requests, and passes **432/432 exact
comparisons**. It includes fresh `frombytes`, conversion and terminal export;
every target request records one native operation, with one GPU dispatch and
no fallback. Receipt: `perf-convert-20260925-initial-stages.json`. Median total
milliseconds at 1024 × 768:

| Conversion | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB→L | 0.460813 | 0.141417 | 2.019209 | 1.935730 |
| L→RGB | 0.631500 | 0.267167 | 2.237250 | 1.743792 |

This serial, instrumented diagnostic is not sustained-throughput evidence.
Both SIMD directions are substantially slower than CPU and Pillow. The source
inspection identifies repeated scalar gathers and wide constants in RGB→L,
and per-block shuffle-index construction, padding and append bookkeeping in
L→RGB. A previously verified packed grayscale helper already exists and should
be considered before adding another RGB→L implementation. These performance
investigations remain behind the parity repairs. No coverage collection ran.

## Conversion parity checkpoint — 2026-09-25

Four implementation attempts were used, with parity taking priority over the
planned SIMD loop optimization. The visit is checkpointed as incomplete and
the next operation is `Image.putpixel`. Results in this section supersede the
conversion baseline only for their stated inputs and boundaries.

| Varied audit, unchanged 540 cases | Passed comparisons | Failed comparisons |
| --- | ---: | ---: |
| Initial | 1,099 | 521 |
| 1. Mode routing and premultiplied alpha | 1,391 | 229 |
| 2. Direct palette exceptions and typed samples | 1,464 | 156 |
| 3. Empty conversions | 1,540 | 80 |
| 4. Exact GPU HSV arithmetic | 1,542 | 78 |

Retained changes and their reasons:

1. **Direct routes versus normalization.** RGBX, RGBa and La destinations now
   follow the source-specific conversion route. LA→RGBX preserves alpha;
   RGBA→RGBX supplies opaque padding. RGBA→RGBa and LA→La premultiply;
   RGBa→RGBA and La→LA unpremultiply with the existing exact helpers. Other
   RGBa routes normalize through RGB and drop alpha; unsupported La routes
   report the reference's failed La→L conversion. Palette-attached alpha is
   distinct from a pending transparency entry when producing RGBX.
2. **Palette and typed exceptions.** The first normalization trial regressed
   RGBa→P/PA: those direct converters consume stored samples. That exception
   is corrected, with no remaining regression against the original varied
   audit. PA→P preserves indices and palette instead of requantizing. Explicit
   core P→P now copies. I;16/I;16L/I;16B→I/F preserve numeric values above 255;
   I;16N follows the reference's clipped L fallback. Other typed destinations
   use clipped luma and identity palette indices. YCbCr→I;16-family normalizes
   through RGB, unlike the direct Y-band conversion to L.
3. **Empty images.** SIMD conversion admits supported empty layouts and
   records `scalar-control` with no vector blocks. WEB palette conversion
   returns the ordinary palette and empty indices without allocating diffusion
   scratch. These are correctness repairs, not accelerated pixel-work evidence.
4. **GPU HSV.** The old shader changed Pillow's mixed float32/float64
   arithmetic and used approximate device division. Each hue sector has two
   ratios known to be zero or one; eliminating those terms leaves one ratio
   and one correctly rounded sector operation. Integer quotient construction
   produces the exact float32 ratios, including ties to even. The sector is
   represented in units of 2^-24 for wrapping and division by six. An integer
   significand product and shift then reproduce the reference's promoted
   multiply by 255 and truncation. This avoids introducing a general software
   float64 implementation; its integer-loop cost still needs performance work.

### Exact evidence and unresolved parity

There are 571 new maintained input-only cases: the 540-case source/destination
audit, seven alpha-boundary cases, and 24 typed-boundary cases. Known failures
are retained, not removed or relabeled. All receipts below are under local
`build/migration-parity/`.

- The maintained selection now has 1,380 cases including benchmark workflows:
  **4,062/4,140 comparisons pass**. All 78 failures are the 26 unsupported LAB
  destination cases across three backends. Receipt:
  `perf-convert-20260925-checkpoint-parity.json`.
- The unchanged varied audit passes **1,542/1,620**, with the same 78 LAB
  failures. Receipt: `perf-convert-20260925-varied-checkpoint-parity.json`.
- The dependent public Color selection passes **909/909** comparisons.
  Receipt: `perf-color-20260925-convert-regression-parity.json`.
- All **16,777,216 RGB triples** produce exact HSV bytes on actual CPU, SIMD
  and GPU execution. The GPU receipt records one dispatch, no fallback, and
  a complete upload/readback. The independent integer formulation also matches
  live Pillow over that domain. Receipts:
  `perf-convert-20260925-domains-checkpoint.json` and
  `perf-convert-20260925-hsv-integer-proof.json`.
- In the domains receipt, all 24 alpha comparisons and all 72 typed
  comparisons pass. The alpha inputs exhaust 65,536 value/alpha pairs across
  eight conversion routes; typed inputs exhaust every unsigned-16 value across
  four source spellings and six destinations. These eager and fallback routes
  are not all native acceleration evidence.
- The domains receipt totals **105/111 exact comparisons passed**. Six fail
  on metadata: P→RGBX with either RGB or RGBA attached palette and
  `info["transparency"] = 37` loses that dictionary entry on each backend.
  Pixel bytes match, but those cases remain failures. The Python conversion
  wrapper reconstructs metadata from core information and omits the host
  overlay; it needs a contract-aware metadata repair.

LAB remains unsupported in core conversion. The reference routes these
destinations through ImageCms profiles and a color-management transform;
an equivalent Rust implementation is still needed. Further metadata forms,
matrix paths, cross-binding behavior and other platforms remain unproven.

### Performance checkpoint

The seven existing conversion benchmarks still exercise default-mode copies;
their definitions and thresholds are unchanged. A new `--operation convert`
option in the existing fresh-throughput diagnostic explicitly measures L→RGB
and RGB→L. It preserves the 16-frame, queue-depth 1/2/4, five-warmup-window,
five-by-twenty measurement policy and exact live-reference output checks.

`perf-convert-20260925-checkpoint-throughput.json` completes **40,320 exact
output checks**, including warmup, and 38,400 measured requests. Source and
runtime identities remain stable. Every target request records one native
conversion, and GPU records one dispatch without fallback. Queue-one request
medians include fresh construction, conversion, allocation, transfer and export:

| Conversion, 1024 × 768 | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L→RGB | 0.579958 ms | 0.232208 ms | 2.371188 ms | 1.396750 ms |
| RGB→L | 0.572375 ms | 0.237104 ms | 2.220959 ms | 1.823292 ms |

CPU meets the latency target on these large cases. SIMD is slower than Pillow
and misses 5× in both directions. GPU meets SIMD latency here but remains
slower than Pillow. Completed images per second:

| Conversion | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L→RGB | 1 | 1,593 | 3,980 | 414 | 628 |
| L→RGB | 2 | 1,677 | 4,741 | 763 | 844 |
| L→RGB | 4 | 1,701 | 4,497 | 1,315 | 949 |
| RGB→L | 1 | 1,684 | 3,795 | 424 | 501 |
| RGB→L | 2 | 2,584 | 5,783 | 858 | 686 |
| RGB→L | 4 | 3,102 | 7,096 | 1,545 | 987 |

GPU's best measured throughput remains below SIMD in both directions. This is
host request throughput, not proof of simultaneous device kernels. The original
instrumented stage boundary was also rerun: 432 exact comparisons pass in
`perf-convert-20260925-checkpoint-stages.json`. Timings varied across runs,
including Pillow; this parity-focused visit claims no CPU/SIMD speedup from
those between-run differences and does not equate the serial diagnostic with
sustained throughput.

Concrete remaining performance work:

- Reuse the already verified packed grayscale helper for RGB→L rather than
  scalar gathering and reconstructing wide constants in the current converter.
  For layout-only conversion, hoist shuffle plans, load complete blocks directly,
  pad only the final tail, and remove per-block append/capacity bookkeeping.
- GPU uploads and reads back 3,145,728 bytes for both directions, with 256
  parameter bytes, one full-frame copy and one transport conversion. L input
  and L output each need only 786,432 bytes. A native-byte path must account
  for different input/output layouts, not assume the matching buffers required
  by the existing binary-operation helper.
- The earlier Color constructor stage receipt records two conversions with
  6,291,456 bytes uploaded and read back, two copies and two dispatches. Inspect
  mode queries and materialization boundaries before implementing device
  residency or a fused round trip; preserve the intermediate quantized luma.
- Measure the exact HSV shader separately before tuning its integer loops or
  replacing bounded arithmetic with shared tables. Small-call CPU overhead,
  more sizes/distributions, composed pipelines, and the complete results-matrix
  refresh remain pending.

The reusable skill records direct-route versus fallback representation rules
and bounded mixed-precision reductions. Static indices now contain 14,079
parity cases, 24 coverage declarations, 773 workloads and 54 suites. No coverage
collection ran. Pre-push checks have not been run for this local checkpoint;
no push is included. No public operation has every performance target proven.

## Putpixel baseline — 2026-09-25

The next visit starts after conversion checkpoint `ca766a6a3`; no putpixel
implementation attempt has been made yet. Its 161 maintained cases plus one
benchmark workflow pass **486/486 comparisons** in
`build/migration-parity/perf-putpixel-20260925-initial-parity.json`.

Three additional public RGB checks all fail against live Pillow on CPU:
`putpixel((-1, -1), (1, 2, 3))` should address the last pixel but raises an
unsigned-coordinate conversion error; integer `0x332211` should write
`(17, 34, 51)` but writes `(17, 0, 0)`; and the tuple `(1, 2)` should raise
`TypeError` but is accepted. Receipt:
`build/migration-parity/perf-putpixel-20260925-smoke-parity.json`.
The first repair must address coordinate coercion, packed integer ink, and
mode-dependent tuple validation before performance changes.

The two unchanged benchmark workloads ran as
`migration-benchmark-4d3508912ace44198684e757698e021e`, retained in
`build/migration-parity/perf-putpixel-20260925-initial.json`; the existing exact
gate passes. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.010896 | 0.012271 | 0.014500 | 0.321583 |
| Standard | 0.005520 | 0.007459 | 0.007833 | 0.006958 |

Only the materialized row proves execution on the requested native backend,
without fallback. It misses CPU, SIMD and GPU latency targets. The standard
row has no completed native receipt, and neither row proves sustained
throughput. Source inspection shows the CPU executor clones the full image
before writing one pixel; ownership and mutation semantics must be established
before removing that copy. No coverage collection ran.

## Putpixel parity checkpoint — 2026-09-25

Four implementation attempts are complete. This visit repaired input and palette
semantics before optimization; it does not claim an operation-wide speedup.
The next operation is `ImageChops.screen`, the next unvisited image kernel in
the baseline gap order. Font work retains its separate recorded blockers.

1. Preserve signed coordinates and negative indexing; unpack multiband integer
   ink; validate tuple arity and distinguish lists from tuples. Singleton tuples
   follow scalar ink rules. Narrow I samples to signed 32 bits before any float
   conversion, preserving the low bits of large signed integers. Match host
   coercion errors, including C-int overflow and F-mode numeric conversion.
2. Resolve PA RGB(A) colors through the attached palette instead of writing the
   red component as its index. Validate new palette bytes and allocate before
   coordinate checking, retaining allocation on later failure. Preserve RGBA
   palette entries and separate PA pixel alpha from palette alpha.
3. Refresh an already retained public palette object after allocation; preserve
   its identity. Look up numerically equal integer/float keys before rejecting
   float components for a new palette entry. Defer PA alpha coercion until after
   allocation and bounds validation; accept opaque floating alpha on P/RGB
   palettes through the reference's equality check. Reuse unused entries when
   an RGBA palette is full.
4. Honor host background/transparency indices during palette allocation and
   preserve full-PA-palette allocation errors before new-byte validation.

These rules were checked against live Pillow and its
[`getink` implementation](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c),
[public wrapper](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py),
and [palette allocator](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImagePalette.py).
Image algorithms and palette selection remain in Rust; Python/PyO3 handles
host types, metadata, and retained wrapper objects.

### Parity evidence

The independent CPU input audit has 1,040 cases. Failures changed from
**616 → 63 → 52 → 52 → 52** across the baseline and four attempts. The remaining
52 cases cannot construct LAB images in the target; all 988 cases reaching
putpixel pass. The palette-state audit has 280 cases, including retained palette
identity, complete palette bytes, alpha errors, out-of-bounds writes, full
palettes and reserved indices. Its failures changed **224 → 140 → 18 → 0**
from attempt one through four. These are CPU audits, not backend acceleration
proof. Receipts are `perf-putpixel-20260925-audit-*.json` and
`perf-putpixel-20260925-palettes-*.json` under `build/migration-parity/`.

The maintained generator now includes 1,164 additional input-only cases,
including the failing LAB cases and palette boundary cases. With the existing
161 cases and one benchmark workflow, the final run has **1,326 cases and
3,978 backend comparisons: 3,825 pass, 153 fail**. Every failure belongs to
one of 51 LAB setup cases on each of CPU/SIMD/GPU. No other maintained case
regressed. Final receipt:
`build/migration-parity/perf-putpixel-20260925-checkpoint-final-parity.json`.
Color's shared pixel-normalization caller also passes **909/909 comparisons**
in `perf-color-20260925-putpixel-regression-parity.json`.

The benchmark runner initially rejected negative test inputs before executing
anything: the annotation-derived manifest allowed neither null coordinates
nor null/string colors. The manifest generator now explicitly admits those
inputs for this endpoint's error tests. The cases, exact expected-error
comparison and performance workloads were retained. Contract validation passes;
the final manifest SHA-256 is
`516ae7f0dc786e5f95b6c09ea869ec3d0d68b9341af1e25350343deb2ed0c0f6`.

### Performance evidence and blockers

Unchanged benchmark run `migration-benchmark-03715a3b5e5c4b9190c4f2e7d556dc78`
is retained in `perf-putpixel-20260925-checkpoint.json`; its exact benchmark
gate passes. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.012396 | 0.014146 | 0.014291 | 0.271250 |
| Standard | 0.005750 | 0.007416 | 0.007771 | 0.007459 |

The materialized row completes on each requested native backend without
fallback, but misses all three latency targets. The standard row still has no
completed native receipt. Small-row CPU/SIMD backend time is only 0.000813 /
0.001146 ms respectively; most public latency lies outside those executors.

The maintained throughput diagnostic now accepts `--operation putpixel`.
It constructs each fresh input, writes its center pixel, and exports the entire
result within timing. Its unchanged 16-frame windows, queue depths 1/2/4 and
sample policy produced **40,320 exact comparisons**, including 38,400 measured
requests, with stable source identity and native receipts without fallback.
Receipt: `perf-putpixel-20260925-checkpoint-throughput.json`.
Queue-depth-one median end-to-end milliseconds at 1024 × 768:

| Mode | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.087688 | 0.068354 | 0.067021 | 2.045729 |
| RGB | 0.929896 | 0.349396 | 0.352188 | 1.419855 |

CPU passes on these large inputs. SIMD reaches only 1.31× / 2.64× Pillow,
missing 5×. GPU exceeds SIMD latency. Completed images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 10,060 | 12,959 | 13,119 | 404 |
| L | 2 | 9,580 | 14,517 | 14,280 | 708 |
| L | 4 | 9,200 | 14,501 | 14,476 | 913 |
| RGB | 1 | 1,013 | 2,645 | 2,668 | 639 |
| RGB | 2 | 1,334 | 3,122 | 3,958 | 1,039 |
| RGB | 4 | 1,420 | 3,010 | 4,260 | 1,295 |

GPU throughput remains below SIMD at every depth. Each GPU request uploads and
reads back 3,145,728 bytes, plus 256 parameter bytes, one full-frame copy and
one mode conversion. L contains only 786,432 native bytes. Its median GPU
backend time is 1.964542 ms; RGB backend time is 1.167521 ms.

Next-visit optimization decisions:

- CPU clones the entire DynamicImage and SIMD copies the entire native byte
  buffer before changing one pixel. The SIMD store already uses one masked
  vector block. Investigate uniquely owned storage and detaching shared storage
  once before tuning that store; retain snapshots and pending pipeline readers.
  Consecutive writes should reuse an owned intermediate when permitted.
- GPU format expansion, transfers and synchronization dominate this sparse
  update. Investigate native byte transport and updates to uniquely owned
  resident storage. Full result export still requires readback. A CPU fallback
  or a submission-only timing cannot meet the GPU requirement.
- Reduce the measured small-call host cost only after profiling construction,
  binding coercion, pipeline creation and export separately. The new palette
  checks preserve observable allocations and failures and cannot simply be
  reordered or removed.
- LAB construction, additional host protocols/state interactions, other sizes
  and modes, composed pipelines, other bindings and platforms remain open.
  Passing these probes is not exhaustive parity or performance proof.

The reusable optimization skill now explains sparse-write ownership costs and
lookup/validation ordering. Static indices contain 15,243 parity cases, 773
workloads and 54 suites. No coverage collection ran. Generated evidence docs
remain explicit about stale/missing broad evidence; focused results are not yet
fully incorporated into the global acceptance matrix. The current-manifest
matrix at `build/migration-parity/optimization-goals-current.json` retains all
209 selected rows (208 operations and one constant), using the historical broad
baseline as diagnostic evidence only. Exports outside that manifest remain
pending. No public operation meets every target. No push or pre-push campaign
is included in this local checkpoint.

## Screen baseline — 2026-09-25

Work moved to `ImageChops.screen` after putpixel checkpoint `7b85bbe66`.
This baseline precedes the Screen implementation attempts below. The maintained
cases plus 23 benchmark workflows pass **573/573 comparisons** on CPU/SIMD/GPU
in `build/migration-parity/perf-screen-20260925-initial-parity.json`.
This is the starting corpus; the finite byte domain, broader mode/shape audit
and fresh sustained-throughput measurement were still pending at this baseline.

All 24 selected standalone/composed workloads completed as
`migration-benchmark-acd42003d2334983b4debe9e71193a90`, retained in
`perf-screen-20260925-initial.json`; the two existing exact benchmark gates pass.
Representative median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.014875 | 0.021208 | 0.017417 | 0.299000 |
| 1 × 1 | 0.012854 | 0.015584 | 0.015916 | 0.198167 |
| 32 × 32 | 0.015229 | 0.016834 | 0.017084 | 0.271438 |
| 256 × 256 | 0.146250 | 0.039083 | 0.042396 | 0.468063 |
| 1024 × 768 | 2.179542 | 0.664625 | 0.560812 | 4.838229 |
| Multiply→Screen, RGB 1024 × 1024 | 6.858959 | 1.131229 | 1.098208 | 4.615875 |
| 260-operation auxiliary chain | 0.857292 | 1.153500 | 1.212583 | 7.815896 |

These rows have completed native execution without fallback. The ordinary
standard row has no terminal native receipt; the resident lifecycle row
reuses materialized results and cannot prove accelerated fresh work. CPU misses
small-input and long-chain targets. Standalone SIMD misses 5× at every measured
size; some composed rows exceed 5× but do not establish the operation-wide
claim. GPU misses SIMD latency throughout these rows. Reciprocal benchmark
latency does not establish sustained throughput.

The SIMD implementation already shares Multiply's packed 16-byte blend helper
and fused Multiply→Screen path. Inspect their callers and allocation/scheduling
costs before duplicating arithmetic. GPU Screen still uses the four-byte
transport shader; the shared native-byte executor used by Multiply is a
candidate once exact mode/shape behavior is verified. No coverage collection
or push ran.

## Screen four-attempt checkpoint — 2026-09-25

Screen remains incomplete and this visit stops after four implementation
attempts. The next operation is `ImageChops.difference`. Retained changes:

1. Execute standalone GPU Screen on native bytes. Four independent samples fit
   each word instead of expanding each logical pixel to four bytes. Preserve
   logical modes, rounded word bounds and the exact final output length.
2. Use the existing CPU cheap-byte scheduling policy: serial work below 4 MiB,
   grouped rows above it, with each clipped source retaining its own stride.
3. Carry native bytes through the existing Multiply→Screen GPU fusion. Keep
   Multiply's intermediate truncation and require the same secondary execution
   source. Receipts report two public operations and one completed dispatch.
4. Stream the fused SIMD kernel across contiguous rows, scheduling 64 KiB tiles
   at 4 MiB instead of individual rows at 256 KiB. Use the actual partial-tile
   length and record the vector work and final scalar tail actually executed.

The preliminary mode audit passed 345 comparisons, including all 65,536 byte
pairs. The generator now retains 132 additional cases: 112 standalone mode,
shape and exhaustive-domain cases, plus 20 fused cases. All existing cases and
benchmark policies are unchanged. Final maintained/workflow parity passes
**975/975 comparisons**, and standalone/fused large-buffer boundary probes pass
**24/24**, including sizes below, at and above the parallel threshold. These
are exact live-Pillow comparisons across CPU/SIMD/GPU; rejected/empty cases
without native receipts do not establish acceleration.

Final parity artifacts under `build/migration-parity/` are
`perf-screen-20260925-final-maintained-parity.json`,
`perf-screen-20260925-final-extra-parity.json`,
`perf-screen-20260925-final-tiles-parity.json` and
`perf-screen-fused-20260925-final-tiles-parity.json`.

All original 24 workloads completed in
`migration-benchmark-91517a38c15d4612a97c428a478e24ad`, retained as
`perf-screen-20260925-final.json`. A content audit found two more Screen-containing
workflows, `pipeline-chain.matrix-006` and `pipeline-chain.matrix-007`, omitted
from the original name-based selection. Their unchanged workloads and exact
parity are recorded separately in `perf-screen-20260925-final-extra.json` and
its parity artifacts. Thus all 26 currently identified Screen-containing
workloads have final diagnostic timings; the two additions have no paired
pre-change measurement. Future inventories must inspect workflow steps too.

Representative final medians in milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015541 | 0.017104 | 0.017688 | 0.182542 |
| 1 × 1 | 0.013376 | 0.015605 | 0.016188 | 0.180625 |
| 32 × 32 | 0.016042 | 0.016396 | 0.017209 | 0.176292 |
| 256 × 256 | 0.146250 | 0.036667 | 0.042605 | 0.698187 |
| 1024 × 768 | 2.208166 | 0.555375 | 0.600271 | 1.860625 |
| Multiply→Screen, L 1024 × 1024 | 1.389937 | 0.300917 | 0.320667 | 0.897958 |
| Multiply→Screen, RGB 1024 × 1024 | 6.948604 | 0.991292 | 1.069729 | 2.221833 |
| Multiply→Screen, RGBA 1024 × 1024 | 7.183084 | 1.665042 | 1.394167 | 3.185604 |
| 260-operation auxiliary chain | 0.882542 | 1.096604 | 1.144001 | 8.185291 |

The ordinary standard row still has no terminal native receipt; the resident
row reuses materialized results. Neither proves accelerated fresh execution.
The two existing exact benchmark gates pass. All workloads retain their original
policies; workflow-level output parity is recorded separately above.

Across the visit, standalone 1024 × 768 CPU latency changed from 0.664625 to
0.555375 ms and GPU from 4.838229 to 1.860625 ms. Fused L/RGB GPU latency changed
from 5.474208/4.615875 to 0.897958/2.221833 ms. SIMD fusion gains are modest:
L/RGB/RGBA 1024² medians changed from 0.338229/1.099958/1.481041 immediately
before attempt four to 0.320667/1.069729/1.394167 afterward. These individual
runs vary; they do not establish stable guarantees. Some small GPU medians
regressed, including 256² from 0.468063 to 0.698187 ms. This is retained as an
open performance gap, not hidden by the larger-image improvements.

The fresh-output diagnostic completed **40,320 exact checks**, including
**38,400 measured completions**, with unchanged source hashes and consistent
runtime binaries. `perf-screen-20260925-final-throughput.json` includes two
fresh input constructions, execution, synchronization, complete export, worker
scheduling and receipt capture. Queue-one median request milliseconds:

| Mode | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.821021 | 0.119063 | 0.153355 | 0.454729 |
| RGB | 3.713250 | 0.534521 | 0.575917 | 1.102271 |

Completed fresh images per second, measured from window wall time:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1187.3 | 7470.3 | 5148.3 | 1705.1 |
| L | 2 | 1238.5 | 9546.1 | 8484.8 | 2227.3 |
| L | 4 | 1234.1 | 10897.0 | 10384.8 | 3108.1 |
| RGB | 1 | 264.2 | 1723.9 | 1598.9 | 642.1 |
| RGB | 2 | 289.7 | 2491.2 | 2456.1 | 916.5 |
| RGB | 4 | 301.2 | 2645.8 | 2684.5 | 1085.0 |

Fresh GPU request medians fell from 3.251584/3.418896 ms for L/RGB to
0.454729/1.102271 ms. Each L upload, auxiliary upload and readback now moves
786,432 bytes, versus 3,145,728 previously; RGB moves 2,359,296 bytes per buffer.
Parameter bytes fell from 256 to 16 and transport mode conversion from one to
zero. CPU meets Pillow latency and SIMD exceeds 5× on these two fresh samples,
but that does not replace the failed small/standard-boundary results. GPU
still misses SIMD latency and throughput at every measured queue depth.

Remaining blockers and next-visit decisions:

- Small-call CPU overhead and standalone SIMD's 5× target remain unresolved.
  Profile construction, binding, allocation and export separately from the
  already packed byte arithmetic. CPU can beat the explicit SIMD route on these
  fresh inputs; inspect generated loops and handoffs before adding instructions.
- GPU launch, mapping, readback and host creation remain after eliminating
  expansion. Measure those phases and small-input variability; another arithmetic
  rewrite cannot remove their fixed cost. Required full host output remains timed.
- Distinct-secondary, longer mixed pipelines and the 260-operation chain retain
  the general transport path. Remove repeated intermediate traffic only while
  preserving secondary identities, observable quantization and completion.
- Additional modes/state interactions, sizes, composed pipelines, bindings,
  hardware and platforms remain unproven. No operation-wide completion is claimed.

The optimization skill now explains retuning task granularity after fusion and
checking whether composed dispatch bypasses compact primitive transport. Static
indices contain 15,375 parity cases, 773 workloads and 54 suites. Generated
specification/evidence docs were refreshed from existing artifacts; no coverage
collection ran. The complete selected-operation matrix still contains 208
operations plus one constant and reports zero completed operations. Integrating
all focused receipts and auditing exports outside that manifest remain pending.
This is a local checkpoint; pre-push verification and a push were not run.

## Difference baseline — 2026-09-25

Work moved to `ImageChops.difference` after Screen checkpoint `054218947`.
No Difference implementation attempt has been made yet. The maintained cases
and five workflow inputs pass **180/180 comparisons**; the independent mode,
shape and finite-domain probe passes **345/345**, including all 65,536 byte
pairs. Artifacts are `perf-difference-20260925-initial-parity.json` and
`perf-difference-20260925-initial-modes-parity.json` under
`build/migration-parity/`. Retaining the new probe inputs in the generator and
collecting fresh sustained-throughput evidence are the next preparation steps.

Selection inspected workload inputs as well as names. All six identified
workloads completed in `migration-benchmark-cbf8911a083b4281ada18333b54e62f6`,
retained in `perf-difference-20260925-initial.json`; its existing exact parity
gate passes. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015625 | 0.017354 | 0.019750 | 0.780104 |
| 32 × 24 | 0.015667 | 0.016292 | 0.016917 | 0.324250 |
| Mixed pipeline `matrix-020` | 0.055813 | 0.041688 | 0.044875 | 1.124417 |
| Mixed pipeline `matrix-032` | 0.021001 | 0.023730 | 0.024938 | 0.333688 |
| SIMD Chops RGB chain | 4.087459 | 0.939063 | 0.735438 | 4.539250 |

These five rows have completed requested-backend receipts without fallback.
The sixth, ordinary standard row, has no terminal native receipt and is not
acceleration evidence. CPU misses small-operation targets; SIMD exceeds 5× only
on the measured larger RGB chain; GPU trails SIMD throughout. The corpus does
not yet establish standalone size scaling or sustained throughput.

Inspect the existing native SIMD Difference helper and its callers before
adding arithmetic. GPU still uses the four-byte-per-pixel Difference transport;
Screen's shared native-byte route is a candidate after preserving Difference's
mode guards and exact partial-word behavior. CPU still uses the general row
scheduling policy. Start from measured transfers, allocation and scheduling
costs, and cap this visit at four attempts. No coverage collection ran.

## Difference four-attempt checkpoint — 2026-09-25

Difference remains incomplete. This visit ends after four attempts, including
two rejected experiments; the next operation is `ImageChops.darker`.

1. **Retained:** native-byte GPU Difference through the existing shared executor.
   Read, compute and return every stored byte without expanding each logical
   pixel to four bytes. Mode admission, paired dimensions and final-word guards
   remain explicit; no arithmetic or fallback contract changed.
2. **Retained:** CPU cheap-byte scheduling, serial below 4 MiB and grouped rows
   above it. Use exact `u8::abs_diff`, retaining independently clipped source
   strides and partial final groups.
3. **Rejected:** construct SIMD output through a pre-reserved vector of 16-byte
   arrays, iterator extension and flattening instead of zero-fill plus stores.
   The isolated loop was 2.6–4.0× slower on 2,304–4,196,352-byte inputs. Its
   out-of-line iterator fold retained chunk-state loads/stores and repeated
   block-size checks. The deployed loop already emits `uabd.16b`; substituting
   an intrinsic would not remove arithmetic instructions. No SIMD implementation
   change from this experiment was retained.
4. **Rejected:** compact the native-word grid into fuller 16×16 GPU workgroups.
   This reduced a 32 × 24 RGB request from 36 groups to three and passed exact
   parity, but public timing did not establish a dependable improvement.
   Its 32 × 24 median changed from 0.207229 to 0.536854 ms; the retained-layout
   rerun was also 0.533583 ms. Variability prevents attributing that slowdown to
   geometry. Fewer dispatched lanes alone was insufficient evidence to retain
   the change. The original dispatch layout is restored.

The rejected allocation source, timings and assembly are retained locally as
`perf-difference-20260925-allocation-*` and
`perf-difference-20260925-native-asm.txt` under `build/migration-parity/`.
Rejected GPU-layout results use the `perf-difference-20260925-attempt4` prefix;
they are not the final retained binary's results.

The generator retains 118 new Difference cases, including all 65,536 byte pairs,
mode/shape/tail cases and six word/workgroup-boundary inputs. Four additional
size workloads use the existing matrix policy. All pre-existing cases and
workloads are unchanged. The ten-workload pre-change baseline is
`migration-benchmark-1ca949aed659484a8670c972da8e4b71`, retained as
`perf-difference-20260925-initial-expanded.json`.

On the final retained build, exact parity passes **546/546 Difference
comparisons**, **12/12 large-buffer threshold comparisons**, and **1,245/1,245
GPU comparisons** for the shared executor's other maintained operations.
Artifacts are `perf-difference-20260925-final-parity.json`,
`perf-difference-20260925-final-tiles-parity.json` and
`perf-native-byte-layout-20260925-retained-parity.json`. The earlier CPU/SIMD/GPU
shared-executor cohort also passed 3,735 comparisons both before and during
the rejected layout trial. Rejected/error/empty cases lacking terminal native
receipts establish behavior only, not acceleration.

All ten retained-code workloads completed in
`migration-benchmark-2d2e63c68bd745788828d4c7faf1d6bc`, stored as
`perf-difference-20260925-final.json`. The existing exact benchmark gate passes;
workflow output comparisons are included in the separate parity cohort above.
Final median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015355 | 0.016771 | 0.016854 | 0.526188 |
| 32 × 24 | 0.015542 | 0.015250 | 0.015541 | 0.533583 |
| 1 × 1 | 0.012792 | 0.014604 | 0.015041 | 0.292854 |
| 32 × 32 | 0.015750 | 0.015229 | 0.016938 | 0.485958 |
| 256 × 256 | 0.186416 | 0.032542 | 0.030770 | 0.437979 |
| 1024 × 768 | 2.420937 | 0.437709 | 0.380459 | 1.848542 |
| Mixed pipeline `matrix-020` | 0.051896 | 0.037187 | 0.043708 | 0.902771 |
| Mixed pipeline `matrix-032` | 0.020479 | 0.021313 | 0.022667 | 0.317667 |
| SIMD Chops RGB chain | 3.813479 | 0.593750 | 0.564291 | 2.221105 |

These rows have completed native requested-backend receipts without fallback.
The tenth, ordinary standard row, still has no terminal native receipt. CPU's
1024 × 768 median improved from 0.801292 to 0.437709 ms and GPU's from
3.221687 to 1.848542 ms. Some small GPU results are worse than the initial run;
those gaps and the observed variability remain unresolved. SIMD exceeds 5× on
256², 1024 × 768 and the larger RGB chain, while small and mixed rows miss.

Fresh sustained-throughput evidence is
`perf-difference-20260925-final-throughput.json`: **40,320 exact output checks**,
including **38,400 measured completions**, with unchanged source hashes and
consistent runtime binaries. Each request constructs both inputs, executes,
waits for completion and exports the full result. Window time includes worker
scheduling and receipt capture. Queue-one median milliseconds:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.553167 | 0.100895 | 0.097001 | 0.473354 |
| RGB | 2.870188 | 0.418708 | 0.485291 | 1.258771 |

Completed fresh images per second from window wall time:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1761.2 | 8753.3 | 9306.8 | 2063.8 |
| L | 2 | 1893.0 | 10826.4 | 12106.9 | 3244.8 |
| L | 4 | 1869.8 | 12149.4 | 12526.6 | 4584.0 |
| RGB | 1 | 346.1 | 2270.6 | 2004.7 | 746.9 |
| RGB | 2 | 391.8 | 3011.3 | 2652.4 | 1172.0 |
| RGB | 4 | 414.8 | 3248.4 | 2289.7 | 1322.1 |

Fresh CPU L/RGB medians changed from 0.216417/0.613750 to
0.100895/0.418708 ms; GPU from 3.376729/2.883208 to 0.473354/1.258771 ms.
Each GPU L upload, secondary upload and readback now moves 786,432 bytes rather
than 3,145,728; RGB moves 2,359,296 bytes per buffer. Parameters use 16 bytes
rather than 256, with zero transport mode conversions. SIMD exceeds 5× on these
two fresh samples. GPU still loses to SIMD in latency and throughput at every
measured queue depth; this is not an operation-wide target pass.

Next-visit blockers and decisions:

- Small-call CPU overhead, small/mixed SIMD latency and GPU latency/throughput
  remain unmet. Profile public construction, validation, allocation and export;
  the packed absolute-difference instruction is already present.
- Allocation removal needs code-generation evidence. Fixed-size typed blocks
  or a loop visible to the optimizer may avoid the rejected iterator's dynamic
  state, but that alternative was not attempted within this visit's cap.
- Separate GPU encoding, launch, device execution, mapping/polling and host
  creation before further grid changes. Measure enough paired runs to resolve
  the small-input variability; arithmetic and lane counts do not identify the
  dominant completion cost.
- Mixed pipelines still use general transport. Any compact composed route must
  retain every operation's logical mode, intermediate semantics and secondary
  operand. Additional sizes, state interactions, bindings and platforms remain
  unproven.

The skill now explains instruction recognition, iterator state and useful GPU
lanes, including rejecting work reductions without measured gains.
Static indices contain 15,493 parity cases, 777 workloads and 54 suites.
Generated docs were refreshed using existing artifacts; no coverage collection
ran. The selected-operation matrix retains 208 operations plus one constant,
with zero fully completed operations. Broad evidence remains historical;
focused-result integration and exports outside the manifest remain pending.
No push or pre-push campaign is included in this checkpoint.

## Darker baseline — 2026-09-25

Work moved to `ImageChops.darker` after Difference checkpoint `b4074be28`.
No Darker implementation attempt has been made yet. Maintained cases plus
seven workflow inputs pass **171/171 comparisons**. The independent 19-mode
shape audit and all 65,536 byte pairs pass **345/345**. Artifacts under
`build/migration-parity/` are `perf-darker-20260925-initial-parity.json` and
`perf-darker-20260925-initial-modes-parity.json`. Retaining those additional
inputs and measuring fresh sustained throughput remain the next preparation.

All eight workloads selected by names and workflow contents completed in
`migration-benchmark-03e2e8d07b9b43c389084e317db7421c`, retained as
`perf-darker-20260925-initial.json`; its existing exact benchmark gate passes.
Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.014313 | 0.016459 | 0.017334 | 0.598313 |
| 32 × 24 | 0.013917 | 0.015396 | 0.016063 | 0.297855 |
| 1 × 1 | 0.012125 | 0.014521 | 0.015042 | 0.543417 |
| 32 × 32 | 0.014104 | 0.015083 | 0.015501 | 0.327562 |
| 256 × 256 | 0.122646 | 0.030813 | 0.030604 | 0.768625 |
| 1024 × 768 | 1.708708 | 0.688480 | 0.371437 | 3.337708 |
| SIMD Chops RGB chain | 2.688646 | 0.694458 | 0.556709 | 4.058229 |

These seven rows have completed requested-backend receipts without fallback.
The ordinary standard row has no terminal native receipt. CPU misses small
inputs, SIMD misses 5× on every measured row, and GPU trails SIMD throughout.
The current SIMD helper already performs packed byte minima and shares the
secondary operand. Inspect output construction and public overhead before
changing arithmetic. CPU still uses the general row scheduling policy, and GPU
still expands native pixels to four-byte transport. Reuse the measured
Difference findings, preserving Darker's mode/shape semantics. Do not repeat
the rejected iterator or dispatch-layout experiments without a new hypothesis.
No coverage collection or push ran.

## Darker checkpoint — 2026-09-25

This visit stops after three implementation attempts under the requested cap.
CPU scheduling and GPU native transport are retained. The SIMD allocation
candidate is rejected; no full-operation target pass is claimed.

1. **Retain CPU scheduling for cheap bytes.** `op_chops_darker` now uses the
   existing serial-under-4-MiB policy and grouped rows above that threshold.
   Byte minima, source strides, clipped shapes and error behavior are unchanged.
2. **Retain native GPU transport.** Darker reuses the validated byte layout and
   separate second-input upload. The shader handles four independent bytes per
   word and guards the valid final word. Logical mode admission stays in the
   existing preflight. This removes expansion and the result conversion, while
   preserving every active channel. Dispatch geometry is unchanged.
3. **Reject typed-block SIMD output collection.** Fixed `[u8; 16]` slices and an
   exact-size mapped iterator initialize vector blocks directly, flatten without
   copying and reserve the padded tail. Unlike Difference's rejected dynamic
   chunk iterator, assembly shows six-vector unrolling with paired loads/stores
   and packed minima. The isolated allocating loop improves about 8–18% on
   several larger sizes, with a small regression at exactly 4 MiB. Public
   evidence does not justify retention: fresh L queue-one latency improves from
   0.094251 to 0.084020 ms, but RGB worsens from 0.362583 to 0.391729 ms and RGB
   throughput falls at all measured queue depths. The change affects a shared
   helper; retain its patch and investigation while public performance remains unproven.

Candidate evidence is `perf-darker-20260925-attempts1-3*.json` under
`build/migration-parity/`, benchmark run
`migration-benchmark-9e157cb2ce81409ea471212750e9e11a`.
The allocation probe, assembly excerpt and rejected patch have suffixes
`allocation-probe.rs`, `allocation-probe.log`, `allocation-asm.txt` and
`attempt3-simd.patch` under the same operation/date prefix.
Candidate parity passed **1,194 comparisons**: 507 Darker, 12 large threshold/tail
cases and 675 shared SIMD cases. Fresh candidates passed **40,320 output checks**
with unchanged sources and consistent runtimes. These are candidate results;
retained-code evidence follows separately.

Added **112 maintained Darker cases** for modes, clipped/empty shapes, vector
boundaries and all 65,536 byte pairs. Existing cases are unchanged: zero removed
or modified. The expanded pre-change Darker baseline passed 507 comparisons;
the pre-change shared SIMD baseline passed 675. The fresh-input throughput
runner now supports Darker with two newly constructed inputs, full output
materialization and native execution receipts. Scheduling, receipt capture and
both image uploads remain accounted for.

The retained build passes **519/519 focused parity comparisons** (507 maintained/workflow and 12 large scheduling/tail comparisons). All eight benchmark workloads complete in `migration-benchmark-e2bef7620a1a4532bed6f7c1b21f8beb`, artifact `perf-darker-20260925-final.json`; the existing exact benchmark gate passes. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015750 | 0.015771 | 0.017417 | 0.514876 |
| 32 × 24 | 0.014833 | 0.015270 | 0.016979 | 0.285146 |
| 1 × 1 | 0.012563 | 0.015771 | 0.016001 | 0.209500 |
| 32 × 32 | 0.015229 | 0.016084 | 0.016313 | 0.434563 |
| 256 × 256 | 0.136728 | 0.031896 | 0.034313 | 0.316125 |
| 1024 × 768 | 1.811813 | 0.427354 | 0.409750 | 1.714438 |
| SIMD Chops RGB workload | 2.917125 | 0.536917 | 0.543563 | 2.147521 |

These seven rows have terminal requested-backend receipts and no fallback. The ordinary standard row lacks terminal native evidence and cannot prove acceleration. The RGB workload named a chain contains one Darker operation, so it is not fusion evidence. Small CPU cases and most SIMD cases still miss their targets. GPU remains slower than SIMD, including the small cases whose timings vary markedly between runs.

Final fresh evidence, `perf-darker-20260925-final-throughput.json`, includes **40,320 exact output checks**, of which **38,400 are measured completions**. Source hashes stay unchanged and runtime binaries agree across processes. Queue-one median milliseconds:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.396666 | 0.088667 | 0.091105 | 0.431104 |
| RGB | 2.211708 | 0.395166 | 0.432605 | 1.052688 |

Completed fresh images per second from window wall time:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 2441.3 | 9527.2 | 9372.9 | 2283.4 |
| L | 2 | 2521.5 | 11739.0 | 11496.1 | 3566.6 |
| L | 4 | 2481.8 | 12612.4 | 12493.9 | 5001.6 |
| RGB | 1 | 445.3 | 2434.0 | 2052.7 | 836.0 |
| RGB | 2 | 507.8 | 2636.1 | 2611.0 | 1199.0 |
| RGB | 4 | 521.3 | 2335.9 | 2275.3 | 1364.1 |

Fresh CPU L/RGB medians improve from 0.205833/0.539250 to 0.088667/0.395166 ms; GPU improves from 2.968126/2.839854 to 0.431104/1.052688 ms (about 6.9×/2.7×). Each native GPU L buffer moves 786,432 bytes and each RGB buffer 2,359,296, compared with 3,145,728 previously. This applies independently to the first upload, second upload and readback. Parameters shrink from 256 to 16 bytes; transport mode conversions drop to zero. GPU still trails SIMD at every measured queue depth. Final fresh SIMD reaches about 4.35× Pillow for L and 5.11× for RGB; these two samples do not establish operation-wide completion.

The restored SIMD path is itself slower on final fresh RGB than the candidate (0.432605 versus 0.391729 ms), despite outperforming the candidate in the maintained large RGB workload (0.543563 versus 0.624938 ms). Therefore the initial apparent fresh RGB regression does not establish a causal regression from typed collection. Public evidence is inconsistent; the candidate remains out because a dependable end-to-end gain is unproven within this visit. No fourth implementation attempt is spent on that uncertainty.

Next-visit blockers and decisions:

- Small-input constructor, validation, object allocation and export costs still dominate the CPU/SIMD target gaps. Packed byte minima already exist; changing arithmetic is not the next attack.
- Typed-block allocation removes loop bookkeeping, but whole-call results vary by workload and run. Profile allocation and surrounding copies, then use paired same-environment comparisons before revisiting it. Do not infer a public win from the microbenchmark or the lower initialized-byte count.
- GPU launch, encoding, wait/mapping and host output creation remain after compact transport. Measure those stages before changing dispatch geometry again. Fresh throughput must still include two image constructions, both uploads and complete output export.
- Additional sizes, mode/state combinations, composed pipelines, bindings and platforms remain unproven. None is silently excluded from the goal.

The optimization skill records the typed-block decision, capacity for the padded tail and the need to reject unproven public gains. Static indices contain **15,605 parity cases, 777 workloads and 54 suites**. Generated documentation was refreshed from existing artifacts; no coverage collection ran. The global selected matrix still contains 208 operations plus one constant, with zero fully completed operations; broad evidence remains historical and focused-result integration remains pending. No push or pre-push verification campaign is part of this checkpoint. Work moves to `ImageChops.lighter`.

## Lighter baseline — 2026-09-25

Work moved to `ImageChops.lighter` after Darker checkpoint `9a1c90f47`. Before implementation changes, 162 original maintained/workflow comparisons and 345 independent mode/shape/exhaustive-byte-pair comparisons pass. Added 112 maintained parity cases and four deterministic size workloads, with zero existing cases/workloads changed or removed. The expanded unchanged-code baseline passes **510/510 comparisons**. Artifacts are `perf-lighter-20260925-initial-parity.json`, `initial-modes-parity.json` and `initial-expanded-parity.json` under `build/migration-parity/` with the same operation/date prefix.

All eight selected workloads complete in `migration-benchmark-6904e6b7b1de461280732bf77cc0962a`, artifact `perf-lighter-20260925-initial.json`. Selection inspects workflow contents as well as names. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.014124 | 0.015917 | 0.016292 | 0.286354 |
| 32 × 24 | 0.014396 | 0.015979 | 0.015875 | 0.540812 |
| 1 × 1 | 0.012854 | 0.014542 | 0.014979 | 0.248437 |
| 32 × 32 | 0.015959 | 0.015937 | 0.016334 | 0.217709 |
| 256 × 256 | 0.136146 | 0.031208 | 0.032250 | 0.718687 |
| 1024 × 768 | 1.921520 | 0.705375 | 0.490687 | 3.415541 |
| SIMD Chops RGB workload | 3.068459 | 0.946604 | 0.526438 | 4.044875 |

These seven rows have terminal native receipts without fallback. The standard row lacks that proof; the RGB workload contains one Lighter operation despite its chain name. Fresh evidence is `perf-lighter-20260925-initial-throughput.json`, with **40,320 exact checks**, **38,400 measured completions**, unchanged sources and consistent runtime binaries. Queue-one median milliseconds:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.396062 | 0.212062 | 0.084687 | 2.965104 |
| RGB | 2.292312 | 0.537875 | 0.387854 | 2.804813 |

CPU uses the general row policy despite inexpensive byte maxima. GPU still expands both inputs and the output transport to four-byte pixels. SIMD already uses packed maxima and the shared bytewise helper; do not repeat Darker’s inconclusive allocation experiment without new evidence. CPU small calls, most SIMD sizes and all GPU latency/throughput comparisons remain unmet. No implementation attempt, coverage collection or push is included in this baseline. Static inventory is now **15,717 parity cases and 781 benchmark workloads across 54 suites**.

## Lighter checkpoint — 2026-09-25

This visit retains two implementation attempts and moves on within the 3–4 attempt ceiling. The existing cheap-byte CPU scheduling policy removes per-row task overhead below 4 MiB and groups rows above it. Native GPU byte transport removes pixel expansion for both inputs and result conversion; four independent bytes share each word, with an explicit final-word bound and the existing logical-mode preflight. Arithmetic, clipping, invalid-input behavior and dispatch geometry are unchanged. SIMD already uses packed maxima; Darker’s inconclusive allocation candidate is not repeated.

**522/522 focused parity comparisons pass**: 510 maintained/workflow and 12 large threshold/tail comparisons. Artifacts are `perf-lighter-20260925-final-parity.json` and `perf-lighter-20260925-final-tiles-parity.json` under `build/migration-parity/`. The isolated release parity build completed and formatting/public-boundary checks passed.

All eight workloads complete in `migration-benchmark-aa3687126a0f4bc49d0444dfcd5b6a80`, artifact `perf-lighter-20260925-final.json`; the exact benchmark gate passes. Seven materialized workloads have completed requested-backend receipts without fallback. The standard row still lacks terminal native evidence. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015812 | 0.016438 | 0.017708 | 0.508792 |
| 32 × 24 | 0.014687 | 0.016584 | 0.017146 | 0.211542 |
| 1 × 1 | 0.012750 | 0.016000 | 0.016167 | 0.207375 |
| 32 × 32 | 0.015125 | 0.016459 | 0.017791 | 0.441709 |
| 256 × 256 | 0.136730 | 0.031145 | 0.033563 | 0.321375 |
| 1024 × 768 | 1.897541 | 0.505417 | 0.412645 | 1.800937 |
| SIMD Chops RGB workload | 2.858084 | 0.589729 | 0.549750 | 2.049542 |

`perf-lighter-20260925-final-throughput.json` records **40,320 exact fresh-output checks**, including **38,400 measured completions**, with unchanged source hashes and consistent runtime binaries. Queue-one median milliseconds:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.405604 | 0.081958 | 0.098417 | 0.432895 |
| RGB | 2.266437 | 0.402375 | 0.384292 | 0.992666 |

Completed fresh images per second from window wall time:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 2404.6 | 10549.9 | 9196.2 | 2273.9 |
| L | 2 | 2388.1 | 11701.6 | 11193.6 | 3500.6 |
| L | 4 | 2547.3 | 12767.8 | 12960.5 | 4934.8 |
| RGB | 1 | 436.9 | 2373.2 | 2470.1 | 950.9 |
| RGB | 2 | 506.7 | 3047.1 | 3136.6 | 1404.9 |
| RGB | 4 | 554.1 | 2678.5 | 2951.6 | 1549.9 |

Fresh CPU L/RGB medians improve from 0.212062/0.537875 to 0.081958/0.402375 ms. GPU improves from 2.965104/2.804813 to 0.432895/0.992666 ms (about 6.8×/2.8×), and throughput improves at every measured queue depth. Each GPU L/RGB upload, secondary upload and readback now uses 786,432/2,359,296 bytes instead of 3,145,728. Parameters use 16 instead of 256 bytes, and transport mode conversions are zero. All timings still include fresh inputs and complete output export.

Remaining blockers:

- CPU misses small public calls. SIMD misses 5× on small/intermediate rows and fresh L (about 4.12×); fresh RGB reaches about 5.90×, which is only one workload. Constructor, validation, allocation and export costs need direct attribution before another arithmetic change.
- GPU still trails SIMD in both latency and sustained throughput at every measured queue depth. Separate command encoding, launch, device work, wait/mapping and host output creation next. Small GPU timings vary and some regress relative to the baseline; compact transport does not solve fixed completion overhead.
- Binding mode lookups can materialize lazy data. Do not remove thread release or change borrowed-versus-snapshotted ownership merely because a normal metadata call looks cheap. No binding behavior was changed in this visit.
- Broader modes, sizes, state interactions, compositions, bindings and platforms remain unproven. The operation is not complete.

These decisions reuse the existing skill’s byte-cost scheduling, compact transport and public-boundary rules. No speculative extra attempt was made to fill the attempt budget. The global selected matrix remains 208 operations plus one constant, with zero fully completed operations; it includes 781 workloads and historical broad timings. Focused-result integration remains pending. No coverage collection or push ran. Work moves to `ImageChops.add_modulo`.

## Add Modulo baseline — 2026-09-25

Work moved to `ImageChops.add_modulo` after Lighter checkpoint `0622ee10e`. No implementation attempt has been made yet. Original maintained/workflow cases pass **189/189 comparisons**, and an independent 19-mode shape audit plus every byte pair passes **345/345**. Added **112 maintained parity cases and four size workloads**, with zero existing cases/workloads modified or removed. The expanded unchanged-code baseline passes **537/537 comparisons**. Artifacts under `build/migration-parity/` use prefix `perf-add-modulo-20260925-` and suffixes `initial-parity.json`, `initial-modes-parity.json` and `initial-expanded-parity.json`.

All eight workloads selected by names and workflow contents complete in `migration-benchmark-e69c90c1ecb44407b1a0498948aea429`, artifact `perf-add-modulo-20260925-initial.json`. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015375 | 0.016354 | 0.016730 | 0.547313 |
| 32 × 24 | 0.015083 | 0.015604 | 0.016105 | 0.231375 |
| 1 × 1 | 0.012938 | 0.014625 | 0.015666 | 0.539208 |
| 32 × 32 | 0.015166 | 0.015626 | 0.015833 | 0.219812 |
| 256 × 256 | 0.135167 | 0.033834 | 0.030583 | 0.602812 |
| 1024 × 768 | 1.757334 | 0.683979 | 0.426584 | 3.290188 |
| SIMD Chops RGB workload | 2.832792 | 0.852125 | 0.573416 | 3.888375 |

These seven rows have completed native requested-backend receipts without fallback. The standard row lacks terminal native proof. The workload named a chain contains one Add Modulo operation; no fusion result is claimed. CPU misses small calls, SIMD misses 5× on every maintained row, and GPU trails SIMD throughout.

Fresh baseline `perf-add-modulo-20260925-initial-throughput.json` contains **40,320 exact checks**, **38,400 measured completions**, unchanged source hashes and consistent runtime binaries. Queue-one median milliseconds:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.401062 | 0.210563 | 0.090083 | 2.856667 |
| RGB | 2.244854 | 0.559729 | 0.401437 | 2.805334 |

CPU still uses general row scheduling. GPU expands each input and the result to four-byte transport. SIMD already uses wrapping packed-byte addition, so ordinary modulo arithmetic is not the first optimization target. Start with the measured scheduling/transport mechanisms while preserving native channel semantics, clipping and tail bytes. Further instruction or allocation changes need new public-path evidence; do not repeat the rejected Difference/Darker experiments.

The input inventory is now **15,829 parity cases and 785 workloads across 54 suites**. Static declarations and generated docs were refreshed; no coverage collection ran. The selected global matrix retains 208 operations plus one constant and zero fully completed operations. Historical broad evidence and missing focused-result integration remain explicit limitations. No push or pre-push verification campaign ran.

## Add Modulo checkpoint — 2026-09-25

This visit retains two implementation attempts and moves on within the 3–4 attempt ceiling. CPU wrapping addition uses the existing cheap-byte scheduling policy: serial below 4 MiB, grouped rows above it. GPU now transports native bytes for both operands and the result, computing four independent samples per word with an explicit final-word guard. Logical-mode admission, byte-wise wrapping, clipping and the general transport fallback remain unchanged. SIMD already uses packed wrapping addition; no SIMD change was made.

**549/549 focused comparisons pass**: 537 maintained/workflow cases and 12 large scheduling/tail comparisons. The release parity build and formatting/public-boundary checks passed. Evidence under `build/migration-parity/` uses prefix `perf-add-modulo-20260925-` with suffixes `attempts1-2-parity.json` and `attempts1-2-tiles-parity.json`. These files describe the retained code.

All eight workloads complete in `migration-benchmark-a5fbd68c0b9b4e40833b9653c3c6f656`, artifact `perf-add-modulo-20260925-attempts1-2.json`; the existing exact benchmark gate passes. Seven materialized workloads have completed native requested-backend receipts without fallback; the ordinary standard row lacks terminal native proof. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015521 | 0.018084 | 0.018417 | 0.519396 |
| 32 × 24 | 0.015166 | 0.017188 | 0.017334 | 0.203604 |
| 1 × 1 | 0.013083 | 0.016792 | 0.016792 | 0.407855 |
| 32 × 32 | 0.015104 | 0.016542 | 0.017146 | 0.397896 |
| 256 × 256 | 0.136146 | 0.030375 | 0.034021 | 0.343145 |
| 1024 × 768 | 1.882583 | 0.393687 | 0.421791 | 1.824021 |
| SIMD Chops RGB workload | 2.858376 | 0.550708 | 0.542937 | 2.163125 |

Fresh evidence `perf-add-modulo-20260925-attempts1-2-throughput.json` contains **40,320 exact output checks**, including **38,400 measured completions**, with unchanged source hashes and consistent runtime binaries. Queue-one median milliseconds:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.409042 | 0.098354 | 0.091063 | 0.461458 |
| RGB | 2.370604 | 0.417854 | 0.385979 | 1.037562 |

Completed fresh images per second from window wall time:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 2362.6 | 8944.3 | 9335.0 | 2118.5 |
| L | 2 | 2449.2 | 11049.0 | 11223.7 | 3362.9 |
| L | 4 | 2376.7 | 12071.3 | 11880.1 | 4694.7 |
| RGB | 1 | 414.5 | 2199.1 | 2410.1 | 896.8 |
| RGB | 2 | 483.0 | 2858.2 | 2946.5 | 1297.3 |
| RGB | 4 | 536.0 | 2644.2 | 2684.4 | 1450.9 |

Fresh CPU L/RGB latency improves from 0.210563/0.559729 to 0.098354/0.417854 ms. GPU improves from 2.856667/2.805334 to 0.461458/1.037562 ms (about 6.2×/2.7×), with higher throughput at every measured queue depth. Each GPU L/RGB primary upload, secondary upload and readback moves 786,432/2,359,296 bytes instead of 3,145,728. Parameters shrink from 256 to 16 bytes and transport mode conversions are zero. Construction, both inputs and full output materialization remain inside the public work boundary.

Remaining blockers and decisions:

- CPU still misses small calls. SIMD misses 5× on small/intermediate maintained rows and fresh L (about 4.49×); fresh RGB reaches about 6.14×. Fixed public call/validation/allocation costs and complete memory traffic need direct attribution before another inner-loop change.
- GPU still loses to SIMD in latency and throughput at every measured queue depth. Some small GPU results regress relative to baseline. Separate encoding, device work, mapping/wait and host output creation; compact bytes do not remove fixed completion overhead.
- A possible future shader formula separates each packed byte into low seven bits and a high bit: `((a & 0x7f7f7f7f) + (b & 0x7f7f7f7f)) ^ ((a ^ b) & 0x80808080)`. Low-bit sums cannot cross byte boundaries, and the XOR restores the high-bit contribution modulo 256. Independent checks of 131,072 words pass (`perf-add-modulo-20260925-packed-word-proof.json`). This is an arithmetic proof only; it was not installed or timed on the GPU. Do not count it as an optimization attempt or a speedup. Use it only if device execution is a meaningful measured cost.
- Broader modes, sizes, state interactions, compositions, bindings and platforms remain unproven. No operation-wide target pass is claimed.

The skill now explains carry isolation and the exact packed-add formula, including the limit of an instruction-count argument. The global selected matrix remains 208 operations plus one constant and zero completed operations. Inventory remains 15,829 parity cases and 785 workloads across 54 suites. Historical broad evidence and missing focused-result integration remain explicit. No coverage collection, push or pre-push verification campaign ran. Work moves to `ImageChops.subtract_modulo`.

## Subtract Modulo baseline — 2026-09-25

Work moved to `ImageChops.subtract_modulo` after Add Modulo checkpoint `ce5f0db28`. No implementation attempt has been made yet. The original maintained/workflow comparison passes **186/186**, and the independent 19-mode shape/byte-pair audit passes **345/345**. Added **112 maintained cases and four size workloads**, with no existing inputs modified or removed. The expanded unchanged-code baseline passes **534/534 comparisons**. Large scheduling boundaries, partial tiles and both directions of unequal source widths pass **18/18**. Artifacts use `perf-subtract-modulo-20260925-` under `build/migration-parity/`, with suffixes `initial-parity.json`, `initial-modes-parity.json`, `initial-expanded-parity.json`, `initial-tiles-parity.json` and the retained `tiles-probe.py`.

All eight workloads selected by names and workflow contents complete in `migration-benchmark-e07afaff7b83411da0548da5028aa1c7`, artifact `perf-subtract-modulo-20260925-initial.json`. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015479 | 0.017479 | 0.018021 | 0.524271 |
| 32 × 24 | 0.015000 | 0.016479 | 0.018020 | 0.222625 |
| 1 × 1 | 0.014292 | 0.016188 | 0.016375 | 0.258999 |
| 32 × 32 | 0.015249 | 0.016667 | 0.017105 | 0.464021 |
| 256 × 256 | 0.134937 | 0.032583 | 0.032438 | 0.472521 |
| 1024 × 768 | 1.977896 | 0.879916 | 0.478021 | 3.507167 |
| SIMD Chops RGB workload | 2.918646 | 0.776645 | 0.576645 | 4.160250 |

These seven rows have terminal native requested-backend receipts without fallback. The standard row lacks terminal native proof. The RGB workload contains one operation despite its chain name. CPU small cases miss Pillow, SIMD misses 5× on most rows, and GPU trails SIMD throughout.

Fresh baseline `perf-subtract-modulo-20260925-initial-throughput.json` includes **40,320 exact output checks**, **38,400 measured completions**, unchanged source hashes and consistent runtime binaries. Queue-one median milliseconds:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.378250 | 0.212374 | 0.084041 | 2.891187 |
| RGB | 2.184792 | 0.537791 | 0.404313 | 2.796771 |

CPU uses general row scheduling and GPU expands transport to four bytes per pixel. SIMD already uses wrapping packed subtraction. Reuse the validated cheap-byte scheduling and compact transport mechanisms first, preserving independent source strides and every active byte. No coverage collection or push ran. Inventory is now **15,941 parity cases and 789 workloads across 54 suites**. The selected matrix still has 208 operations plus one constant, with zero fully completed operations; broad evidence is historical and focused-result integration remains pending.

## Subtract Modulo checkpoint — 2026-09-25

This visit retains two implementation attempts and moves on within the 3–4 attempt ceiling. CPU wrapping subtraction now uses the existing cheap-byte policy: serial below 4 MiB and grouped rows above it. GPU reuses native-byte transport for both inputs and the result, with independent per-byte subtraction and an explicit final-word bound. Mode admission, clipping, errors and arithmetic are unchanged. SIMD already uses packed subtraction and was not modified.

**552/552 focused comparisons pass**: 534 maintained/workflow comparisons and 18 large boundary, partial-tile and unequal-source-width comparisons. The release parity build and formatting/public-boundary checks passed. Artifacts under `build/migration-parity/` use prefix `perf-subtract-modulo-20260925-` with suffixes `final-parity.json` and `final-tiles-parity.json`.

All eight workloads complete in `migration-benchmark-1e0282f4affb492abc6cf3a5350d1946`, artifact `perf-subtract-modulo-20260925-final.json`; the exact benchmark gate passes. Seven materialized workloads have terminal requested-backend receipts without fallback. The ordinary standard row lacks terminal native evidence. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015083 | 0.016813 | 0.018812 | 0.289771 |
| 32 × 24 | 0.014792 | 0.015562 | 0.017708 | 0.306812 |
| 1 × 1 | 0.012875 | 0.014937 | 0.016771 | 0.444250 |
| 32 × 32 | 0.015312 | 0.015417 | 0.017063 | 0.248729 |
| 256 × 256 | 0.137792 | 0.032417 | 0.033833 | 0.662999 |
| 1024 × 768 | 1.856604 | 0.440250 | 0.498437 | 2.253312 |
| SIMD Chops RGB workload | 2.969646 | 0.608563 | 0.643750 | 2.245667 |

`perf-subtract-modulo-20260925-final-throughput.json` records **40,320 exact fresh-output checks**, including **38,400 measured completions**, unchanged source hashes and consistent runtime binaries. Queue-one median milliseconds:

| Mode, 1024 × 768 | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.429042 | 0.101334 | 0.101292 | 0.468104 |
| RGB | 2.425687 | 0.437646 | 0.447250 | 1.154270 |

Completed fresh images per second from window wall time:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 2235.9 | 8646.2 | 8616.1 | 1660.4 |
| L | 2 | 2372.1 | 10614.0 | 10443.3 | 2248.6 |
| L | 4 | 2351.5 | 11199.9 | 10205.5 | 3380.2 |
| RGB | 1 | 401.8 | 2085.3 | 2058.0 | 646.1 |
| RGB | 2 | 480.0 | 2689.6 | 2718.2 | 1027.9 |
| RGB | 4 | 516.2 | 2523.7 | 2601.2 | 1276.1 |

Fresh CPU L/RGB medians improve from 0.212374/0.537791 to 0.101334/0.437646 ms. GPU improves from 2.891187/2.796771 to 0.468104/1.154270 ms (about 6.2×/2.4×), with higher throughput at every measured queue depth. GPU L/RGB primary upload, auxiliary upload and readback each use 786,432/2,359,296 bytes instead of 3,145,728. Parameter bytes fall from 256 to 16 and transport mode conversions to zero. Both fresh inputs and complete materialization remain measured; throughput comes from completed window work, not reciprocal median latency.

Remaining blockers:

- CPU still misses small public inputs. SIMD misses 5× on the maintained rows and fresh L (about 4.24×); fresh RGB reaches about 5.42×. Public construction, validation, allocation and export need attribution before further arithmetic tuning.
- GPU still trails SIMD in latency and throughput at every measured queue depth. Some small and intermediate GPU timings regress relative to baseline. Map/poll/sleep, encoding and host output creation remain unresolved; reduce those costs only with paired completed-work evidence.
- Scheduling changes must retain both original source row widths when clipping the output. The added large unequal-width tests cover each operand being wider across the new grouped-row policy. Existing stride logic required no change.
- More modes, sizes, state interactions, composed pipelines, bindings and platforms remain unproven. No operation-wide target pass is claimed.

The existing skill’s byte-cost scheduling, independent strides and compact-transport rules cover the retained changes. Inventory remains 15,941 parity cases and 789 workloads across 54 suites. The selected global matrix has 208 operations plus one constant and zero fully completed operations; broad evidence remains historical and focused-result integration remains pending. No coverage collection or push ran. Work moves to `ImageChops.logical_and`.

## Logical Chops parity repair — 2026-09-25

Work moved to `ImageChops.logical_and` after Subtract Modulo checkpoint
`3e26475a1`. Its 13 original maintained cases and two workflow inputs passed
45 comparisons, but a 19-mode shape audit exposed a real implementation bug:
Pillow accepts unequal mode-1 dimensions and clips to their overlap; the Rust
public validator returned `ValueError("images do not match")`. This affected
CPU, SIMD and GPU before dispatch. No performance change was attempted first.

A shared-validator audit confirmed the same behavior for AND, OR and XOR,
including reversed widths, crossed width/height bounds and empty overlaps in
either operand. The expanded unchanged-code comparison had **63 failures out
of 1,203 comparisons** (21 per operation across three requested backends).
The implementation now retains the mode-1 check and removes only the incorrect
equal-size restriction. Existing execution paths already perform the clipping.
Rust API documentation now describes the actual mode errors and clipped shape.

Added **363 maintained cases** across the three affected operations, with zero
existing cases modified or removed. These include 19 modes and six shapes,
reversed/crossed clipping, unequal empty inputs and every packed-byte pair in
mode-1 images. The pair inputs are decoded as binary pixels; no byte-oriented
semantics are substituted for Logical Chops.

The rebuilt implementation passes **1,203/1,203 shared comparisons**, plus
**18/18 large Logical AND comparisons** covering scheduling boundaries,
partial packed rows and both directions of unequal source widths. Evidence in
`build/migration-parity/`:

- `perf-logical-and-20260925-initial-parity.json`: original maintained baseline.
- `perf-logical-and-20260925-initial-modes-parity.json`: first clipped-shape failure.
- `perf-logical-shared-20260925-before-parity.json`: shared shape reproduction.
- `perf-logical-all-20260925-before-parity.json`: expanded failing baseline.
- `perf-logical-all-20260925-fixed-parity.json`: all shared cases pass.
- `perf-logical-and-20260925-fixed-tiles-parity.json`: large boundaries pass.

This is parity evidence, not proof of native acceleration for every clipped
input. Backend admission and exact fallback remain separate from public input
validity. The skill now states that distinction and separates output extent
from each input stride. No assertions, thresholds or expected results were
weakened. The isolated release build and formatting/public-boundary checks
passed; generated docs were refreshed from existing evidence, without coverage
collection. No push or pre-push campaign ran.

Logical AND remains the current operation. This parity repair is its first
implementation attempt; next collect size-varied and fresh mode-1 performance
evidence before optimizing scheduling, transport or bit packing. Inventory is
**16,304 parity cases and 789 workloads across 54 suites**. The selected global
matrix still has 208 operations plus one constant and zero fully completed
operations; broad evidence is historical and focused-result integration remains
pending.

## Logical AND performance baseline — 2026-09-25

The shared clipping parity repair is committed as `35fa60a64` and counts as attempt one. Four additional mode-1 size workloads were added without changing existing workloads; all **420/420** selected maintained/workflow comparisons pass after the repair. The throughput runner now accepts Logical AND with packed MSB-first mode-1 input rows, including padding at each row boundary. An odd-width 1031 × 769 check passes **192/192 outputs** with native execution receipts. Other operation/mode contracts are unchanged.

All seven selected workloads complete in `migration-benchmark-ea0ca1b388a44ee8adbe11d0c351a40a`, artifact `perf-logical-and-20260925-baseline.json`. Six materialized workloads have terminal native requested-backend receipts without fallback. The standard row lacks terminal native evidence. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.014604 | 0.015562 | 0.018188 | 0.422188 |
| 1 × 1 | 0.013042 | 0.014458 | 0.017000 | 0.267437 |
| 32 × 32 | 0.013396 | 0.014999 | 0.017500 | 0.265397 |
| 256 × 256 | 0.052979 | 0.049125 | 0.053979 | 0.425792 |
| 1024 × 768 | 0.489355 | 0.692396 | 0.493479 | 4.325875 |
| SIMD Chops mode-1 workload | 0.764187 | 0.885667 | 0.675437 | 5.420647 |

Fresh evidence `perf-logical-and-20260925-baseline-throughput.json` records **20,160 exact checks**, including **19,200 measured completions**, with unchanged source hashes and consistent runtime binaries. Queue-one mode-1 medians at 1024 × 768 are Pillow **1.253333 ms**, CPU **1.407895 ms**, SIMD **1.275750 ms** and GPU **4.361625 ms**. Completed fresh images per second:

| Queue depth | Pillow | CPU | SIMD | GPU |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 762.7 | 691.9 | 768.4 | 212.8 |
| 2 | 956.8 | 1343.0 | 1482.8 | 344.6 |
| 4 | 1057.0 | 2482.8 | 2647.9 | 564.6 |

A separate phase diagnostic retained as `perf-logical-and-20260925-phases.py` and `perf-logical-and-20260925-baseline-phases.json` uses eight depth-one verification windows, discards the first two for stage medians and checks all outputs. It is not a performance gate. SIMD input construction takes **404.000 + 419.584 µs**, while lazy execution plus export takes **448.125 µs**. CPU input construction takes **427.812 + 438.396 µs**, execution/export **569.979 µs**. GPU input construction takes **395.938 + 411.312 µs**, execution/export **3,412.021 µs**. Pillow performs the logical kernel during its operation call; Rust defers it to export, so these individual call phases are not like-for-like kernel comparisons. The full public timings remain the primary evidence.

The source explains the next attacks: default mode-1 `frombytes` extracts and stores one bit at a time; `tobytes` clones the grayscale image and then repeatedly updates output bytes per pixel. Attempts two and three will target byte-block unpacking and borrowed block packing. Reserve the fourth attempt for GPU transport, which currently moves 3,145,728 bytes for each input and readback despite the packed public input being 98,304 bytes. CPU/SIMD tiny calls and every GPU target remain unresolved.

No coverage collection or push ran. Inventory is **16,304 parity cases and 793 workloads across 54 suites**. The selected global matrix retains 208 operations plus one constant and zero fully completed operations; broad timings remain historical and focused-result integration remains pending.

## Logical AND checkpoint — 2026-09-25

Four implementation attempts are complete, counting the earlier shared clipping repair. This visit stops here under the attempt cap and moves to Logical OR; no operation-wide pass is claimed.

1. **Parity first:** the shared unequal-size validation repair was retained in `35fa60a64`.
2. **Packed input decoding:** replace eight per-pixel bit extractions with one 2 KiB lookup table entry and an eight-byte store. Retain MSB-first ordering and independent row padding.
3. **Packed output encoding:** borrow the existing luma bytes, removing the full image clone. Test eight stored bytes for nonzero with carry-isolated word arithmetic, then gather the flags into one output byte. Preserve all nonzero sample values, not only 255; handle partial rows separately. Both I/O changes are retained.
4. **GPU native-byte transport:** reuse the existing native-byte executor for a single equal-size mode-1 Logical AND, eliminating expanded RGBA transport. While checking its semantic admission, a stronger raw-sample audit found another parity defect. The fourth attempt includes the required correction on CPU, SIMD (including in-place and scalar helper paths), and both GPU layouts: Pillow tests each stored sample for truth and produces canonical 0/255 output. SIMD uses an unsigned minimum followed by a zero comparison; GPU uses carry-isolated per-byte truth flags. No fifth performance experiment was attempted.

Mode-1 public `putdata`/`putpixel` can retain 1 and 2 as stored samples. Pillow Logical AND returns 255 for that pair, whereas the former bitwise implementation returned zero. Packed `frombytes` fixtures cannot expose this because they decode only 0/255 pixels; packed output alone also conceals noncanonical nonzero results. The first 13 new cases reproduce **33 failures in 39 comparisons**, retained in `perf-logical-and-stored-20260925-before-parity.json`. Fifteen maintained cases now cover every stored-byte pair, raw output and unchanged input observations, vector/word/grid tails, clipped strides, empty inputs, materialized inputs and composed execution. **Zero existing cases were modified or removed.**

The final shared bit-I/O audit passes **1,839/1,842 comparisons**, including **45/45 new stored-sample comparisons**. Its only failures are exactly the same three pre-existing `1 → LAB` conversion failures recorded before these changes and at the earlier conversion checkpoint. They remain visible in `perf-mode1-io-20260925-final-parity.json`; this is not an all-green shared audit. The new materialized exhaustive case has a native GPU receipt with 65,536-byte inputs/readback, no fallback and canonical output. All **18/18 large Logical AND boundary comparisons** pass in `perf-logical-and-20260925-final-tiles-parity.json`. The focused Rust bit-I/O tests also pass: all 256 input bytes, 10,240 nonzero packing vectors and widths 0–33 with multiple row counts.

The final unchanged seven-workload benchmark completes as `migration-benchmark-8e427dbc6ace48b4b166a2440032192d`, artifact `perf-logical-and-20260925-final.json`. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.016521 | 0.017417 | 0.019958 | 0.302458 |
| 1 × 1 | 0.012708 | 0.015625 | 0.016855 | 0.536208 |
| 32 × 32 | 0.013375 | 0.015771 | 0.016396 | 0.224500 |
| 256 × 256 | 0.053334 | 0.025729 | 0.030291 | 0.295083 |
| 1024 × 768 | 0.484083 | 0.338000 | 0.139229 | 0.858125 |
| SIMD Chops mode-1 workload | 0.764334 | 0.363792 | 0.234979 | 0.911459 |

Six materialized rows retain completed native requested-backend evidence without fallback. The standard row remains a terminal-evidence gap. Fresh 1024 × 768 evidence in `perf-logical-and-20260925-final-throughput.json` passes **20,160 exact checks**, including **19,200 measured completions**, with unchanged source hashes and consistent runtime binaries. Queue-one median milliseconds are Pillow **1.234104**, CPU **0.286459**, SIMD **0.124667**, GPU **0.526583**. Completed fresh images per second:

| Queue depth | Pillow | CPU | SIMD | GPU |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 797.0 | 3328.7 | 6976.2 | 1759.5 |
| 2 | 975.6 | 6445.2 | 11583.8 | 3074.1 |
| 4 | 1050.6 | 9353.2 | 16828.5 | 4768.5 |

CPU/SIMD/GPU fresh medians improve from **1.407895/1.275750/4.361625 ms** to **0.286459/0.124667/0.526583 ms**. SIMD is about **9.9×** faster than Pillow for this fresh workload; that does not satisfy the smaller maintained workload requirements. GPU improves about **8.3×** against its baseline and increases throughput at every tested depth, but remains slower than SIMD. Each GPU input and readback now transfers **786,432 bytes**, down from 3,145,728; parameter bytes fall from 256 to 16 and mode conversions to zero. Stored samples remain byte-oriented; the packed public image is 98,304 bytes. Both imports, complete export, synchronization and allocation remain included. The odd-width 1031 × 769 fresh check adds **192/192 passing outputs** in `perf-logical-and-20260925-final-packed-check.json`.

Intermediate evidence `perf-logical-and-20260925-attempts2-3*.json` isolates the retained I/O changes. Its SIMD phase diagnostic reduces the two imports from **404.000 + 419.584 µs** to **23.792 + 24.500 µs**, and lazy execution/export from **448.125 µs** to **69.271 µs**. The final diagnostic (`perf-logical-and-20260925-final-phases.json`, 512 exact outputs) records SIMD imports **24.562 + 25.667 µs**, execution/export **81.416 µs**, and GPU execution/export **465.750 µs** versus the initial 3,412.021 µs. These are diagnostic call phases, not isolated kernel comparisons: Pillow is eager and Rust is lazy.

Remaining blockers and next decisions:

- CPU still misses the smallest public calls. At larger sizes, CPU execution/export is about 184.687 µs in the final diagnostic; its old fine-row scheduling remains unchanged in this visit. Attribute fixed wrapper/allocation costs and use the existing measured cheap-byte scheduling policy when revisiting.
- SIMD misses 5× on the maintained rows despite the fresh 1024 × 768 win. Further work must reduce caller/materialization overhead and memory passes, with separate tiny-input evidence.
- GPU still misses SIMD latency and completed throughput at all measured queue depths. The byte path removes expansion but retains launch, mapping, output creation and eight stored bits per logical pixel. A packed-bit/resident route must count packing/unpacking and preserve canonical result samples and composed behavior. Host wait phases require direct attribution before changing completion policy. Small GPU timings also vary or regress; no universal improvement is claimed.
- The stronger truth audit must be applied to Logical OR and XOR on their turns. Existing binary-decoded fixtures alone do not prove their stored-sample contract.
- The three known LAB conversion failures remain at the conversion checkpoint. More bindings, modes, sizes, composed pipelines and platforms remain unproven.

The reusable optimization skill now records byte-domain lookup expansion, carry-isolated bit packing, borrowing before materialization, row padding, eager/lazy attribution and mutation-created semantic states. The isolated release build and formatting/public-boundary checks passed. Generated documentation was refreshed from existing evidence; input generation updated static coverage declarations only. **No coverage collection or push ran.** Inventory is **16,319 parity cases and 793 workloads across 54 suites**. The selected global matrix still has 208 operations plus one constant and zero fully completed operations; broad timings remain historical and focused-result integration remains pending.

## Logical OR checkpoint — 2026-09-25

Logical AND checkpoint `24e1e4b5f` is complete. Logical OR receives **three attempts**, all retained; work now moves to Logical XOR under the attempt cap. The inherited packed-bit I/O improvements are part of this operation's starting implementation, not new Logical OR gains.

1. **Repair stored-sample parity before timing.** Public mode-1 writes retain arbitrary byte samples. Pillow Logical OR returns canonical 255 whenever either input is nonzero; the former bitwise result could be any nonzero byte. Fifteen new maintained cases expose **39 failures in 45 comparisons** before the fix (`perf-logical-or-stored-20260925-before-parity.json`). CPU, SIMD (including in-place and scalar helper paths), and GPU now canonicalize truth. The SIMD path tests the combined byte against zero. Raw output observations prevent packed serialization from concealing this defect.
2. **CPU scheduling.** Reuse the measured cheap-byte scheduling policy: sequential work below 4 MiB of decoded output, grouped complete rows above it. Retain each source's independent stride when clipping dimensions. The truth formula remains exact.
3. **GPU transport.** Admit a single equal-size mode-1 Logical OR to the existing native-byte executor. Pack four stored samples per GPU word, guard the active word count and truncate transport padding. Truth commutes with OR, so combine source words before one carry-isolated per-byte truth normalization; this shortcut is invalid for AND or XOR.

Added **15 parity cases and four size-varied benchmark workflows**, with zero existing parity cases changed or removed. The fresh throughput runner now accepts Logical OR with the same complete-request policy as AND: two fresh packed imports, operation, full packed export, synchronization, worker scheduling and receipt capture. It still rejects unsupported modes and requires actual native execution and full image/secondary transfer accounting.

After the repair, **462/462 maintained/workflow comparisons** and **18/18 large boundary comparisons** pass. Both suites pass again after scheduling/transport changes, including all 65,536 stored-byte pairs, raw output/input observations, a composed operation, native word/grid tails and both clipped source-stride directions. Final artifacts are `perf-logical-or-20260925-final-parity.json` and `perf-logical-or-20260925-final-tiles-parity.json`. This does not assert native execution for every clipped/invalid parity case.

The parity-correct baseline is `migration-benchmark-e57e2a7b607d461fa4166687c829782f` (`perf-logical-or-20260925-baseline.json`). Final run `migration-benchmark-d24f86c4afa14756baf3b9f4449a6ce7` (`perf-logical-or-20260925-final.json`) completes all seven workloads. Six materialized rows have native completed receipts without fallback; the standard row still lacks terminal native evidence. Final median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.014750 | 0.018626 | 0.016438 | 0.262354 |
| 1 × 1 | 0.014938 | 0.014584 | 0.015271 | 0.517458 |
| 32 × 32 | 0.013917 | 0.014875 | 0.015292 | 0.198729 |
| 256 × 256 | 0.061125 | 0.024396 | 0.024562 | 0.283334 |
| 1024 × 768 | 0.522958 | 0.131791 | 0.123667 | 0.971230 |
| SIMD Chops mode-1 workload | 0.637438 | 0.179500 | 0.179209 | 1.034020 |

Fresh 1024 × 768 runs `perf-logical-or-20260925-baseline-throughput.json` and `perf-logical-or-20260925-final-throughput.json` each pass **20,160 exact checks**, including **19,200 measured completions**, with unchanged source hashes and consistent runtime binaries. Final queue-one medians are Pillow **1.128938 ms**, CPU **0.118417 ms**, SIMD **0.118437 ms**, GPU **0.502938 ms**. Final completed fresh images per second:

| Queue depth | Pillow | CPU | SIMD | GPU |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 874.3 | 7174.4 | 7337.9 | 1920.6 |
| 2 | 1065.8 | 12159.8 | 11979.4 | 3514.6 |
| 4 | 1183.1 | 17377.1 | 17210.5 | 5104.8 |

Fresh CPU latency improves from **0.267729 to 0.118417 ms** (about 2.3×); GPU improves from **3.186021 to 0.502938 ms** (about 6.3×). SIMD remains essentially unchanged at **0.118583 → 0.118437 ms**. GPU throughput improves at every measured depth. Each GPU input/readback falls from 3,145,728 to **786,432 bytes**, parameters from 256 to 16, and mode conversions from one to zero. The odd-width 1031 × 769 fresh check passes **192/192 outputs** in `perf-logical-or-20260925-final-packed-check.json`.

Remaining blockers:

- CPU still misses the materialized small-operation and 32 × 32 rows. The 1 × 1 win in this run is not a universal fixed-overhead solution.
- SIMD reaches about **9.5× Pillow** on the fresh large workload, but misses 5× on the maintained rows. Wrapper/materialization costs and extra memory passes remain.
- GPU remains slower than SIMD in latency and throughput at every measured depth despite removing expansion. Launch, completion waits, allocation/output creation and decoded-byte transport remain. Resident or packed-bit execution must preserve canonical samples and count host conversion costs; the current gains do not prove those paths.
- Logical XOR still needs its own stored-sample audit. Wider bindings/platforms and composed pipelines remain unproven. The known LAB conversion failures remain at their earlier checkpoint; this OR-only audit did not rerun conversion.

The optimization skill now explains when truth normalization can move across an operator, allowing less work without changing the result. The isolated release build and formatting/public-boundary checks passed; documentation refresh uses existing evidence only. **No coverage collection or push ran.** Inventory is **16,334 parity cases and 797 workloads across 54 suites**. The selected matrix remains 208 operations plus one constant, with zero fully completed operations; broad timing evidence remains historical and focused-result integration remains pending.

## Logical XOR checkpoint — 2026-09-25

Logical OR is committed as `eec8a8608`. Logical XOR completes **three attempts**, all retained, then moves to Overlay under the attempt cap. Shared bit-I/O improvements from Logical AND are inherited baseline behavior.

1. **Stored-sample parity repair.** Two different nonzero mode-1 bytes are both true, so their logical XOR is false. The old bitwise kernels disagreed. Fifteen new maintained cases reproduce **39 failures in 45 comparisons** (`perf-logical-xor-stored-20260925-before-parity.json`). CPU, SIMD (including in-place and scalar helper paths), and GPU now compare operand truths and produce canonical 0/255 output. SIMD XORs zero-comparison masks directly: complementing both boolean inputs leaves XOR unchanged and avoids separate inversions.
2. **CPU scheduling.** Reuse the cheap-byte policy below/above 4 MiB of decoded output, retaining separate input strides and complete-row grouping. This changes scheduling without changing the repaired formula.
3. **GPU native-byte transport.** Reuse the existing executor for a single equal-size mode-1 XOR. Keep one nonzero flag per stored byte, XOR those flags, and expand only the final flags to 0/255. Word-count guards and returned lengths exclude transport padding; composed/unsupported layouts retain their existing correct paths.

Added **15 parity cases and four benchmark workloads**, with zero existing parity cases changed or removed. New cases cover every stored-byte pair, raw outputs and unchanged source samples, composed and pre-materialized inputs, partial vectors/words/grids, unequal strides and empty overlaps. The repaired baseline and final implementation each pass **459/459 maintained/workflow comparisons** plus **18/18 large boundary comparisons**. Final artifacts are `perf-logical-xor-20260925-final-parity.json` and `perf-logical-xor-20260925-final-tiles-parity.json`. Clipped/invalid parity cases are not claimed as native acceleration evidence.

Baseline run `migration-benchmark-65fd8d650463420f9f9c5eec61b73270` and final run `migration-benchmark-9584e6abc7184a3583c614a316dd69a5` complete all seven selected workloads (`perf-logical-xor-20260925-baseline.json` and `perf-logical-xor-20260925-final.json`). Six materialized rows have native completed receipts without fallback; the standard row still lacks terminal evidence. Final median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.013479 | 0.016688 | 0.016855 | 0.304500 |
| 1 × 1 | 0.012042 | 0.014813 | 0.015896 | 0.521479 |
| 32 × 32 | 0.013459 | 0.018792 | 0.015708 | 0.302479 |
| 256 × 256 | 0.062480 | 0.026146 | 0.024250 | 0.542938 |
| 1024 × 768 | 0.599292 | 0.165417 | 0.123500 | 0.918917 |
| SIMD Chops mode-1 workload | 0.753292 | 0.193020 | 0.188292 | 0.967938 |

Fresh 1024 × 768 baseline/final runs (`perf-logical-xor-20260925-{baseline,final}-throughput.json`) each pass **20,160 exact checks**, including **19,200 measured completions**, with unchanged source hashes and consistent runtime binaries. Final queue-one medians are Pillow **0.979624 ms**, CPU **0.116958 ms**, SIMD **0.121584 ms**, GPU **0.509729 ms**. Final completed images per second:

| Queue depth | Pillow | CPU | SIMD | GPU |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 1008.4 | 7419.6 | 7207.0 | 1867.5 |
| 2 | 1266.9 | 12210.4 | 12004.7 | 3113.4 |
| 4 | 1460.9 | 17936.9 | 17695.1 | 5268.2 |

CPU fresh latency improves from **0.283063 to 0.116958 ms** (about 2.4×); GPU improves from **3.172708 to 0.509729 ms** (about 6.2×), with higher throughput at every measured depth. SIMD remains close to baseline (**0.122583 → 0.121584 ms**). Each GPU input/readback is **786,432 bytes** instead of 3,145,728; parameters shrink from 256 to 16 bytes and mode conversions from one to zero. The fresh runner includes both imports, full packed export, synchronization and receipts. The final odd-width 1031 × 769 check passes **192/192 outputs** in `perf-logical-xor-20260925-final-packed-check.json`.

Remaining blockers:

- CPU still misses small public calls; some small timings regress or vary. Large scheduling wins do not establish the all-input CPU target.
- SIMD reaches about **8.1× Pillow** on the fresh large workload, but every maintained row still misses 5×, including 1024 × 768 at about 4.85×. Fixed caller/materialization costs and memory passes remain.
- GPU still trails SIMD in latency and throughput at all measured depths. The 1 × 1 GPU row regresses relative to baseline, and 256 × 256 is slightly slower. Launch/wait/output costs and decoded-byte transport remain unresolved; resident/packed execution must include conversion and preserve canonical truth samples.
- Wider bindings/platforms and composed pipelines remain unproven. The earlier LAB conversion blockers remain; this XOR-only audit did not rerun conversion.

The skill records keeping predicates compact until the final store and cancelling shared XOR inversions. The isolated release build and formatting/public-boundary checks passed. Generated documentation was refreshed from existing evidence only. **No coverage collection or push ran.** Inventory is **16,349 parity cases and 801 workloads across 54 suites**. The selected matrix still has 208 operations plus one constant and zero fully completed operations; broad timings remain historical and focused-result integration remains pending. Next is `ImageChops.overlay`, whose historical selected rows show CPU, SIMD and GPU deficits; collect current parity and baseline evidence before changing it.

## Overlay baseline — 2026-09-25

Logical XOR is committed as `8906fed2c`; work moves to `ImageChops.overlay`. **No Overlay implementation attempt has been made yet.** The original 48 cases plus two workflows pass **150/150 comparisons**. The expanded audit adds **127 maintained cases** without changing or removing existing cases: 19 modes and six shapes, all stored-byte pairs in L, mutation-created mode-1 samples with raw result/input observations, materialized and composed execution, and tails/clipping/empty inputs. Four benchmark size variants are also added without changing prior workloads. The expanded current implementation passes **543/543 comparisons** in `perf-overlay-20260925-expanded-parity.json`.

Run `migration-benchmark-63f0094bcb14472d9735d8de5f1f20b5` (`perf-overlay-20260925-baseline.json`) completes all seven selected workloads. Six materialized rows have completed native backend receipts without fallback; the standard row remains a terminal-evidence gap. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015438 | 0.017167 | 0.018812 | 0.361396 |
| Existing 32 × 24 | 0.016229 | 0.015938 | 0.021917 | 0.542542 |
| 1 × 1 | 0.013000 | 0.014729 | 0.015292 | 0.313688 |
| 32 × 32 | 0.016937 | 0.016021 | 0.023459 | 0.357480 |
| 256 × 256 | 0.254750 | 0.100730 | 0.530729 | 0.713459 |
| 1024 × 768 | 3.438416 | 0.726271 | 6.463729 | 3.471583 |

The maintained fresh runner now supports Overlay with both fresh inputs and full output/receipt accounting. L/RGB 1024 × 768 baseline `perf-overlay-20260925-baseline-throughput.json` passes **40,320 exact checks**, including **38,400 measured completions**, with unchanged source hashes and consistent runtime binaries. Queue-one median milliseconds:

| Mode | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.902249 | 0.237917 | 2.178729 | 3.186229 |
| RGB | 4.449333 | 0.617021 | 6.704125 | 2.993438 |

Completed fresh images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1083.8 | 3844.7 | 453.9 | 306.9 |
| L | 2 | 1130.1 | 5836.3 | 859.7 | 466.2 |
| L | 4 | 1129.2 | 6547.0 | 1567.1 | 672.4 |
| RGB | 1 | 221.2 | 1347.6 | 146.7 | 318.9 |
| RGB | 2 | 245.2 | 1672.4 | 278.3 | 491.1 |
| RGB | 4 | 255.3 | 1780.4 | 486.6 | 679.2 |

SIMD is substantially slower than both CPU and Pillow. Its current shared Overlay/HardLight loop pads and copies every eight-byte block, widens to 32-bit lanes, evaluates both branches, and narrows through scalar arrays. Selecting the active branch before multiplication bounds at least one factor by 127 and the other by 255, so the product is at most 32,385. The candidate exact quotient `n = product + 1; (n + (n >> 7) + (n >> 14)) >> 7` matches integer division by 127 for all **32,386** possible products; its maximum sum is 32,640. This math-only check is retained as `perf-overlay-20260925-div127-proof.json`, and is neither an implementation attempt nor a performance claim. If applied to the shared helper, HardLight must receive affected parity verification too.

GPU still transports 3,145,728 bytes for each input and readback in both modes, with 256 parameter bytes and one mode conversion. Native-byte transport is a candidate after the SIMD bottleneck. CPU uses a 64 KiB pair lookup table; do not assume the cheap arithmetic scheduling crossover applies without measurement. Tiny caller overhead, true composed throughput, wider modes/bindings/platforms and the global targets remain unresolved.

Generated documentation is refreshed from existing evidence only. **No coverage collection or push ran.** Inventory is **16,476 parity cases and 805 workloads across 54 suites**. The selected matrix remains 208 operations plus one constant and zero fully completed operations; broad timings remain historical and focused-result integration remains pending.

## Overlay checkpoint — 2026-09-25

Overlay completes **three implementation attempts** after baseline commit `fa69c7ce3`. Two are retained and the third is rejected. Work moves to HardLight; Overlay remains incomplete.

1. **Retained: narrower exact SIMD arithmetic and complete blocks.** Select the low/high branch operands before multiplying, so at least one factor is at most 127. Products are at most 32,385, allowing sixteen 16-bit lanes instead of eight 32-bit lanes. For `n = product + 1`, `(n + (n >> 7) + (n >> 14)) >> 7` is exact division by 127 over this range, with a maximum intermediate sum of 32,640. Widen and narrow with existing vector helpers, process complete sixteen-byte arrays directly, and pad only the final partial block. This removes duplicated branch arithmetic and per-block padded copies without approximating Pillow's formula. HardLight shares the helper and receives affected parity checks.
2. **Retained: native-byte GPU transport.** Reuse the existing byte executor for one eligible equal-size Overlay operation. Four independent stored samples occupy each word; guard the active word count and exclude transport padding from output. Preserve the existing admission checks and correct routes for other layouts/composed work. The shader's exact arithmetic is unchanged.
3. **Rejected: parallel SIMD tiles from 1 MiB.** Disjoint 64 KiB vector-aligned tiles preserve parity. Relative to attempt two, fresh RGB queue-one latency falls from **0.691834 to 0.561084 ms**, but queue-depth-four throughput falls from **2,532.5 to 2,245.2 images/s**. The maintained large SIMD row barely changes (**0.835209 → 0.820813 ms**). The experiment does not establish a sufficient latency-and-throughput gain and is removed. Subsequent final measurements are slower for several unchanged subjects too; the timing variability prevents attributing all differences to scheduling. The rejected candidate and both surrounding runs remain available, rather than selecting only the favorable medians. No fourth tuning attempt is made.

The finite-domain proofs check all **32,386 possible products** against integer division and all **65,536 byte pairs for each of Overlay and HardLight** against live Pillow and the existing lookup tables. Two focused Rust tests exercise the actual SIMD helpers and pass. The proof artifacts are `perf-overlay-20260925-div127-proof.json` and `perf-overlay-20260925-selected-branch-proof.json`.

The restored final runtime passes **543/543 Overlay comparisons**, **666/666 affected HardLight comparisons**, and **384/384 fresh odd-width L/RGB outputs**. Artifacts are `perf-overlay-20260925-final-parity.json`, `perf-overlay-hardlight-20260925-final-parity.json`, and `perf-overlay-20260925-final-odd-check.json`. Earlier retained attempt two also passes **18/18 large Overlay boundary comparisons**. The rejected parallel candidate separately passes **30/30 Overlay and 30/30 HardLight threshold/tail/clipped-stride comparisons**; those are candidate evidence, not additional final-run comparisons. No existing assertions, cases, workloads or thresholds were weakened. Invalid/clipped parity cases are not all claimed as native acceleration.

Final unchanged-policy benchmark `migration-benchmark-6bcb0be15f7f47a6a723c184e07832d0` (`perf-overlay-20260925-final.json`) completes all seven workloads. Six materialized rows have completed native backend receipts without fallback; the standard deferred row still lacks terminal execution evidence. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.018458 | 0.017959 | 0.021354 | 0.437416 |
| Existing 32 × 24 | 0.017313 | 0.017375 | 0.017688 | 0.217145 |
| 1 × 1 | 0.012854 | 0.016146 | 0.017125 | 0.196333 |
| 32 × 32 | 0.017188 | 0.017354 | 0.017792 | 0.389541 |
| 256 × 256 | 0.258124 | 0.097271 | 0.066959 | 0.372271 |
| 1024 × 768 | 3.545459 | 0.701792 | 0.819020 | 1.826626 |

Fresh L/RGB 1024 × 768 final evidence (`perf-overlay-20260925-final-throughput.json`) passes **40,320 exact checks**, including **38,400 measured completions**, with unchanged source hashes and consistent runtime binaries. Both fresh imports, the operation, full export, allocation and synchronization remain inside the request boundary. Final queue-one median milliseconds:

| Mode | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.951063 | 0.248833 | 0.231062 | 0.484521 |
| RGB | 4.771959 | 0.740187 | 0.790021 | 1.454937 |

Final completed fresh images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1004.2 | 3643.2 | 4082.3 | 1664.7 |
| L | 2 | 1045.4 | 5384.7 | 6579.8 | 2388.2 |
| L | 4 | 1035.5 | 6072.4 | 8531.9 | 3803.0 |
| RGB | 1 | 197.3 | 1045.5 | 1165.9 | 540.0 |
| RGB | 2 | 227.8 | 1394.3 | 1136.9 | 1000.2 |
| RGB | 4 | 241.9 | 1383.9 | 1153.9 | 1020.0 |

Against the original fresh baseline, final SIMD latency improves from **2.178729 to 0.231062 ms in L** and **6.704125 to 0.790020 ms in RGB** (about **9.4×/8.5×**). GPU improves from **3.186229 to 0.484521 ms in L** and **2.993438 to 1.454938 ms in RGB** (about **6.6×/2.1×**). GPU transfers each input/readback as **786,432 L bytes** or **2,359,296 RGB bytes**, formerly 3,145,728 in both modes; parameters fall from 256 to 16 bytes and mode conversions from one to zero. CPU is unchanged in this visit. The intermediate `attempt1`, `attempt2` and `attempt3` benchmark/throughput artifacts isolate each decision and expose run-to-run variability.

Remaining blockers and next decisions:

- CPU still misses several tiny public-call rows. It retains the 64 KiB pair LUT and its original scheduling; applying a cheap-byte crossover without measuring lookup/cache costs is not justified.
- SIMD exceeds 5× Pillow on the final fresh RGB workload (about **6.0×**), but reaches only about **4.1×** in fresh L and misses 5× on every final maintained row. Attribute allocation, caller/materialization costs and memory passes before another instruction-level rewrite.
- GPU remains slower than SIMD in both latency and throughput at every final measured queue depth. Native transport removes expansion, but launch, mapping/waits and output construction remain. Attribute those phases and evaluate resident/composed work with complete transfer and fresh-output accounting.
- Small timings vary and some regress. The rejected scheduling experiment has no proven universal crossover; future work needs controlled concurrency/bandwidth evidence rather than assuming more threads help. Wider bindings, platforms, modes and composed pipelines still lack complete performance proof.
- The pre-existing mode-1 to LAB parity failures remain at their earlier conversion checkpoint. This Overlay audit does not claim to repair or rerun that separate operation.

The skill now explains selecting branch operands before range reduction, exact bounded division and why internal parallelism must be evaluated under request concurrency. The isolated release build and formatting/public-boundary checks pass. **No coverage collection or push ran.** Inventory remains **16,476 parity cases and 805 workloads across 54 suites**; the selected matrix retains 208 operations plus one constant and **zero fully completed operations**. Broad timing evidence remains historical and focused-result integration remains pending. HardLight is next, inheriting the verified shared SIMD improvement as its starting point.

## HardLight baseline — 2026-09-25

Overlay is checkpointed after three attempts as `30c0ac522`. Work moves to `ImageChops.hard_light`; **zero HardLight-specific implementation attempts** have been made. The exact sixteen-lane SIMD helper retained during Overlay is inherited baseline behavior, not a new HardLight gain.

The maintained audit adds **127 cases** and **four benchmark workloads**, with zero existing cases or workloads changed or removed. It exercises 19 modes, vector tails, clipping and empty inputs, exhaustive L byte pairs, mutation-created mode-1 samples with raw result/input observations, materialized inputs and composed execution. The expanded baseline passes **540/540 comparisons** in `perf-hardlight-20260925-baseline-parity.json`. The earlier shared-helper audit additionally established exact HardLight SIMD arithmetic across every byte pair.

All seven selected workloads complete in `migration-benchmark-acfc50eae75844aebac94b9cc4b5008e`, artifact `perf-hardlight-20260925-baseline.json`. Six materialized rows have completed native CPU/SIMD/GPU receipts without fallback; the standard deferred row remains a terminal-evidence gap. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015500 | 0.018479 | 0.018917 | 0.527625 |
| Existing 32 × 24 | 0.017062 | 0.017500 | 0.016375 | 0.269021 |
| 1 × 1 | 0.013083 | 0.016792 | 0.015584 | 0.267583 |
| 32 × 32 | 0.017271 | 0.017355 | 0.016771 | 0.398208 |
| 256 × 256 | 0.256917 | 0.090083 | 0.054208 | 0.644563 |
| 1024 × 768 | 3.438063 | 0.648062 | 0.741958 | 3.286375 |

The fresh runner now supports HardLight with the same complete two-input policy and receipt accounting. L/RGB 1024 × 768 baseline `perf-hardlight-20260925-baseline-throughput.json` passes **40,320 exact checks**, including **38,400 measured completions**, with unchanged source hashes and consistent runtime binaries. Queue-one median milliseconds:

| Mode | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.893750 | 0.230313 | 0.191459 | 3.005875 |
| RGB | 4.435291 | 0.678646 | 0.700354 | 2.993021 |

Completed fresh images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1105.2 | 4004.2 | 4886.9 | 283.9 |
| L | 2 | 1139.6 | 5992.4 | 7453.2 | 480.1 |
| L | 4 | 1131.1 | 6947.7 | 9835.6 | 591.2 |
| RGB | 1 | 223.8 | 1259.2 | 1396.1 | 276.4 |
| RGB | 2 | 223.1 | 1694.8 | 2152.0 | 455.1 |
| RGB | 4 | 249.2 | 1850.8 | 2762.3 | 563.1 |

GPU latency is about **15.7× SIMD in L** and **4.3× in RGB**, and its throughput trails SIMD at every tested depth. It still expands the inputs to RGBA transport. First investigate admitting eligible equal-size HardLight work to the established native-byte executor, preserving its distinct branch condition on the second sample and active-word bounds. CPU retains a 64 KiB pair LUT and fine-row scheduling; compare its scheduling cost with sequential/grouped work before borrowing the cheap-byte threshold. SIMD reaches about **6.3× Pillow in fresh RGB**, but only **4.7× in L** and under 5× on every maintained row. Do not repeat Overlay's rejected internal parallelism experiment without new evidence that resolves the concurrency tradeoff.

Generated documentation was refreshed from existing evidence; input generation updated static coverage declarations only. **No coverage collection or push ran.** Inventory is now **16,603 parity cases and 809 workloads across 54 suites**. The selected matrix retains 208 operations plus one constant and zero fully completed operations. Broad timings remain historical; merging focused results into the global matrix and auditing public exports outside the manifest remain pending.

## HardLight checkpoint — 2026-09-25

HardLight completes **three attempts**. Attempts one and two are retained; attempt three is rejected. Work moves to SoftLight; HardLight remains incomplete.

1. **Native-byte GPU route.** Admit one eligible equal-size HardLight operation to the existing executor. Treat each stored byte as an independent sample, preserve HardLight's condition on operand two, handle partial words with an active-word guard, and leave composed/unsupported layouts on their existing correct route. This removes RGBA expansion and the mode conversion.
2. **Exact branch-selected arithmetic.** Before multiplying, select the active side's operands. One factor is then at most 127 and the other at most 255, bounding the product at 32,385. Replace branch-local division and two possible products with `n = product + 1; q = (n + (n >> 7) + (n >> 14)) >> 7`, which exactly computes floor division by 127 on this bounded domain. Return `q` on the low branch and `255 - q` on the high branch. Maximum intermediate is 32,640. This improves measured concurrent GPU throughput.
3. **Rejected native-byte mode branch.** A dedicated mode-9 branch removed the existing channel `select`s, but did not help. Against attempt two, L fresh queue-depth-four throughput fell from **4,197 to 3,933 images/s**, and RGB fell from **1,358 to 1,198**. It is removed. An extra per-invocation mode branch can outweigh a few uniform channel selects; inspect generated shader and measure full-request throughput before adding a specialization.

After the final restore, the release parity build succeeds. Exact parity passes **540/540 maintained comparisons**, **30/30 large boundary comparisons**, and **384/384 fresh odd-width L/RGB outputs**. The retained arithmetic candidate separately passes the same 540+30 comparisons. Artifacts: `perf-hardlight-20260925-final-parity.json`, `perf-hardlight-20260925-attempt2-tiles-parity.json`, and `perf-hardlight-20260925-final-odd-check.json`. The attempt-three branch also passes parity; its timing is the reason for rejecting it, not a correctness issue. No existing assertion or workload changed.

Final retained benchmark `migration-benchmark-58c184625ba243029670fe87eb1d218a` (`perf-hardlight-20260925-attempt2.json`) completes all seven selected workloads. Six materialized rows have terminal native CPU/SIMD/GPU receipts with no fallback; the standard deferred row still lacks terminal execution evidence. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015875 | 0.017937 | 0.019584 | 0.440229 |
| Existing 32 × 24 | 0.015855 | 0.018625 | 0.018313 | 0.201603 |
| 1 × 1 | 0.012688 | 0.016417 | 0.016792 | 0.390000 |
| 32 × 32 | 0.017063 | 0.017563 | 0.017813 | 0.204438 |
| 256 × 256 | 0.250625 | 0.090833 | 0.056334 | 0.614771 |
| 1024 × 768 | 3.866313 | 0.756500 | 0.835646 | 2.235292 |

Fresh L/RGB 1024 × 768 evidence (`perf-hardlight-20260925-attempt2-throughput.json`) passes **40,320 exact checks**, including **38,400 measured completions**, with stable source hashes and runtime identities. Both input constructions, HardLight, full output export, scheduling and completion stay inside the request. Queue-one median milliseconds:

| Mode | Pillow | CPU | SIMD | GPU |
| L | 0.899729 | 0.235292 | 0.202583 | 0.507333 |
| RGB | 4.449999 | 0.623375 | 0.687208 | 1.191208 |

Completed fresh images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| L | 1 | 1073.4 | 3745.0 | 4653.2 | 1863.8 |
| L | 2 | 1097.7 | 5511.0 | 6912.3 | 2873.7 |
| L | 4 | 1094.9 | 6464.4 | 8981.2 | 4197.1 |
| RGB | 1 | 221.0 | 1311.0 | 1357.8 | 785.6 |
| RGB | 2 | 242.7 | 1733.8 | 2071.8 | 1207.6 |
| RGB | 4 | 254.9 | 1782.1 | 2596.3 | 1357.8 |

Against the fresh baseline, GPU latency falls from **3.006 to 0.507 ms in L** and **2.993 to 1.191 ms in RGB** (about **5.9×/2.5×**). At queue depth four, GPU throughput rises from **591 to 4,197 L images/s** and **563 to 1,358 RGB images/s**. Each GPU input/readback transfers 786,432 L bytes or 2,359,296 RGB bytes; the former route transferred 3,145,728 bytes for each mode/input. Uniform/native mode uses 16 parameter bytes with no format conversion. These changes preserve exact sample values.

Remaining blockers:

- GPU still trails SIMD in fresh latency and throughput at every measured queue depth. The arithmetic and transport gains do not meet the GPU target. Launch, completion, output creation and remaining transfers need phase attribution before further shader work.
- SIMD reaches about **4.4× Pillow in L** and **6.5× in RGB** on the final fresh request. It misses 5× in L and misses 5× on the small maintained rows. Shared arithmetic is already narrowed to sixteen 16-bit lanes, so inspect wrapper/materialization and complete-request costs before more lane-level tuning.
- CPU misses Pillow on tiny maintained calls; CPU is already faster on larger calls. That small-call floor is shared binding/allocation work and needs direct phase evidence.
- Run-to-run latency varies materially. The three attempt artifacts remain side by side. No all-size or all-mode performance claim is established; wider platforms and composed execution remain unproven.

The optimization skill records exact bounded division and the rejected uniform shader branch lesson. The `RUSTC_WRAPPER= make build-parity` build passes. Formatting and public API boundary checks pass; parity and full-request benchmarks above pass. **No coverage collection or push ran.** The pre-existing operation-wide inventory remains **16,603 cases, 809 workloads, zero fully completed operations**. Focused results have not been merged into the global optimization matrix. SoftLight is next.

## SoftLight baseline — 2026-09-25

HardLight was checkpointed after three attempts in `3089962ec`. Work moves to `ImageChops.soft_light`; no SoftLight implementation change has been made yet. The baseline audit adds **127 parity cases** and **four benchmark workloads**, with zero existing cases or workloads changed or removed. Cases include every stored-byte pair, mutation-created mode-1 samples and raw result observations, 19 supported mode layouts, tails, clipping, empty inputs and composed execution. The expanded current implementation passes **543/543 comparisons** in `perf-softlight-20260925-baseline-parity.json`.

The baseline run `migration-benchmark-b4dd7cf0394f4f6483ab048537d70d1c` (`perf-softlight-20260925-baseline.json`) completes all seven selected workloads. Six materialized rows have terminal requested-backend receipts without fallback; the standard deferred row remains a terminal-execution gap. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015875 | 0.020646 | 0.021104 | 0.266354 |
| Existing 32 × 24 | 0.017167 | 0.018604 | 0.018041 | 0.345959 |
| 1 × 1 | 0.012979 | 0.017396 | 0.017417 | 0.181688 |
| 32 × 32 | 0.018250 | 0.017667 | 0.018687 | 0.295166 |
| 256 × 256 | 0.311063 | 0.090355 | 0.107729 | 0.430812 |
| 1024 × 768 | 4.514375 | 0.636875 | 0.806999 | 2.200229 |

Fresh L/RGB 1024 × 768 baseline (`perf-softlight-20260925-baseline-throughput.json`) passes **40,320 exact checks**, including **38,400 measured completions**. Source hashes remain unchanged and runtime files match across isolated subjects. Median milliseconds:

| Mode | Pillow | CPU | SIMD | GPU |
| L | 0.981209 | 0.290583 | 1.461000 | 3.840666 |
| RGB | 4.687188 | 0.853854 | 4.535812 | 3.782374 |

The largest baseline row identifies the primary blockers: SIMD takes **4.783 ms** on 1024 × 768 versus Pillow at **4.420 ms**, and GPU takes **4.446 ms**. The fresh runner includes two input constructions, the SoftLight operation, complete export, scheduling and completion; it prevents a deferred result or native fallback from appearing as acceleration.

## SoftLight checkpoint — 2026-09-25

SoftLight completes **three attempts**, all retained. The operation remains incomplete and the next operation can start after this checkpoint.

1. **Retained: 16-lane 16-bit SIMD arithmetic.** The original eight-lane kernel widened each byte to 32 bits and converted lane vectors to scalar arrays twice to call the exact `/255` helper. It also padded/copy-filled every full eight-byte block. Keep divide-by-255 products in 16-bit vectors: each is at most 65,025 and `floor(x/255) = (n + (n >> 8)) >> 8`, with `n = x + 1`; maximum folded numerator is 65,280. For term one, `c = (255-a)*a`, `h = c >> 8`, `l = c & 255`; then `floor(c*b/65536) = (h*b + ((l*b) >> 8)) >> 8`. This keeps the decomposition exact while the bounded intermediate sum fits 16 bits. Process full sixteen-byte blocks directly and pad only the final tail. Preserve Pillow's two intermediate truncations and final saturation.
2. **Retained: native-byte GPU transport.** Treat each stored byte as an independent SoftLight sample and reuse the bounded native-byte executor. Pack four samples per word, guard the active word count and remove the four-channel expansion. Keep exact division order and exclude transport padding from the result.
3. **Retained: tiled parallel SIMD above 1 MiB.** The SoftLight arithmetic is heavier than bytewise blends; give the parallel pool disjoint vector-aligned 64 KiB tiles only above the measured threshold. Keep small inputs serial. The 1 MiB L request stays serial; RGB crosses the threshold and improves measured latency and concurrent throughput. This SoftLight crossover is evidence for its compute-heavy kernel only; do not copy it to other operations.

Every attempt passes **543/543 maintained comparisons**. Attempt-specific boundary probes each pass **30/30 comparisons** around vector, word, workgroup, threshold and clipping edges. Final odd-width fresh checks pass **384/384 outputs**. Exact parity includes the exhaustive L and mode-1 stored-byte cases; no existing assertion, case or threshold was changed. Artifacts include `perf-softlight-20260925-attempt3-parity.json`, `perf-softlight-20260925-attempt3-tiles-parity.json`, and `perf-softlight-20260925-final-odd-check.json`.

The final retained benchmark `migration-benchmark-035e0d11b0ac4ea6be7699c769164ea5` (`perf-softlight-20260925-attempt3.json`) completes all seven workloads. Six materialized rows have terminal native backend receipts without fallback. Median milliseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Materialized operation | 0.015875 | 0.020646 | 0.021104 | 0.266354 |
| Existing 32 × 24 | 0.017167 | 0.018604 | 0.018041 | 0.345959 |
| 1 × 1 | 0.012979 | 0.017396 | 0.017417 | 0.181688 |
| 32 × 32 | 0.018250 | 0.017667 | 0.018687 | 0.295166 |
| 256 × 256 | 0.311063 | 0.090355 | 0.107729 | 0.430812 |
| 1024 × 768 | 4.514375 | 0.636875 | 0.806999 | 2.200229 |

Final fresh L/RGB 1024 × 768 evidence (`perf-softlight-20260925-attempt3-throughput.json`) passes **40,320 checks**, including **38,400 measured completions**, with stable source hashes/runtime identities. Final queue-one median milliseconds:

| Mode | Pillow | CPU | SIMD | GPU |
| L | 0.954563 | 0.282875 | 0.413187 | 0.524875 |
| RGB | 4.684730 | 0.742834 | 0.719187 | 1.315354 |

Final completed fresh images per second:

| Mode | Queue depth | Pillow | CPU | SIMD | GPU |
| L | 1 | 958.0 | 2966.9 | 2215.3 | 1409.0 |
| L | 2 | 952.6 | 3698.4 | 3940.4 | 2190.7 |
| L | 4 | 998.8 | 4788.0 | 5356.5 | 2944.3 |
| RGB | 1 | 201.8 | 1043.9 | 1124.6 | 547.7 |
| RGB | 2 | 230.4 | 1178.1 | 1366.8 | 834.3 |
| RGB | 4 | 223.6 | 1268.8 | 1628.6 | 1064.2 |

SIMD improves fresh queue-one latency from **1.461 to 0.413 ms in L** and **4.536 to 0.719 ms in RGB** (about **3.5×/6.3×** versus its starting implementation). At queue depth four, its throughput rises from **2,151 to 5,357 L images/s** and **623 to 1,629 RGB images/s**. GPU fresh latency falls from **3.841 to 0.525 ms in L** and **3.782 to 1.315 ms in RGB** after native-byte transport. GPU queue-depth-four throughput rises from **528 to 2,944 L images/s** and **458 to 1,064 RGB images/s**. The added SIMD tiling halves the materialized 1024 × 768 SIMD row from **1.507 to 0.807 ms** and increases fresh RGB SIMD throughput.

Remaining blockers:

- SIMD meets 5× only on fresh RGB and the maintained 1024 × 768 RGB row. It reaches only about **2.3×** on fresh L, and small maintained rows still miss the target.
- GPU remains slower than SIMD in latency and completed throughput for both modes. Queue-one GPU latency is **0.525 vs 0.413 ms in L** and **1.315 vs 0.719 ms in RGB**. Submission, completion wait, readback and output creation remain; attribute these costs before further shader arithmetic work.
- CPU is faster than Pillow at larger sizes, but misses the smallest fixed-cost rows. That does not come from this SIMD/GPU work and needs its own call-phase diagnosis.
- Timings vary across runs. The same-policy baseline and each attempt remain side-by-side. No wide-platform, all-composition or all-size completion claim is established.

The reusable skill now records the bounded 16-bit division/reduction used to remove scalar SIMD lane extraction, including the intermediate limits needed to preserve exact truncation. The release `build-parity` build, formatting and public API boundary checks pass. **No coverage collection or push ran.** Inventory is now **16,730 cases and 813 workloads across 54 suites**. The operation-wide manifest remains 208 operations plus one constant with zero fully completed operations; focused results are not integrated into its broad historical matrix. Next is operation selection from the remaining ranked gaps.

## ImageChops Composite four-attempt checkpoint — 2026-09-25

The next operation visit is `PIL.ImageChops.composite`, shared with
`PIL.Image.composite` through `CompositeModule`. The input generator adds 31
input-only pixel cases and four L benchmark sizes. The expanded cohort contains
47 composite comparisons and passes **47/47 on CPU, 47/47 on SIMD, and 47/47
on Metal GPU** under strict backend selection. No assertion, threshold or
expected output was weakened. Expanding the mask/destination matrix exposed a
SIMD empty-overlap bug: disjoint source and destination must return an
image2-sized copy, including zero-sized outputs. Both image and shape planners
now encode empty intersection as a valid zero-area region; the result builder
admits empty dimensions. The implementation retains that parity fix.

The baseline identified three avoidable costs: CPU widened L data to RGB and
then narrowed it again while calling `get_pixel`/`put_pixel` per sample; CPU
and SIMD materialized the L mask before use; GPU expanded all inputs/results
to packed RGBA and moved six times as many bytes as the native L case requires.
The 1024 × 768 L medians by bounded attempt are:

| Run | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Baseline | 0.438625 | 3.629125 | 0.740146 | 8.345083 |
| Attempt 1 | 0.456375 | 0.847521 | 0.718562 | 6.534021 |
| Attempt 2 | 0.402021 | 0.588000 | 0.722083 | 5.645917 |
| Attempt 3 trial | 0.399083 | 0.576167 | 0.901250 | 5.484250 |
| Attempt 4 | 0.334833 | 0.477209 | 0.698230 | 1.193020 |

Four attempts were made and the checkpoint is ready for the next operation:

1. **Retained CPU native-byte path.** For matching L/LA/RGB/RGBA operands,
   borrow stored samples, copy image2's native bytes for the required output
   canvas, and blend only the overlap. This removes RGB expansion and per-pixel
   image accessors. It cuts the large L request from 3.63 ms to 0.85 ms, but
   remains slower than Pillow.
2. **Retained direct mask reads and row parallelism.** For an L mask, read its
   byte directly; for supported LA/RGBA alpha masks, select the stored alpha
   band. This avoids a full GrayImage conversion. Reuse the existing
   `apply_effect_rows` threshold for large destinations while keeping small
   images serial. CPU L falls to 0.59 ms and fresh RGB throughput rises above
   Pillow at each measured host queue depth.
3. **Rejected SIMD one-pass output construction.** Appending newly blended
   vector blocks to a capacity-reserved Vec avoided copying image2 into an
   initialized output first. Exact parity held, but 1024 × 768 L latency
   regressed from 0.72 to 0.90 ms. The experiment was reverted. The copy plus
   vector-update path remains.
4. **Retained native-byte GPU composite.** For full-size, same-mode operands
   and supported L/alpha masks, pack four output bytes per storage word and
   move source, image2, mask and output in their native layouts. Dispatch one
   guarded shader and exclude transfer padding from the returned image. On
   1024 × 768 L, input traffic falls from 3,145,728 to 2,359,296 bytes,
   auxiliary traffic from 6,291,456 to 1,572,864 bytes, readback from
   3,145,728 to 786,432 bytes, and mode-conversion count from one to zero.
   Latency falls from 5.65 to 1.19 ms. Clipping, empty images, palette and
   mixed-mode operands, and other mask layouts retain their existing route.

Attempt 4 also measures seven maintained L workloads. CPU remains slower than
Pillow on all seven, including 1 × 1, 32 × 32, 256 × 256 and 1024 × 768. SIMD
does not meet the 5× requirement on any measured row. At 1024 × 768, it is
0.70 ms versus Pillow's 0.33 ms (2.1× slower). GPU launch cost dominates small
materialized calls: 16 × 16 is about 0.56 ms versus Pillow's 0.02 ms.

Fresh three-input request windows verify L and RGB separately at 1024 × 768.
Each run uses 16 changing inputs, queue depths 1/2/4, five warmup windows and
five samples of 20 windows. Queue-one through queue-four completed operations
per second are:

| Mode | Queue | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 3,138 | 2,007 | 1,414 | 1,200 |
| L | 2 | 4,027 | 2,445 | 1,436 | 1,225 |
| L | 4 | 4,829 | 2,289 | 1,444 | 1,207 |
| RGB | 1 | 483 | 954 | 463 | 517 |
| RGB | 2 | 705 | 990 | 476 | 536 |
| RGB | 4 | 895 | 1,064 | 478 | 540 |

CPU throughput is **0.47–0.64× Pillow in L** and **1.19–1.97× in RGB**.
SIMD ranges from **0.30–0.45× Pillow in L** and **0.53–0.96× in RGB**, far
below 5×. The native GPU route is **0.84–0.85× SIMD in L** and **1.11–1.13×
SIMD in RGB**. Thus the GPU now exceeds SIMD RGB throughput but still misses
the L throughput and all-mode latency goals. It remains slower than SIMD in
both large L latency (1.19 versus 0.70 ms) and most small workloads.

Local parity and performance receipts use `perf-composite-20260925-` under
`build/migration-parity/`. The current blocker list is CPU L latency and L
throughput; SIMD's 5× target across both modes and sizes; GPU L throughput and
latency plus its small-request launch floor; and broad composed/API coverage.
The native GPU eligibility and fallback boundary are part of the evidence;
passing fallback does not count as acceleration. No coverage was collected and
no push ran. Generated contract docs now index **16,761 parity cases and 817
benchmark workloads**. Aggregating existing receipts reports zero compatible
evidence IDs and three stale/incompatible artifacts; focused Composite receipts
are not integrated into that global evidence schema. The work is checkpointed
here so optimization can move to the next operation and revisit Composite only
when new profiling evidence targets one of these remaining limits.
