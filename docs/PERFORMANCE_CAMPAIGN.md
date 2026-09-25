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
