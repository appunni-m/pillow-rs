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

### Equalize blocker revisit (2026-09-25)

Three bounded attempts revisited the remaining GPU and small-input SIMD gaps.
Strict public parity passed all 181 selected equalize cases on CPU, SIMD, and
GPU after the retained changes. No coverage collection was run.

The first attempt replaced Equalize's histogram-clear compute dispatch with
`CommandEncoder::clear_buffer`. Its strict GPU receipt reports three compute
dispatches instead of four. The initial paired workload run measured backend
medians of 342/287/402/1,264 µs for masked L 64 × 64 and RGB 32 × 32, 256 × 256,
and 1024 × 768; after the clear they were 423/288/386/1,286 µs. This removes
device work but does not demonstrate a latency or throughput win. Later samples
varied more, so the structural dispatch reduction is retained without claiming
faster requests. Autocontrast keeps its compute clear.

The second attempt fused histogram construction, the exact integer CDF scan,
and remapping into one workgroup for native L/RGB inputs up to 4,096 pixels,
including masks. It passed all 181 strict cases after excluding the internal
shader from host LUT extraction. The one-workgroup backend medians were 745 µs
for masked L 64 × 64 and 591 µs for RGB 32 × 32, versus 342 and 287 µs before
fusion. The fused shader was removed: its local atomics and cross-lane scan
underused the device, so the extra synchronization cost outweighed two fewer
dispatches.

The third attempt uses direct scalar table reads for Equalize's Luma8 SIMD
path at up to 4,096 samples, where the portable 16-lane LUT expands each block
into sixteen swizzles and a chain of lane selects. The focused SIMD backend
median moved from 4.6 to 4.0 µs across the two receipts, but the whole masked
workflow measured 36.5 to 38.3 µs; that is not a demonstrated end-to-end win.
RGB remains on the vector path because the scalar experiment showed no clear
benefit for its interleaved three-channel layout. This is a size/layout-specific
tradeoff, not evidence that scalar LUT application is generally faster.

The final unchanged-policy receipt is
`build/migration-parity/perf-imageops-equalize-20260925-attempt3b.json` with its
parity sidecar. Its measured whole-workflow medians are:

| Equalize workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Masked L 64 × 64 | 0.0635 | 0.0384 | 0.0383 | 0.3920 |
| RGB 32 × 32 | 0.0534 | 0.0134 | 0.0138 | 0.3590 |
| RGB 256 × 256 | 0.2062 | 0.0318 | 0.0314 | 0.6035 |
| RGB 1024 × 768 | 2.5772 | 0.4844 | 0.5319 | 2.0311 |

CPU remains faster than Pillow on these four rows. SIMD reaches 5× only for
RGB 256 × 256 in this receipt; masked and tiny inputs remain below 5×, and the
large RGB row is just under 5×. GPU remains slower than SIMD on every row.
Across reruns, Pillow, CPU and GPU medians varied substantially, so the table is
diagnostic rather than stable throughput evidence. The equalize operation is
still incomplete; blockers remain the small/masked SIMD gap, GPU latency and
throughput relative to SIMD, and performance outside this cohort. The next
ranked operation remains `PIL.ImageFont.FreeTypeFont.font_variant`.

## Font variant checkpoint

The unchanged-policy whole-workflow baseline for
`pil-imagefont-freetypefont.font-variant.standard` includes constructing the
source font and its variant. Attempt 2 measured Pillow at 0.041084 ms and CPU
at 3.950000 ms. Its timing artifact is
`perf-font-variant-20260925-attempt2-benchmark.json`.

The behavior audit found that the Python wrapper reused the native handle's
original settings and bytes after public attributes or the source file changed.
The correction resolves the current public size/index/encoding/layout settings,
reopens path-backed sources, and preserves `font_bytes` for memory-backed sources.
Unreadable font paths now produce Pillow's `OSError("cannot open resource")`.
The live-reference check `scripts/test_font_variant_parity.py` passes all 18
source and state scenarios, including changed path contents, changed public
bytes, empty and short buffers, and independent source/variant sizing. The
eight maintained `font_variant` migration cases also pass on the rebuilt target.
The malformed-buffer mismatch was fixed in fontdone's driver-probe error
precedence: empty memory reports `Cannot_Open_Resource`, short unsupported
buffers report `Invalid_Stream_Operation`, and the final 17-byte BDF probe
reports `Invalid_File_Format`, matching Pillow's `cannot open resource`,
`invalid stream operation`, and `broken file` results. No assertion or threshold
was relaxed.

A first reuse attempt cloned parsed tables and rebuilt the public FFI face;
it left total latency near 3.95 ms because `face_to_ffi` repeated metadata and
handle construction. Attempt 3 instead cloned the existing face record and
rebuilt only its mutable size, stream, palette, and transform state. Median
CPU latency fell to 2.036750 ms; the variant phase fell from 1.966896 ms to
0.032771 ms. The total workflow is about 1.94× faster, while still about 50×
slower than Pillow. Receipt:
`perf-font-variant-20260925-attempt3-benchmark.json`.

Attempt 4 tested a one-entry per-thread parsed-face cache keyed by exact bytes
and face index. The benchmark remained 1.967375 ms with 1.941313 ms in setup,
so the cache did not establish a material end-to-end improvement and was
removed. Receipt: `perf-font-variant-20260925-attempt4-benchmark.json`. The
retained direct clone preserves independent `FT_Face` and `FontData` mutation
state; Rust regression tests verify size isolation and final driver-probe errors.

The current evidence still misses the CPU target: source-face setup costs about
1.94–2.00 ms against Pillow's 0.0225 ms. The variant operation itself is now
near 0.033 ms, so the remaining high-return investigation is the eager SFNT
face-opening path and its table parsing/copies. SIMD and GPU timings are near
CPU timings, but both receipts say backend execution is not proven; font loading
has no demonstrated SIMD or GPU path. No font-variant target is complete. The
next visit continues with the unresolved `PIL.ImageOps.invert` parity failures,
starting at the crop metadata loss and GPU autocontrast first divergence.

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

## Autocontrast four-attempt checkpoint — 2026-09-25

The next operation is `PIL.ImageOps.autocontrast`, selected from the ranked
image-kernel gaps after Composite. Before optimization, its cohort passed
187/187 CPU comparisons and 20/20 strict SIMD plus 20/20 strict GPU comparisons.
Each GPU parity run selected the GPU backend without fallback. The full focused
cohort spans L/RGB, cutoffs, masks and histogram edge cases. No expected value,
comparison threshold, or coverage target changed; no coverage was collected.

The baseline's receipt-backed materialized timings exposed two distinct costs.
The CPU histogram was a scalar per-pixel/per-channel dependency loop, while
the old GPU histogram kernel dispatched one workgroup whose active lanes each
scanned most of the image and atomically updated global bins. GPU launch,
transfer, wait and export costs also remained large after fixing that kernel.
At RGB 1024 × 768 the baseline medians were 2.635 ms Pillow, 1.597 ms CPU,
1.464 ms SIMD and 8.421 ms GPU. At RGB 256 × 256, CPU lost to Pillow
(0.309 vs 0.191 ms), while GPU took 1.807 ms.

The retained changes and bounded trials were:

1. **Retained native CPU/SIMD histogram construction.** For unmasked native
   L/RGB input, use the existing parity-tested per-channel histogram path and
   preserve the original mask-aware generic path. This removed scalar channel
   bookkeeping and reused the banked histogram implementation. It cut the
   1024 × 768 RGB CPU/SIMD rows to 0.896/0.816 ms in the first post-change run.
   The exact binary64 LUT formula, clipping and channel cutoffs remain unchanged.
2. **Retained parallel GPU histogram.** Reuse the equalize shader's private
   workgroup histograms and grid-stride input distribution instead of the
   one-workgroup, full-image scan. Keep the reduction, cutoff and remap stages
   separate so workgroups never depend on an in-dispatch global barrier. This
   changed the general GPU route from a serial scan to four native passes and
   passed strict GPU parity 20/20.
3. **Retained one-dispatch path for fresh unmasked L/RGB.** Derive the same
   exact LUT from host-visible native bytes, lower the operation to the existing
   GPU LUT remap, and reduce four dispatches to one. This path requires finite
   cutoff, nonempty L/RGB bytes, and no mask. Masked and unsupported layouts
   keep the general four-pass GPU path. Strict GPU parity remained 20/20; a
   40,320-output fresh-input run matched Pillow byte-for-byte, recorded exactly
   one GPU dispatch per request, and reported no fallbacks.
4. **Rejected SIMD lookup-selection tree.** Replace 15 repeated high-nibble
   equality selects with a four-level bit-selection tree around the same 16
   nibble tables. Strict SIMD parity passed 20/20, but receipt-backed RGB
   1024 × 768 latency changed from 0.741 to 0.772 ms and queue-four throughput
   moved only about 0–3% across L/RGB. The table-swizzle count did not change;
   this trial was reverted. The locked `wide` crate also lacks the newer
   multi-vector shuffle API, and this crate rejects unsafe intrinsics, so no
   dependency or unsafe-code change was justified by the measured headroom.

The best retained materialized run is attempt 3. Its median milliseconds are:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB 256 × 256 | 0.259 | 0.202 | 0.110 | 0.454 |
| RGB 1024 × 768 | 2.828 | 1.052 | 0.741 | 1.712 |

CPU is faster than Pillow on these two rows, but the small-to-large crossover
and every supported mode/shape still need broader confirmation. SIMD is only
2.35× Pillow at 256 × 256 and 3.82× at 1024 × 768, below 5×. GPU remains 4.13×
slower than SIMD at 256 × 256 and 2.31× slower at 1024 × 768.

The same fresh 1024 × 768 L/RGB workload uses 16 changing frames per window,
queue depths 1/2/4, five warmups and five samples of 20 measured windows.
The 40,320 exact outputs include 38,400 timed requests. Source and runtime
hashes were stable. Median completed fresh requests per second were:

| Mode | Queue | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1 | 1,630 | 2,271 | 2,652 | 487 |
| L | 2 | 2,731 | 3,745 | 4,127 | 717 |
| L | 4 | 3,942 | 4,941 | 5,635 | 959 |
| RGB | 1 | 336 | 830 | 908 | 488 |
| RGB | 2 | 583 | 1,208 | 1,378 | 774 |
| RGB | 4 | 861 | 1,487 | 1,742 | 1,071 |

The host-derived LUT saved isolated GPU latency, but it did not make GPU
throughput catch SIMD: queue-four GPU remained 5.9× behind SIMD for L and 1.6×
behind for RGB. Against attempt 2's four-pass route, attempt 3 raised L
queue-four rate by 7% and lowered RGB by 11%. This mode-to-mode split and small
run-to-run changes mean dispatch count alone does not predict completed
throughput. The host histogram is now on the GPU request's critical path; a
future attempt should attribute host histogram, submission, completion and
readback separately before changing the route again.

Evidence is retained under `build/migration-parity/perf-autocontrast-20260925-`.
The focused parity cohort is exact on CPU/SIMD/GPU. The checkpoint is incomplete:
SIMD misses 5×, GPU misses SIMD latency and throughput, and CPU requires small,
masked and broader mode/shape verification. Continue with the next ranked
untouched operation; revisit Autocontrast only when profiling can target its
measured LUT or GPU host/device bottleneck.

## ImageOps CropBorder four-attempt checkpoint — 2026-09-25

`PIL.ImageOps.crop` lowers to `PipelineOp::CropBorder`. Its eight-workload
baseline showed CPU latency 0.46–1.05 ms on 1024 × 768 L/LA/RGB/RGBA, SIMD
0.09–0.44 ms, and GPU 1.12–1.76 ms. Inspection separated the costs: CPU's
`DynamicImage::crop_imm` traversed generic pixels; SIMD manually loaded and
stored every 16-byte block; GPU uploaded a four-byte-per-pixel RGBA expansion
even for L/LA/RGB, then synchronously mapped a 2.2 MB output. Small requests
were already near the Python-call floor.

Four attempts are checkpointed. Exact Pillow parity remains fixed:

1. **Retained CPU native-row copy.** For L/LA/RGB/RGBA byte buffers, compute
   checked source and output strides and copy complete cropped row spans. Keep
   `crop_imm` for typed and floating-point images. Use division-based
   width-first then height validation so `2 * border` cannot wrap and preserve
   Pillow's exact-half empty result. This removes generic per-pixel access on
   native layouts.
2. **Retained SIMD native slice copy.** Replace the hand-built `wide::u8x16`
   load/store loop with Rust's optimized slice copy for each row. Crop is data
   movement, not arithmetic; manual vector construction and per-block array
   extraction added instructions around a copy the compiler/runtime can lower
   to native memory movement.
3. **Retained uninitialized-capacity output construction.** Reserve the exact
   validated byte length and append each copied row. Both paths previously
   zero-filled every destination byte and immediately overwrote it. The output
   is fully initialized by the checked row loop before reconstruction; no
   padding reaches the result.
4. **Retained GPU adjacent-border fusion.** Sum only contiguous CropBorder
   descriptors with checked addition before resource allocation. Do not cross
   another operation. The two-border RGBA chain now dispatches once instead of
   twice and copies only the final region; its GPU median moved from 1.243 to
   1.047 ms in the paired diagnostic runs (about 16%).

Focused exact parity passes **15/15 CPU**, **15/15 strict SIMD**, and **15/15
strict GPU** on the final implementations. A GPU fusion unit test compares
ordered operations with the lowered plan, including an intervening Invert. The
unchanged eight-workload benchmark completes on every attempt. Attempt 2's
ordinary rerun varied 1.2–2.6× even for Pillow; its low-priority utility
repeat also slowed all subjects and is not used for a performance claim. Use
the baseline and final receipt as diagnostic bounds rather than treating their
small medians as stable estimates.

The baseline and final attempt-4 median milliseconds for large native modes
are:

| Workload | Pillow baseline | CPU baseline | SIMD baseline | GPU baseline | Pillow final | CPU final | SIMD final | GPU final |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L 1024 × 768 | 0.127 | 0.462 | 0.094 | 1.681 | 0.092 | 0.037 | 0.036 | 1.537 |
| LA 1024 × 768 | 0.717 | 0.900 | 0.179 | 1.764 | 0.493 | 0.110 | 0.136 | 1.940 |
| RGB 1024 × 768 | 0.813 | 1.050 | 0.315 | 1.260 | 0.679 | 0.229 | 0.213 | 1.240 |
| RGBA 1024 × 768 | 0.892 | 1.052 | 0.436 | 1.124 | 0.697 | 0.322 | 0.301 | 1.125 |
| Two successive RGBA crops | 0.919 | 2.126 | 0.593 | 1.120 | 1.073 | 0.418 | 0.408 | 1.047 |

The retained CPU path is faster than Pillow on the four large single-crop rows
and on the chain. The small 32 × 24 materialized CPU row remains slightly
slower (0.0116 vs 0.0111 ms), within the observed run noise but not proven to
meet the bound. SIMD is faster than Pillow on large rows, but reaches only
2.3–3.6× on final medians, below the 5× target. GPU continues to execute
without fallback, but single-crop latency is 1.12–1.94 ms and remains 3.7–42×
slower than SIMD depending on mode. Its 3,145,728-byte upload and 2,293,760-byte
readback stay four-channel even for native L/LA/RGB. The chain fusion saves one
dispatch, not the fixed submission/map/output cost. This byte-moving operation
has no queue-depth changing-input throughput evidence; reciprocal request
latency is not a sustained-throughput result.

Artifacts use `build/migration-parity/perf-imageops-crop-20260925-`. Core and
GPU-feature checks, the focused fusion unit test, release `make build-parity`,
and `cargo fmt --all -- --check` pass. No coverage was run. The next visit
should move to a different ranked operation. Revisit CropBorder only with new
evidence for the small-call CPU gap, a native-channel GPU transfer/readback
path, or a complete changing-input throughput workload.

## ImageOps Invert four-attempt checkpoint — 2026-09-25

`PIL.ImageOps.invert` has 84 focused parity fixtures. Before editing, all 84
passed on strict CPU, SIMD, and GPU lanes. The old global priority row
`pipeline-chain.long-point.invert-1` has one sample and no warmup, so its
237× GPU factor was not a trustworthy baseline. Focused release measurements
used the same 1024 × 768 RGB single-operation workload and required an actual
backend receipt.

Four attempts:

1. **Rejected CPU zero-filled parallel destination.** Mapping source bytes into
   `vec![0; len]` removed the source clone, but added a complete zero-fill pass.
   Its first measured CPU result was noisy and did not show a reliable whole-
   request gain; do not treat destination allocation alone as copy elimination.
2. **Retained exact-size CPU collection.** Collect `raw.iter().map(|v| 255 - v)`
   directly into the output. This avoids both the full input clone and the
   separate in-place inversion read/write pass. Two focused runs moved CPU
   backend median from 0.231 ms baseline to 0.161 and 0.166 ms. A later final
   sample was 0.103 ms; the whole request was 0.290 ms versus Pillow 1.327 ms
   in that run. Keep this only with the strict parity evidence below; report
   run variance rather than treating the lowest sample as a fixed speedup.
3. **Rejected SIMD block append.** Writing each transformed `u8x16` into an
   exact-capacity `Vec` removed the clone in source, but measured SIMD backend
   time stayed around 0.26 ms and later varied up to 0.29 ms. Per-block vector
   append bookkeeping erased the expected gain, so the SIMD path remains the
   previous clone-plus-in-place vector loop.
4. **Retained GPU native-byte lowering.** For L/LA/RGB/RGBA byte images,
   execute invert through the existing native bytewise Solarize shader at
   threshold zero in four-independent-samples mode. This preserves all stored
   bytes, including alpha, while avoiding RGBA expansion. The 1024 × 768 RGB
   receipt dropped upload and readback from 3,145,728 to 2,359,296 bytes each,
   mode conversions from one to zero, and uniform parameters from 256 to 20
   bytes. Dispatch count stays one. Final GPU latency was 0.677 ms versus
   1.758 ms baseline; the paired run still puts GPU 1.18× slower than SIMD
   (0.572 ms). Timing spread is material across runs.

Final parity passes **84/84 strict CPU**, **84/84 strict SIMD**, and **84/84
strict GPU**. The final focused release build completed; `cargo check` with the
GPU feature and `cargo fmt --all -- --check` pass. No coverage was run.

The operation is checkpointed, not complete. CPU meets its Pillow bound on the
measured large RGB case. SIMD is only 2.3× Pillow, below 5×. GPU latency still
exceeds SIMD, and no changing-input queue-depth benchmark proves higher GPU
throughput. Those gaps remain for a later revisit; useful next evidence is a
SIMD profile that isolates result materialization from the vector loop and a
changing-input GPU queue-depth run that records completed requests per second.
Receipts are under `build/migration-parity/perf-imageops-invert-20260925-*` and
`build/migration-parity/parity-imageops-invert-*-20260925.json`.

## RGB-to-LAB conversion checkpoint — 2026-09-25

`Image.convert("LAB")` previously rejected the destination mode, leaving 78
conversion comparisons failing on CPU, SIMD, and GPU. The core now carries the
33³ LittleCMS sRGB-to-LabV4 CLUT as read-only data, performs LittleCMS's exact
fixed-point coordinate mapping and tetrahedral interpolation, then packs the
same three-byte LAB storage as Pillow. An offline exhaustive comparison found
zero byte differences for all 16,777,216 RGB inputs. The output keeps the
`icc_profile` generated by Pillow's LAB conversion, including its per-call UTC
creation timestamp.

The GPU path has a dedicated `rgb_to_lab.wgsl` dispatch. It reads two packed
words per CLUT vertex, evaluates all six tetrahedron orderings with integer
arithmetic, and writes the packed result directly. A strict 17 × 3 RGB case
completed one GPU dispatch over two workgroups and passed exactly on a
same-second rerun. The broader strict GPU cohort recorded 18 GPU receipts, eight
CPU semantic fallbacks for empty images that cannot dispatch, and one expected
`La` conversion error before a pipeline receipt.

The 27-case strict GPU parity comparison reports 9 passes and 18 image
mismatches. Inspection of every mismatch found identical LAB pixel bytes, image
mode, and size; only ICC profile byte 35 differed, by one second in the
profile's creation timestamp. This timestamp comes from the live Pillow call,
so two correct source and target executions can cross a UTC second boundary.
That makes exact profile-byte comparison nondeterministic. This is a parity
fixture limitation, not a reason to remove the profile or change conversion
behavior. The original checkpoint left these mismatches visible. The follow-up
CI repair below documents the test-contract defect and compares the live ICC
creation field narrowly while retaining exact checks everywhere else.

A release diagnostic timed `convert("LAB")` plus `tobytes()` on one warmed,
preconstructed RGB image, using two warmups and seven samples per size. Each
sample repeated the operation enough times to reduce timer noise. Every subject
produced the same SHA-256 at each size. Medians and single-request throughput
were:

| Size | Pillow ms | CPU ms | SIMD ms | GPU ms | CPU MPix/s | SIMD MPix/s | GPU MPix/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 × 1 | 8.222 | 0.00295 | 0.00317 | 0.252 | 0.339 | 0.316 | 0.004 |
| 16 × 16 | 8.258 | 0.00528 | 0.00549 | 0.227 | 48.5 | 46.7 | 1.13 |
| 32 × 32 | 8.302 | 0.01276 | 0.01205 | 0.247 | 80.3 | 85.0 | 4.14 |
| 256 × 256 | 11.472 | 0.601 | 0.566 | 0.300 | 109.1 | 115.9 | 218.3 |
| 1024 × 768 | 47.164 | 6.923 | 6.862 | 1.472 | 113.6 | 114.6 | 534.1 |

These diagnostic samples put CPU below Pillow and SIMD above the 5× target at
all five sizes; the large-image SIMD ratio is 6.9×. The actual SIMD adapter is
still a scalar-control call into the exact scalar converter, so the measured
ratio does not prove a vectorized LAB kernel. GPU latency and throughput beat
SIMD only at 256 × 256 and above. At 1024 × 768, GPU latency is 4.7× lower and
single-request throughput is 4.7× higher than SIMD. At 32 × 32, GPU latency is
about 20× higher and throughput is about 20× lower. The GPU receipt confirms a
real dispatch, so this gap is dispatch, transfer, and readback cost rather than
CPU fallback. This benchmark repeats one input per sample; it does not prove
changing-input throughput under concurrent requests. The full diagnostic
record is `build/migration-parity/lab-size-benchmark.json`.

The focused Rust example test, all-feature `cargo check`, and release
`make build-parity` pass. No coverage collection ran. Checkpointed gaps are the
nondeterministic ICC timestamp comparison, a vectorized SIMD implementation,
small-image GPU routing or batching, and a concurrent changing-input GPU
throughput measurement. Revisit LAB only with evidence addressing one of those
gaps; the next first-pass operation remains the highest-ranked incomplete row.

## Grayscale SIMD blocker revisit — 2026-09-26

The bounded revisit selected `PIL.ImageOps.grayscale` because the earlier
checkpoint still had a large RGB SIMD gap. The SIMD adapter now admits logical
`RGBa` after applying Pillow's integer unpremultiplication, then runs the native
vector luma path. Its strict focused cohort passes 26/26 cases. CPU and GPU
grayscale lanes were unchanged and retain their earlier focused parity results.
An accidental broad selector also ran 1,737 ImageOps cases: 1,729 passed; the
eight failures are `solarize` empty-image status mismatches, outside this
operation.

The fixed serial SIMD path was rebuilt with `make build-parity`, passed
`cargo check -p pillow-rs --all-targets --all-features --locked`, and passed the
strict SIMD cohort. The six unchanged whole-workflow benchmarks all record six
executions on their requested CPU, SIMD, or GPU backend with no fallback. The
benchmark correctness gate is successful execution; strict output parity is
reported separately. Median milliseconds are:

| Grayscale workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB 1 × 1 | 0.0100 | 0.0108 | 0.0114 | 0.2898 |
| RGB 16 × 16 | 0.0128 | 0.0127 | 0.0126 | 0.2995 |
| RGB 32 × 24 | 0.0108 | 0.0114 | 0.0121 | 0.2919 |
| RGB 32 × 32 | 0.0108 | 0.0112 | 0.0119 | 0.3548 |
| RGB 256 × 256 | 0.0284 | 0.0291 | 0.0339 | 0.5570 |
| RGB 1024 × 768 | 0.4378 | 0.2176 | 0.2666 | 2.2034 |

These short samples vary: the preceding serial receipt measured 0.2610/0.1845/
0.3034/2.2327 ms on the largest row. Both runs show the same limits: CPU is
competitive on large RGB but loses on some small rows; SIMD does not approach
5× Pillow; and GPU is far slower than SIMD. In the latest large sample, SIMD
backend execution is 154.9 µs, while terminal byte export is 167.6 µs. GPU
backend execution is 2.086 ms, consistent with its measured readback/completion
cost. No changing-input throughput claim was collected in this revisit.

Two bounded kernel experiments were rejected. First, enabling
`par_rows_mut!` above 262,144 pixels raised the 1024 × 768 SIMD backend median
from 147.2 to 399.1 µs; the whole workflow rose from 0.3034 to 0.6681 ms.
The serial vector loop is the better path here, so the parallel branch was
removed. Second, an AArch64 `vld3q_u8` structure-load prototype could avoid the
portable RGB deinterleave shuffles, but compilation is rejected by the crate's
`-D unsafe-code` policy. No lint suppression or unsafe exception was added, and
the non-building prototype was removed. A future revisit needs a safe locked
API for structure loads or a measured safe shuffle/layout change, plus a
separate investigation of terminal export cost. The grayscale operation
remains incomplete; no coverage was run.

Receipts: `parity-imageops-grayscale-simd-20260926-checkpoint.json`,
`execution-imageops-grayscale-simd-20260926-checkpoint.json`, and
`perf-imageops-grayscale-20260926-checkpoint.json` under
`build/migration-parity/`. The next operation should come from the current
incomplete-operation ranking; this checkpoint does not clear the outstanding
CPU, SIMD, GPU, or throughput goals.

## Colorize four-attempt checkpoint — 2026-09-26

The selected operation was `PIL.ImageOps.colorize`. Its 43-case focused CPU,
SIMD, and GPU parity cohorts pass. No reference behavior or assertions were
changed. A separate deterministic size sweep (1 × 1, 16 × 16, 32 × 24,
256 × 256, and 1024 × 768; varied `L` bytes; two endpoints, a midpoint, and
non-default control points) also produced byte-identical RGB results between
Pillow 12.2.0 and the strict GPU backend at every size. All five GPU executions
used one real dispatch with no fallback.

Four bounded attempts established the useful limits:

1. The GPU shader previously performed three piecewise integer divisions for
   every pixel. It now uploads the exact CPU-built 256-entry RGB mapping as
   packed opaque words and performs one indexed read per pixel. This preserves
   the reference's floor division and clamp because the LUT is built by the
   already parity-tested mapping function. At 1024 × 768, the receipt still
   records a 3 MiB input upload, a 3 MiB readback, and one full-frame copy;
   public latency is therefore dominated by transport/completion rather than
   just the removed divisions. Repeated small official measurements did not
   show a stable GPU latency improvement.
2. The CPU path now borrows native `L` samples instead of cloning them through
   `to_luma8()`, and emits the final interleaved RGB buffer in one pass. Other
   modes retain the previous Pillow-compatible `to_luma8()` fallback. Strict
   CPU parity passes 43/43. In the seven-repeat 1024 × 768 diagnostic, backend
   time moved from 0.864 to 0.656 ms and whole-workflow time from 2.412 to
   2.259 ms. The three tiny official workloads are noisy and do not establish a
   consistent CPU gain.
3. SIMD's three 16-lane LUT results were originally repacked through four more
   vector shuffles and temporary blocks per input block. Writing those lanes
   directly improved the 256 × 256 diagnostic by 19% and the 1024 × 768 backend
   by 20%, but regressed the 16 × 16 diagnostic by about 9% and the small
   official cohort by roughly 2–5%.
4. The SIMD implementation now keeps the original interleave path below 65,536
   pixels and uses direct channel stores at or above that size. Strict SIMD
   parity passes 43/43. Seven-repeat medians for the large path moved from
   1.377 to 1.126 ms backend time and from 2.793 to 2.595 ms whole-workflow
   time at 1024 × 768; 256 × 256 moved from 0.252 to 0.213 ms whole-workflow.
   The thresholded path leaves tiny cases on the old kernel. The threshold is
   a measured crossover for this target, not a portable hardware guarantee.

The final three-workload official run measured 75–79 µs for Pillow, 18–21 µs
for CPU, 19–21 µs for SIMD, and 316–376 µs for GPU. These small-sample medians
varied across runs, but no backend lost Pillow parity. CPU remained faster than
Pillow, while SIMD reached only about 3.8–4.0× Pillow, below the 5× target. GPU
was about 17–19× slower than SIMD. At 1024 × 768, the last seven-repeat
diagnostic measured whole-workflow medians of 2.704 ms Pillow, 2.259 ms CPU,
2.595 ms SIMD, and 3.245 ms GPU. These are latency samples only; no
changing-input concurrent throughput run was made.

The measurements were collected on a MacBook Pro (Apple M3 Pro, 12 CPU cores
with six performance and six efficiency cores, 18-core GPU, 18 GB RAM) running
macOS 15.7.7. The oracle and target used Python 3.12.13 and Pillow 12.2.0; the
Rust release build used rustc 1.96.1 / LLVM 22.1.2, with `wgpu` 24 and `wide`
1.6.1. The official workloads were 16 × 16 `L`, 16 × 16 materialized `L`, and
32 × 24 `L`; each used five warmups, 20 measured iterations per sample, five
samples, warm cache, single-request concurrency, and the whole-workflow
boundary. The size sweep used seven repetitions per side and size, with no
concurrent request load. These numbers describe this machine and workload set.

The next SIMD revisit should first attack `native_lut_chunk`: each channel
currently lowers to sixteen byte swizzles and a serial select chain, three
times per block. Evaluate a safe wider-table lookup or an exact vectorized
piecewise formula, then inspect generated instructions and measure both sides
of the 65,536-pixel crossover. The next GPU revisit should measure packed
transport and host/device wait separately, retain the grayscale source layout
instead of expanding each source sample into packed words if the executor can
preserve exact RGB output, and test changing-input throughput at queue depth.
Do not spend another attempt on shader arithmetic until the 3 MiB upload and
readback path is reduced or a device-side timestamp proves arithmetic remains
the dominant cost. The Colorize CPU/Pillow objective is met, but the SIMD and
GPU goals remain open; move to the next ranked operation for its first pass.

Receipts and timing artifacts are under `build/migration-parity/`, including
`parity-imageops-colorize-cpu-20260926-onepass.json`,
`parity-imageops-colorize-simd-20260926-threshold.json`,
`parity-imageops-colorize-gpu-20260926-threshold.json`, and
`perf-imageops-colorize-20260926-threshold.json`. No coverage collection ran.

## Verify checkpoint — 2026-09-26

The focused `PIL.Image.Image.verify` cohort passes 25/25 on strict CPU. The
oracle distinguishes the generic `Image.verify()` no-op from file-backed
`ImageFile.verify()` behavior. The wrapper now records whether an image came
from `Image.open()` and delegates verification only for that encoded source;
new and derived images follow the generic no-op contract even when their Rust
representation is a deferred pipeline. This prevents `verify()` from forcing
the same deferred pipeline a second time. It preserves the file-backed path,
and no parity assertions or fixtures changed.

Two bounded attempts were enough to locate the useful boundary. A PyO3 fast
path that avoided releasing the GIL for already-loaded images did not produce
a measurable public-latency gain and was reverted. The provenance guard was
retained because it removes redundant work on derived pipelines and expresses
the source API's class behavior. The extension was rebuilt after reverting the
first attempt so the final measurements match the checked-in source.

The official whole-workflow run used five warmups, 20 iterations per sample,
five samples for the standard row; the three matrix rows used one warmup, three
iterations per sample, and two samples. Median latency is in microseconds:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| 16 × 16 RGB new + verify | 4.375 | 5.750 | 5.875 | 5.417 |
| matrix-030, two-operation chain | 17.979 | 20.208 | 22.313 | 335.855 |
| matrix-031, two-operation chain | 12.605 | 13.334 | 14.146 | 295.188 |
| matrix-034, three-operation chain | 19.188 | 22.604 | 23.500 | 592.042 |

Only the standard Verify workload has a parity-pass benchmark gate. Matrix rows
passed their successful-execution gates; the separate 25-case strict cohort is
the Verify parity evidence. The standard workflow still loses to Pillow on all
target backends. Phase medians attribute most of its deficit to setup: Pillow
spends 3.167 µs in setup versus 4.042 µs on CPU, while a direct method-only
measurement puts the wrapper's conditional no-op overhead at about 0.020 µs.
The next operation is therefore `PIL.Image.new`, which constructs that image;
further micro-optimizing the no-op method cannot recover the workflow gap.
GPU matrix latency is dominated by terminal dispatch/completion and is not a
Verify cost. These runs did not measure changing-input concurrent throughput.

Receipts are `build/migration-parity/parity-verify-20260926-final.json` and
`build/migration-parity/perf-image-verify-20260926-final.json`. The final
benchmark artifact passes schema validation, and `RUSTC_WRAPPER= make build-parity`
succeeds. No coverage collection ran.

## Image.new checkpoint — 2026-09-26

The first strict CPU cohort found two existing LAB constructor mismatches;
78/80 cases passed. Pillow accepts LAB as a three-band mode, including empty
images. Rust already stores LAB in its three-byte RGB raster representation but
`Image::new` did not admit LAB or retain its explicit logical mode. The core
constructor now uses that existing storage and keeps the `LAB` tag. The
unchanged 80-case cohort passes 80/80, including other nonempty color/mode
cases and both prior empty-size regressions. No parity case or assertion
changed.

The official `pil-image.new.standard` whole-workflow run passes its parity gate.
Median latency is 3.083 µs for Pillow, 3.459 µs for CPU, 3.250 µs for SIMD,
and 3.000 µs for GPU. The requested backend does not dispatch for this eager
constructor, so these SIMD/GPU rows are the same CPU-side operation and do not
represent accelerator throughput. The strict workflow adapter still reports
a CPU loss. In a separate uninstrumented direct-call diagnostic, the median
for omitted-color RGB construction was:

| Size | Pillow | CPU | CPU speedup |
| --- | ---: | ---: | ---: |
| 16 × 16 | 870.5 ns | 576.2 ns | 1.51× |
| 256 × 256 | 4.480 µs | 3.069 µs | 1.46× |
| 1024 × 768 | 131.932 µs | 95.823 µs | 1.38× |

Those direct measurements used seven warm samples per size with result
replacement inside the loop; they omit the parity adapter and leave pipeline
telemetry disabled, as normal runtime does. Strict per-step attribution found
2.708 µs for Pillow and 3.000 µs for the target. The target-only strict lock
walk was probing primitive mode and dimension arguments for `_rust_image`;
the generic walker now returns immediately for exact built-in scalars while
still traversing containers and locking image objects. This removes about
83 ns from that strict target step without changing any fixture or lock rule,
but does not resolve the official whole-workflow gap. The direct public-call
measurements show omitted-color RGB construction ahead of Pillow; they do not
account for every path in the official workload. Keep this distinction visible
and investigate the workflow boundary before attributing its remaining loss to
the constructor implementation.

The local default `make build-parity` fails before compilation because this
worktree's ignored `.cargo/config.toml` forces `sccache`, which returns
`Operation not permitted` while launching rustc. `RUSTC_WRAPPER= make build-parity`
compiles successfully. This is a local wrapper restriction; no project build
setting was changed. Receipts are
`build/migration-parity/parity-image-new-20260926-final.json` and
`build/migration-parity/perf-image-new-20260926-final.json`. No coverage ran.
The next known parity blocker is `PIL.Image.frombytes("LAB", ...)`.

## Image.frombytes checkpoint — 2026-09-26

The class-level constructor rejected LAB before checking buffer length because
`FromBytesMode` omitted it. LAB uses the existing three-byte RGB raster layout,
but must retain the explicit `LAB` mode tag so mode queries and raw byte export
remain exact. The decoder now treats LAB as three bytes per pixel and follows
the existing raw RGB-storage path. Empty LAB `frombytes` calls take the
constructor path, which was fixed in the preceding checkpoint. The authority
and generated manifest add LAB only for the module and instance `frombytes`
surfaces; the shared YAML mode anchor was moved without changing its old list,
so unrelated operation claims did not expand.

The first performance review found an avoidable second full-buffer copy for
mutable input: the Python shim converted `bytearray` to `bytes`, then Rust
cloned the borrowed slice into raster storage. Bytes and bytearrays now pass
through unchanged. PyO3's owned vector path transfers exact-sized buffers into
raw raster storage; borrowed immutable bytes still make their one required
owned copy. Surplus data is trimmed to the required pixel length, while packed
mode-1 and 16-bit endian conversion retain their necessary output allocation.
The pointer-identity unit test proves that an owned RGB vector becomes raster
storage directly. The exact parity case also confirms the returned image owns
its bytes and a mutable input remains unchanged.

The existing strict input cohort now contains **66 frombytes cases** across
module and instance calls, including LAB, empty width/height, short and trailing
buffers, and bytearray input. All 66 pass with CPU, SIMD, and GPU selected; the
constructor does not dispatch image work to those accelerators. The focused Rust
byte-export tests pass **5/5**, including the owned-buffer identity test. No
expectations were weakened.

The benchmark review caught a measurement defect while replacing the 1 × 1
standard input with a 1024 × 768 RGB bytearray throughput case. Its first builtin
fixture regenerated 2.3 MB of bytes inside every timed workflow, producing
roughly 97–100 ms samples dominated by Python test-data generation. That result
was discarded. The fixture now creates one deterministic bytearray per
subprocess and reuses it across timed calls. The corrected official workload
passes its parity gate and reports these median latencies:

| Subject | Latency | Throughput |
| --- | ---: | ---: |
| Pillow | 0.367 ms | 2,722 ops/s |
| CPU | 0.080 ms | 12,451 ops/s |
| SIMD selected | 0.106 ms | 9,415 ops/s |
| GPU selected | 0.132 ms | 7,593 ops/s |

CPU is **4.57× faster** than Pillow on this public throughput workload. The
execution receipt is `not_proven` for SIMD and GPU because constructors do not
produce accelerator work; those rows are host-side timing variation under
different selectors, not SIMD/GPU speedups. This operation is a CPU allocation
and copy, so the SIMD ≥5× and GPU throughput goals are not applicable without
changing image residency and paying transfer costs.

An uninstrumented direct-call check used 100 warmups and five timed batches per
size with caller data prepared outside the timer. It asserted exact output bytes
for both `bytes` and `bytearray`; mutable inputs remained unchanged. Median
latency and target speedup over Pillow were:

| Size | Input | Pillow | Target | Speedup |
| --- | --- | ---: | ---: | ---: |
| 16 × 16 | bytes | 2.041 µs | 1.047 µs | 1.95× |
| 16 × 16 | bytearray | 2.007 µs | 1.057 µs | 1.90× |
| 256 × 256 | bytes | 26.133 µs | 3.958 µs | 6.60× |
| 256 × 256 | bytearray | 27.878 µs | 3.557 µs | 7.84× |
| 1024 × 768 | bytes | 387.145 µs | 101.197 µs | 3.83× |
| 1024 × 768 | bytearray | 405.563 µs | 101.460 µs | 4.00× |
| 2048 × 2048 | bytes | 1.880 ms | 0.244 ms | 7.72× |
| 2048 × 2048 | bytearray | 1.875 ms | 0.287 ms | 6.52× |

The ordinary CPU path beats Pillow at all measured sizes. Tiny inputs are
dominated by wrapper/allocation cost; large inputs are dominated by storage
allocation and memory copy. Keep the no-extra-copy ownership path and the
throughput-sized fixture; do not add a SIMD/GPU path that only uploads and
reads back bytes. Receipts are
`build/migration-parity/parity-image-frombytes-20260926-throughput-cpu.json`,
`...-simd.json`, `...-gpu.json`, and
`build/migration-parity/perf-image-frombytes-20260926-corrected.json` with its
`-parity.json` gate. No coverage collection ran.

## ImageFont.truetype checkpoint — 2026-09-26

The baseline constructor workload measured 1.959 ms on CPU versus 22.17 µs
for Pillow. Profiling attributed about 86% of target time to eager creation of
fontdone's glyph-to-script map during FFI face construction. The map is not
observed by a newly opened font; it is needed when the autofitter property is
queried or glyph loading synchronizes that property state. A face-local
`OnceLock` now builds it at first use, preserving the returned map and later
glyph behavior while removing that unused constructor work. The first
correctness-gated CPU median fell to 23.94 µs. A second shared-ownership change
lets the adapter and parsed SFNT data retain the same immutable font-byte
allocation instead of copying it again; the measured 23.33 µs median was within
run-to-run noise and did not qualify as a standalone latency win.

The third attempt caches one small static SFNT source face per thread, capped
at 256 KiB. Reuse requires the exact source bytes and face index; every call
still creates an independent mutable face, and size/charmap state is reset by
the existing clone path. Variable fonts, CFF data, unsupported interpreter
state, large fonts, and different bytes fall back to normal opening. This
reduced the warm repeated-constructor CPU median to 20.25 µs; an unchanged-policy
repeat measured 19.83 µs. The first and repeat Pillow medians were 22.33 and
22.23 µs. The CPU path is therefore ahead of Pillow by 9–11% on these warm
samples, not by an order of magnitude. The two samples are diagnostics, not a
stable guarantee for every font or a cold one-off open.

The final build pinned to fontdone `80b8224293a50c41753a3f295bbc304e581bbd7c`
measured these medians:

| Selector | Latency | Reciprocal rate |
| --- | ---: | ---: |
| Pillow | 23.833 µs | 41,959 ops/s |
| CPU | 20.209 µs | 49,484 ops/s |
| SIMD | 21.062 µs | 47,479 ops/s |
| GPU | 26.625 µs | 37,559 ops/s |

This agrees with the checkpoint direction; the single run remains a diagnostic
sample. The target execution receipt is `not_proven` for each selector. The
rates are reciprocal single-request latency at concurrency one, not sustained
throughput.

The cache initially panicked at thread exit because the cached face's size
state destructor accessed a thread-local registry after that registry had
already been destroyed. `FT_Init_FreeType` now initializes the registry before
the adapter can initialize its cache TLS. Repeated-process benchmark completion
and the focused fontdone memory-face tests exercise the corrected lifetime.
Keep this initialization ordering if the cache or registry TLS changes.

The unchanged 15-case `truetype` parity cohort passes on CPU, SIMD, and GPU
configurations (45 comparisons total). The benchmark records `not_proven` for
all three target backends: font construction makes no SIMD or GPU dispatch.
Selector-specific timings therefore measure host-side noise, not acceleration;
the SIMD ≥5× and GPU-vs-SIMD objectives cannot be credited for this constructor.
The benchmark uses concurrency one and reciprocal latency for its operations
per second figure, so it does not establish sustained throughput. Keep the
operation checkpointed with that limitation rather than claiming those goals
met. Receipts are
`perf-font-truetype-20260926-source-cache.json` and
`perf-font-truetype-20260926-source-cache-repeat.json`, with their parity
sidecars, `perf-font-truetype-20260926-pinned.json` with its parity sidecar,
and `truetype-{cpu,simd,gpu}-pinned-parity.json`. No coverage collection ran.

The broad `cargo test --locked -p pillow-rs` run completed 238/255 tests and
failed 17 GPU execution/receipt assertions outside this constructor operation.
Rerunning those cases individually passed 12; five GPU-path assertions remain
failing in `cmyk_filtered_rotate_stays_on_exact_host_control`,
`f_resize_f64_wide_special_value_outputs_native`,
`f_resize_ordered_f64_subnormal_vertical_native_matches_cpu`,
`f_resize_compact_box_special_over_binding_native_matches_cpu`, and
`typed_luma16_nearest_affine_transform_uses_native_word_path`. Keep these as
separate GPU-operation blockers; do not relax their expected-backend checks to
make this constructor checkpoint look green.


## ImageFont.font_variant checkpoint — 2026-09-26

The four-attempt revisit found and retained one parity correction and three
bounded CPU-path changes. An empty public `font_bytes` value now maps to
Pillow's `OSError("cannot open resource")` at the Python adapter boundary;
fontdone keeps its FreeType-level `Invalid_Stream_Operation`. The live
reference test now passes 19 source/state scenarios, including empty and
mutable public bytes, and all eight maintained variant cases pass with CPU,
SIMD, and GPU selected (24 comparisons). No assertions or thresholds changed.

Attempt 1 constructs an eligible static SFNT variant directly instead of
cloning a `Font` and resetting its cloned face globals, bytecode context,
raster scratch, size state, and non-SFNT fields. It preserves SFNT names,
charmap, italic flag, and BDF strikes while creating fresh mutable face state.
Attempt 2 skips source-byte comparison only when slice pointer and length prove
that the candidate is the same live allocation; distinct allocations still
require exact byte equality. Attempt 3 avoids the `BytesIO` round trip for
already-immutable Python `bytes` while retaining the existing snapshot for
mutable byte-like input. Attempt 4 records the original memory-backed bytes
object and tells Rust to reuse its owned `Rc<Vec<u8>>` only while that public
attribute is still the identical object. The maintained standard workload is
path-backed, so the last two memory-source changes do not affect its hot path.

The official standard benchmark's medians show substantial run-to-run
variation. The baseline after the `truetype` checkpoint measured Pillow/CPU at
40.855/49.167 µs. Attempt 1 measured 40.959/42.084 µs, and its unchanged repeat
measured 42.042/40.916 µs. Attempt 3 measured 40.541/37.750 µs, but Attempt 4
measured 37.813/38.459 µs. Attempt 2 was a noisy outlier at 59.104 µs on CPU.
Every run passed its benchmark parity gate. The latest phase medians clarify
the unresolved operation-level gap:

| Attempt 4 phase | Pillow | CPU |
| --- | ---: | ---: |
| Source setup | 20.979 µs | 18.208 µs |
| `font_variant` call | 16.250 µs | 19.375 µs |
| Whole workflow | 37.813 µs | 38.459 µs |

Setup is faster, but the `font_variant` call remains 19.2% slower than Pillow;
the whole workflow is still 1.7% slower in the latest run. Attempt 3 also had a
slower CPU operation phase (19.021 versus 17.375 µs) despite winning its whole
workflow sample. The operation is therefore checkpointed incomplete. SIMD and
GPU execution receipts are `not_proven`; their selector timings do not prove
acceleration for a host-side font operation, and concurrency-one reciprocal
rates do not establish sustained throughput.

The next visit to this operation should avoid converting freshly read
path-backed bytes into an owned vector before checking them against the source
face. On exact equality, reuse parsed tables and only build independent mutable
face state; on mismatch, take one owned copy and open the new bytes. Then profile
the remaining parsed-table clone cost. This work is checkpointed after four
attempts so the next ranked operation can receive its first pass. Receipts are
`perf-font-variant-20260926-attempt{1,1b,2,3,4}.json` with matching parity
sidecars, `font-variant-attempt4-{cpu,simd,gpu}-parity.json`, and
`test_font_variant_parity.py`. No coverage collection ran.

## Pipeline operation inventory repair — 2026-09-26

The docs CI regression exposed two unrepresented `PipelineOp` variants. The
canonical inventory is 88 operations after collapsing the `BoxBlurXY` alias
and including five eager public operations. `ConvertLab` and `ResizeBoxed` had
no materialized workload specs, so a hard-coded 87-row docs assertion both
failed and concealed incomplete benchmark input. The workload builder now
maps `ConvertLab` to the existing RGB-to-LAB public parity case and
`ResizeBoxed` to the public `resize(box=...)` case. Regenerated inputs report
88/88 workloads, no missing specs, and no missing workload IDs; the docs test
checks all three conditions. This is input completeness evidence, not collected
coverage.

An execution-only smoke of the new boxed-resize row points to a substantial
backend gap. Its 16 × 16 RGB input resizes the `(0, 0, 8, 8)` box back to 16 ×
16. Pillow measured 14.250 µs; target CPU measured 127.708 µs. The target call
phase is 11.000 µs, while terminal materialization takes 105.625 µs. Explicit
SIMD produced no timing or execution receipt because `ResizeBoxed` is rejected
by SIMD preflight. The GPU request completed on CPU with the fallback reason
`exact host semantic control: no valid single-dispatch shader contract`, at
383.605 µs. This workload uses a `successful_execution` gate, so these are
routing and timing diagnostics, not parity-backed performance claims. The next
operation visit is `ResizeBoxed`: first reuse the existing boxed SIMD
coefficient/data-plane code used by `Fit` and `Thumbnail`, then prove exact
outputs before extending mode coverage. The smoke artifact is
`build/migration-parity/perf-new-pipeline-ops-20260926.json`.

## ResizeBoxed checkpoint — 2026-09-26

Four bounded attempts retained the SIMD boxed-resize route, a bounded all-zero
CPU/SIMD shortcut, native GPU dispatch through the shared horizontal/vertical
resize shaders, and vertical source-row compaction. The fourth change derives
the smallest source-row span referenced by the vertical coefficient table,
keeps the exact fixed-point weights, rebases only source-row indices, and
skips horizontal work outside that span. Recomputing coefficients from a
cropped float box was avoided because subtracting an integer from float32 box
coordinates can change Pillow-visible coefficient boundaries. GPU carries the
original first-row offset in the unused resize uniform word and dispatches only
the compact horizontal row count; its intermediate and vertical table use
zero-based rows.

Parity evidence is exact. The maintained `resize.parameter.box` case passes on
CPU, SIMD, and GPU. A nonuniform RGB center-crop workflow passes all six
resampling filters on each backend, with six actual-GPU receipts and no
fallback. A wider 54-case CPU and SIMD differential passes all six filters for
`1`, `P`, `L`, `LA`, `RGB`, `RGBA`, `RGBa`, `RGBX`, and `CMYK`. The existing
zero-input differential also remains exact. No fixture or assertion was
changed to improve these results, and no coverage ran.

End-to-end samples time `resize` through `tobytes` on deterministic nonuniform
RGB inputs. Each row is a seven-sample median from one local run; GPU and SIMD
are explicitly selected and execution receipts name the requested backend.
The full-box geometry is a fractional near-full extent; the crop geometry is
the central half of the source, both resized to three quarters of source
width and height.

| Geometry | Input | Pillow | CPU | SIMD | GPU | SIMD / Pillow | SIMD / GPU |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Near-full | 1024 × 768 | 5.24 ms | 2.08 ms | 1.93 ms | 2.98 ms | 2.71× | 0.65× |
| Near-full | 2048 × 1536 | 20.99 ms | 7.25 ms | 7.20 ms | 6.98 ms | 2.91× | 1.03× |
| Center half | 1024 × 768 | 2.75 ms | 1.37 ms | 1.24 ms | 2.67 ms | 2.23× | 0.46× |
| Center half | 2048 × 1536 | 11.78 ms | 4.96 ms | 3.78 ms | 5.14 ms | 3.12× | 0.74× |

The compact span improves the large crop case structurally, but this table has
no pre-change crop baseline. CPU beats Pillow in these nonuniform samples; the
tiny all-zero fixture remains within measurement noise and does not prove CPU
is never slower. SIMD reaches only 2.2–3.1× Pillow here, short of 5×. GPU is
faster than SIMD only on the single near-full 2048 × 1536 sample, by about 3%,
which is within local run-to-run noise; it loses on the other three. Thus the
SIMD and GPU goals remain open, and CPU parity/performance outside these tested
layouts is not established. Filtered straight-alpha GPU resizes still use
exact host control because the current two-pass dependency is not proven for
those modes.

The measured bottlenecks point to different next attacks. SIMD's reusable
boxed kernel still spends work in two full image passes and row scheduling;
profile horizontal taps, vertical memory access, and per-call coefficient
planning separately before choosing another data layout. GPU's remaining cost
is dominated by dispatch, packed-buffer transfer, and materialization at these
sizes; the two-pass shader throughput only reaches parity with SIMD near the
largest full-box sample. Separate device work from host upload/readback and
test a larger workload before claiming a throughput lead. Do not credit a CPU
fallback or host-computed pixels as native GPU acceleration. This `resize(box=...)`
subcase remains checkpointed as incomplete; the next visit should take the
next highest uncheckpointed operation in the refreshed matrix.

The release extension build, `make fmt clippy`, and `make docs-lint` pass. The
focused `cargo test --locked -p pillow-rs --lib resize` filter compiles and runs
65 existing tests: 51 pass and 14 fail on GPU backend/receipt assertions in
float, integer, 16-bit, and thumbnail resize cases. The changed RGB boxed path
passes strict GPU parity; an isolated ordinary GPU float-resize receipt test
also passes. Keep the broader typed/GPU receipt failures visible as separate
blockers rather than weakening their checks. No coverage collection ran.

## ImageFont.getname checkpoint — 2026-09-26

The Python binding originally copied both borrowed Rust names into temporary
`String`s before PyO3 created the returned Python strings. Attempt 1 created
the Python strings directly from the borrowed names. Attempt 2 passed the two
`Option<&str>` values straight to `PyTuple::new`, preserving `None` conversion
while removing the extra match/conversion layer. Attempt 2 is retained.

The live `getname.behavior.default` case passes exactly on CPU, SIMD, and GPU
(one comparison per selected backend). Repeated calls return equal tuple
values, while both Pillow and pillow-rs create a fresh tuple and fresh name
strings each time. Caching those Python objects would change observable
identity; font variation also changes the reported style name, so the binding
must read the current Rust name fields on each call.

Unprofiled 500-repeat call-phase measurements show the intended reduction:

| Version | Pillow call | pillow-rs call | pillow-rs setup + call |
| --- | ---: | ---: | ---: |
| Original binding | 1.00 µs | 1.58 µs | 29.46 µs |
| Attempt 1: direct borrowed strings | 1.00 µs | 1.29 µs | 28.71 µs |
| Attempt 2: direct tuple conversion | 1.00 µs | 1.17 µs | 24.04 µs |

The separate correctness-gated standard workload measured Pillow at 23.88 µs
and CPU at 22.12 µs after attempt 2, so the whole-workflow CPU target passed
that local sample. The getter call itself remains about 17% slower than Pillow.
SIMD measured 30.08 µs and GPU 21.42 µs in that run, but neither has an actual
backend receipt: returning two strings has no pixel data plane to dispatch.
Those selector timings do not establish SIMD acceleration or GPU throughput.
The sample is from a dirty worktree and remains diagnostic, not accepted
backend evidence.

Two measurement caveats remain. `profile_migration_benchmark.py --backend
pillow` currently prepends the target package path even for the source side;
it therefore imports pillow-rs `12.2.0-alpha.1` and fails the pinned Pillow
identity check. The call-phase comparison above used the isolated source
adapter with `PYTHONPATH` unset. Also, the pushed CI run `332568625` passed
documentation, formatting/clippy, parity build, and Windows type-check jobs,
but the supply-chain job failed and skipped downstream Python/WASM jobs. The
job logs were not accessible without GitHub sign-in. Local `cargo deny` checks
pass; local `cargo audit` exited successfully but reported a registry timeout
while checking whether `paste 1.0.15` is yanked, so the hosted failure cause
remains unresolved. No coverage collection ran.

This checkpoint is complete for the two safe binding changes; the global SIMD
and GPU goals remain inapplicable to this metadata-only call without changing
its execution contract. Move to the next uncheckpointed high-gap image
operation, `PIL.ImageOps.fit`; refresh its strict parity and workload timing
after the `ResizeBoxed` changes before editing it.

## LAB putpixel and build-recovery checkpoint — 2026-09-26

The CI failures exposed two distinct issues. First, logical LAB images use the
RGB8 raster as storage, but Pillow's public A/B bands are signed: storage adds
128 and `getpixel` subtracts 128. `Image.new("LAB", color)` and
`Image.putpixel` had omitted that storage bias. Both now apply it with wrapping
byte arithmetic at the public write boundary; raw `frombytes` data and the
LAB conversion kernel remain in their existing stored-byte domain. SIMD
support is limited to `PutPixel` over this layout, and the GPU accepts LAB only
for non-palette `PutPixel`, whose shader copies the already-normalized bytes.
The strict input-audit passes 33/33 LAB cases on CPU, SIMD, and GPU. The full
1,323-case `putpixel` parity selection passes on strict CPU; the RGB behavior,
mode, and value cases plus the 33 LAB cases pass 34/34 on both SIMD and GPU.

The no-GPU WASM build also exposed GPU-only helpers and a contrast mean field
that were compiled without their feature. The GPU table/imports, affine LUT
helper, and GPU-only contrast field are now gated on `gpu`; the GPU build still
uses them. Separately, the JS parity asset adapter lacked the fixed RGB
`frombytes` bytearray and throughput bytearray inputs. It now transports the
same fixture bytes as `Uint8Array` input; no case, expected output, or assertion
changed. The two targeted JS cases pass. The no-GPU core build completes.

Three bounded `putpixel` performance attempts retained two changes. The Python
RGB-integer path now reuses the mode it already resolved for argument parsing
instead of re-resolving it in the core value adapter. `PipelineOps::one` now
constructs its single operation node directly instead of allocating an empty
node and appending a second node. A special direct RGB `push_op` constructor
was rejected because its sample regressed. The unchanged 16 × 16 RGB scalar
workload measured Pillow/pillow-rs CPU whole-workflow medians of 7.792/9.188 µs
before these changes. The best retained-code run measured 7.375/7.834 µs; a
second standard-policy run measured 7.291/8.292 µs. These samples indicate
progress, but CPU is still slower than Pillow (about 6–14% across the two
retained-code runs), so `putpixel` is checkpointed with that explicit blocker.
The GPU and SIMD selectors have no native-execution receipt for this workload:
it measures lazy construction and stops before materialization. Their timings
do not prove accelerated `putpixel` execution or throughput. A background
`taskpolicy` run changed the scale and ordering of all subjects and is excluded
from the comparison. Standard benchmark and parity receipts are
`build/migration-parity/perf-putpixel-20260926-{baseline,attempt2,retained-final}.json`
and `build/migration-parity/parity-putpixel-20260926-{cpu,simd,gpu}.json`.
No coverage ran.

The LAB timestamp comparison defect and its scoped repair are recorded in the
CI checkpoint below. `ImageOps.fit` has since been checkpointed separately;
its remaining latency and throughput gaps stay open in that operation's table.

### WASM putpixel parity repair — 2026-09-26

The CI WASM cohort uncovered 468/1,325 `putpixel` mismatches even though the
native CPU cohort had passed. The pixel kernel was not the cause: the JS
workflow adapter had flattened Python inputs into JavaScript arrays/numbers,
then called the low-level WASM method with unvalidated unsigned coordinates.
That erased distinctions used by Pillow's wrapper (`tuple` versus `list`, and
`int` versus `float`), rejected valid negative pixel indices, accepted integers
outside `i64`, and checked the color before coordinates. Palette colors add a
second ordering rule: Pillow prepares/allocates a palette entry before checking
coordinates, while PA's explicit alpha conversion happens after the bounds
check.

The adapter now tags raw JSON arrays as Python tuples and explicit `list` and
`sequence` protocols with their corresponding types. It transports unsafe
Python integers as decimal strings decoded to `BigInt`, and float literals as
typed float markers, but only on `putpixel` arguments. Before dispatch it
reproduces Python's `__index__`/C-long/i32 coordinate checks, negative-index
normalization, bounds ordering, mode-specific tuple arity, and integer
conversion errors. For P/PA color values, the adapter invokes the existing
Rust palette preparation path at an impossible coordinate. This reuses the
canonical palette allocator and returns only its pre-write error or palette
side effect; the adapter then validates real coordinates and performs the
actual pixel write. Float-component palette inputs use a private typed marker
so integral-valued Python floats remain floats across the WASM boundary. Large
F-mode `getpixel` outputs are tagged as floats only at the observation
serializer, avoiding a global conversion of large JS numbers to Python floats.

After these changes, strict parity passes all 1,325 `putpixel` workflows on
Node WASM and all 1,325 in a real browser WASM host, with zero failed or
not-run cases. The 3 large-F-value cases also pass after narrowing the JSON type
marker to F-mode pixel observations. The JS package check and the focused LAB
byte-example unit test pass. No fixture, expected output, comparison policy,
coverage threshold, or runtime pixel kernel was changed. This repair restores
WASM behavior; it provides no new CPU latency result or SIMD/GPU execution
receipt. The earlier three bounded native performance attempts remain the
checkpoint: CPU is still 6–14% slower than Pillow in the recorded 16 × 16
whole-workflow samples, and the standard SIMD/GPU samples are lazy operations
without execution evidence. Move on to `PIL.ImageOps.fit` as already queued;
keep `putpixel` on the explicit latency blocker list.

## ImageOps.fit checkpoint — 2026-09-26

Four bounded performance attempts are checkpointed. Attempt 1 added the
SIMD-only all-zero byte-image shortcut before boxed coefficient construction;
the maintained black 32 × 24 → 16 × 16 workload showed SIMD whole-call median
falling from 138.6 µs to 14.5 µs, with exact output and an actual SIMD receipt.
Its terminal phase fell from 117.6 µs to 4.6 µs. Attempt 2 kept the same vector
kernel but ran Fit's small horizontal and vertical row sets serially below
1,024 output pixels. `ResizeBoxed` and `Thumbnail` pass a zero cutoff and keep
their prior scheduling. Attempt 3 applied that operation-scoped cutoff to CPU
Fit's scalar boxed-resize passes, again leaving general boxed-resize scheduling
unchanged. Attempt 4 admitted only L and opaque RGB to the existing native GPU
Fit path; straight-alpha and typed layouts remain on their exact host paths.

The parity fix found while testing the public API was that the target exported
resampling names as strings, while `ImageOps.fit` correctly rejects raw strings
to match Pillow. Consequently `Image.Resampling.BILINEAR` itself was unusable.
`Resampling` is now an `IntEnum` with Pillow's codes (NEAREST 0, LANCZOS 1,
BILINEAR 2, BICUBIC 3, BOX 4, HAMMING 5); legacy `*_INT` names remain aliases
to those values. The direct public `ImageOps.fit(...,
method=Image.Resampling.BILINEAR)` invocation now succeeds and returns exactly
the oracle bytes.

All 84 applicable Fit cases pass exact parity on CPU and SIMD. Strict GPU
parity passes the RGB bilinear Fit case, and patterned non-zero RGB inputs were
compared byte-for-byte across Pillow, CPU, SIMD, and GPU. A 101-repeat small
whole-call diagnostic (32 × 24 → 16 × 16) measured Pillow at 5.17 µs, CPU at
8.25 µs, SIMD at 8.75 µs, and actual GPU at 215.96 µs. GPU telemetry confirms
two device dispatches and no fallback. The small CPU and SIMD whole-call
latencies remain above Pillow.

Larger patterned cases show the scale-dependent ceiling. At 1,024 × 768 →
512 × 512, medians were Pillow 1,988 µs, CPU 1,148 µs, SIMD 790 µs, and GPU
1,893 µs. At 2,048 × 1,536 → 1,024 × 1,024, medians were Pillow 8,250 µs, CPU
3,973 µs, SIMD 3,038 µs, and GPU 6,424 µs. Every output hash matched. The GPU
executed natively in both cases but moved 3.1/12.6 MB to the device and read
back 1.0/4.2 MB across two dispatches. It remains about 2.1× slower than SIMD
at the larger size. SIMD reaches only about 2.7× Pillow there, short of the
5× goal. The maintained zero-filled 32 × 24 workload measured whole-workflow
medians of Pillow 17.83 µs, CPU 15.96 µs, SIMD 13.96 µs, and GPU 314.77 µs;
GPU has an actual-device receipt, but this zero-input row is not representative
of resampling arithmetic.

Fit is checkpointed after four performance attempts. Remaining blockers are
small-call CPU/SIMD overhead, SIMD throughput below 5× Pillow, and GPU host
transfer/readback plus two-pass dispatch cost. Revisit GPU only with a design
that reduces staging or fuses passes; do not expand straight-alpha admission
without strict device parity. The retained evidence is in
`build/migration-parity/fit-*20260926.json` and the direct patterned-input
measurements recorded above. No coverage ran.

The first CI run after [`30eb32f`](https://github.com/appunni-m/pillow-rs/actions/runs/36227483462)
reported 27 LAB conversion cases in each Python lane and 570 failures in each
WASM lane. The WASM reports included those LAB cases plus resize argument
parity failures. Rust, documentation, Windows type-check, and parity-build jobs
passed. The following checkpoint records the fixes and focused reruns.

### LAB ICC timestamp and WASM resize CI repairs — 2026-09-26

The LAB conversion annotations were caused by two independent Python processes
serially invoking Pillow and pillow-rs across a UTC-second boundary. Pillow's
canonical 572-byte LittleCMS LAB profile stamps the current UTC time into ICC
`dateTimeNumber` at bytes 24–35. That is live creation metadata; requiring
separate invocations to emit the same 12 bytes was an invalid parity-test
contract. This is a genuine oracle-comparison defect, not an output mismatch.

The image comparator now permits variation only for that field when both
records are LAB images with the canonical profile header, valid calendar
timestamps, and timestamps no more than one day apart. It then compares the
complete image records after clearing only those 12 bytes. Pixel bytes, mode,
size, all other metadata, the full ICC profile outside the clock field, and
materialized byte observations remain exact. Unit tests verify that pixel
changes, any other profile-byte change, invalid dates, and unrelated times
still fail. No fixture, expected output, or conversion output was changed.

The WASM resize failures exposed a real binding gap: resize boxes were narrowed
to integers and the JS method called the core API that discarded
`reducing_gap`. The binding now carries box coordinates as floats and forwards
`reducing_gap` to `resize_with_options`, matching the core's existing option
handling. The focused all-mode LAB cohort passes 28/28 on Python CPU, Node
WASM, and browser WASM. The full 837-case `Image.resize` cohort also passes on
Node and browser WASM after the binding fix. No coverage ran.

### ImageEnhance and solarize WASM CI repairs — 2026-09-26

The 154 WASM parity failures split into 54 Color/Contrast enhancer failures
and 100 `ImageOps.solarize` failures. The enhancer workflow adapter represented
Color and Contrast as stateless descriptors and called one-shot methods at
`enhance()`. Pillow constructs and retains each degenerate base earlier:
Color retains its converted grayscale image (except L/LA, where it aliases the
source), and Contrast retains its rounded-mean image. Recomputing after source
mutation changed the result. The descriptor also skipped constructor-time
conversion errors for `La` and `RGBa`. The WASM `Image` binding now exposes the
existing core base constructors, and the adapter retains that base and blends
it with the live source. All 572 active Color/Contrast enhance workflows pass
in Node and browser WASM; the focused pre-fix run had 54 failures.

Solarize's WASM adapter narrowed thresholds to `u8`, which wrapped negative
values, truncated fractional values, and mishandled values above 255. It also
used nullish coalescing, turning an explicit Python `None` into the default
threshold and checking unsupported image modes before raising threshold
comparison errors. The adapter now computes the exact 256 results of
`sample < threshold` in the host and sends that table through the existing
core `SolarizeInput::Comparisons` path. The core applies mode validation only
after those comparisons, preserving Pillow's error order. All 392 active
`ImageOps.solarize` workflows pass in both hosts.

After both repairs, `make test-wasm` passed its npm package check and all
16,771 parity workflows on Node WASM and all 16,771 on browser WASM, with zero
failures, skipped cases, or infrastructure errors. The campaign build required
`RUSTC_WRAPPER=` because the configured local `sccache` could not start under
this host's permissions. No fixtures, expected outputs, or comparison rules
were changed, and no coverage ran.

## MedianFilter 3 × 3 checkpoint — 2026-09-26

Three bounded attempts optimized `PIL.Image.Image.filter` with
`MedianFilter(3)`. CPU strict parity passes all 756 Filter workflows; the
dedicated GPU shader passes all 13 selected size-3 median workflows across
byte modes and the F-mode control. No expected output or comparison rule was
changed.

The first CPU change recognizes uniform native-byte images for every rank, not
only min/max, and returns a clone before allocating the output. Every rank of a
uniform image is the same image, so the bounded byte scan removes the window
gather and order-statistic selection entirely while preserving mode. On the
maintained zero-filled 16 × 16 workload, whole-workflow CPU median fell from
141.73 to 16.62 µs (8.53×); the 32 × 32 sample fell from 80.77 to 19.85 µs
(4.07×). These official inputs are constant images and overstate the benefit
for ordinary content.

The second CPU change specializes size 3 for native byte layouts. Each output
channel gathers exactly nine clamped samples into a nine-byte stack array and
selects the requested rank, rather than initializing the general 7 × 7
scratch array. This preserves replicated-edge semantics. In a separate
patterned-input diagnostic, CPU medians changed from 46.02 to 51.50 µs at
16 × 16, 48.92 to 41.83 µs at 32 × 32, 1,036.69 to 439.38 µs at 256 × 256,
and 7,334.13 to 6,371.25 µs at 1,024 × 768. The 16 × 16 result regressed and
remains slower than Pillow; at 32 × 32 CPU also remains slower than Pillow.
SIMD's patterned-input measurements were 34.62, 38.96, 466.25, and 4,760.58 µs
for those sizes. SIMD still misses the 5× target at small and medium sizes.

The third change routes non-F/I size-3 GPU median operations to a compact
shader. The old general shader reserves four 225-sample channel arrays per
invocation and insertion-sorts them; the new shader sorts nine packed samples
with a fixed compare-exchange network. The first shader draft used WGSL's
reserved identifier `pass`; renaming it to `phase` fixed shader validation,
with no behavior or parity relaxation. On the same patterned diagnostic, GPU
latency fell from 982.42 to 176.88 µs at 16 × 16, 1,467.69 to 180.58 µs at
32 × 32, 4,923.31 to 422.90 µs at 256 × 256, and 31,647.17 to 2,411.42 µs at
1,024 × 768 (5.6×–13.1× faster). The 16 × 16 and 32 × 32 GPU paths are still
launch-bound; at 256 × 256 GPU is approximately level with SIMD, and at
1,024 × 768 it is about 2.0× faster than SIMD in this diagnostic. The
maintained whole-workflow row reports about 1.6×, but uses a constant input.

The unchanged-policy whole-workflow receipt
`build/migration-parity/perf-filter-median-3x3-optimized-20260926.json` records
the following medians in microseconds, compared with
`build/migration-parity/perf-filter-20260926.json`:

| Workload | Pillow baseline → current | CPU baseline → current | SIMD baseline → current | GPU baseline → current |
| --- | ---: | ---: | ---: | ---: |
| RGB 1 × 1 | 16.40 → 19.81 | 13.77 → 16.25 | 13.90 → 17.62 | 299.44 → 632.19 |
| RGB 32 × 32 | 67.23 → 67.60 | 80.77 → 19.85 | 18.38 → 19.29 | 403.81 → 322.25 |
| RGB 256 × 256 | 3,370.94 → 3,163.21 | 320.96 → 216.62 | 457.48 → 518.94 | 3,562.69 → 527.56 |
| RGB 1,024 × 768 | 36,733 → 39,671 | 2,882 → 1,926 | 4,119 → 5,342 | 21,960 → 3,348 |
| MedianFilter materialized 16 × 16 | 27.60 → 30.58 | 141.73 → 16.62 | 14.71 → 16.08 | 530.92 → 282.54 |

The official filter generator uses constant-zero images, so the first four rows
do not measure median-selection throughput on varied pixels. The direct
patterned diagnostics are separate and do not include the official workflow's
instrumentation or establish sustained request throughput. Rerun variance also
affects the unrelated 1 × 1 row; do not interpret that row as a filter-kernel
change.

Checkpoint blockers: patterned 16 × 16 CPU is slower than Pillow; SIMD remains
below 5× Pillow for small and medium
inputs; GPU loses to SIMD for small inputs and is only approximately tied at
256 × 256. Add representative patterned benchmark inputs before using the
official cohort to rank this operation. The next revisit should first attribute
small-call fixed costs, then test a selection network or specialized 3×3
median kernel and measure the GPU transfer/dispatch crossover. This operation
is checkpointed, not complete. No coverage ran.

## Image.Image.getchannel checkpoint — 2026-09-26

Four bounded performance attempts are checkpointed. The first changed CPU
`L`/`RGB` extraction to allocate the one-byte result directly. RGB no longer
expands into a full RGBA temporary before discarding three of every four
bytes. A prior 1,024 × 768 RGB sample moved from CPU 0.248 ms against Pillow
0.180 ms to CPU 0.045 ms. The direct `L` path also copies only the logical
`width × height` samples, preserving Pillow's output length if the source
buffer contains trailing data.

The SIMD work exposed the cost of interleaved channel extraction. An explicit
five-pixel RGB shuffle rebuilt padded vectors and spilled selected lanes
through scalar arrays; it measured 1.259 ms for RGB, 0.514 ms for LA, and
0.719 ms for RGBA at 1,024 × 768. Replacing it with channel-constant strided
gathers reduced RGB to 0.345 ms. Parallelizing independent rows above 262,144
pixels then reduced the three large final samples to about 0.166–0.169 ms.
The fourth attempt tried a 16-pixel shuffle and regressed all three large
layouts, so that SIMD code was removed. The retained SIMD path is the direct
gather/parallel-gather implementation; the small and large samples show that
it still needs a genuinely faster lane-selection kernel to meet the 5× target.
The gathers do not use explicit wide-vector operations, so their receipts now
record zero vector blocks and identify scalar versus parallel gather work.
Backend receipts prove the SIMD route was selected without fallback; they do
not prove that this particular kernel emitted SIMD instructions.

The GPU attempt reads each packed output pixel's first byte directly into the
returned `L` image, avoiding a second full-frame RGBA-to-L host copy. The
shader still uses packed RGBA working storage: a 1,024 × 768 run uploads
3,145,728 bytes and reads back 3,145,728 bytes for a 786,432-byte result.
Consequently it executes natively but remains dominated by dispatch,
synchronization, and full-frame transfers. Reducing transfer width requires a
native one-byte GPU working/output path or fusing extraction with an adjacent
GPU producer; another host-side repack cannot remove that bottleneck.

The original standard workload timed only lazy `getchannel()` construction,
then materialized outside the clock, so it did not measure the operation's
pixel work. The generated standard case now times both the call and
`tobytes()`. Three additional 1,024 × 768 RGB, LA, and RGBA cases use that same
materialized boundary. Receipt
`migration-benchmark-af6462b5496a4f679896a39489121616` reports these median
latencies in milliseconds after correcting vector-path telemetry:

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| RGB 16 × 16 | 0.004917 | 0.005959 | 0.006333 | 0.208625 |
| RGB 1,024 × 768 | 0.184792 | 0.051083 | 0.184834 | 0.945855 |
| LA 1,024 × 768 | 0.158917 | 0.093855 | 0.189209 | 1.247813 |
| RGBA 1,024 × 768 | 0.194459 | 0.105437 | 0.216979 | 0.911334 |

These are five-sample medians with 100 timed executions per subject. Receipts
show the requested native CPU, SIMD, and GPU backend for every sample and no
fallback. The small CPU row is 1.21× Pillow latency. At large sizes CPU is
1.7–3.6× faster than Pillow. SIMD throughput is only 0.84–1.00× Pillow,
far below 5×. GPU latency is 4.2–6.6× SIMD latency on large cases and about
33× on the tiny case; reciprocal-latency throughput is correspondingly lower.
Sustained changing-input throughput has not been measured.

All 134 focused `getchannel` parity cases pass exactly on strict CPU, SIMD, and
GPU (402 comparisons total). The four materialized benchmark cases also pass
exact parity on each backend. `make fmt clippy` and both focused Rust
`extract_band` tests pass. No coverage ran. `getchannel` remains incomplete:
small-call CPU overhead, SIMD channel-gather throughput, and packed GPU
transfers are explicit blockers for a later revisit.

## Image.Image.getcolors checkpoint — 2026-09-26

Four bounded implementation attempts are checkpointed. First, histogram,
scalar, and multiband scans stop as soon as the `(maxcolors + 1)`th distinct
value appears; this avoids scanning the remaining pixels when Pillow will
return `None`. Second, multiband and scalar maps reserve only the bounded
working set for `maxcolors <= 256` on images of at least 64 KiB. This avoids
growth during common large-image rejection, but the measured operation still
misses CPU parity. Third, skipping PyO3's GIL detach for small resident images
showed no stable gain and was removed. Fourth, an early uniform-image scan for
tiny RGB/LA/RGBA images was removed: a second pass over a varied 16 × 16 input
still cost about 31 µs against Pillow's 10 µs.

Parity inspection found that Pillow's multiband result order follows occupied
slots in `ImagingGetColors`' hash table, despite the public documentation
describing the list as unsorted. Its table size, packed pixel key, and probe
sequence are observable through result order. LA keys replicate luminance into
the first three packed bytes and put alpha in the fourth; `I` and `F` preserve
raw 32-bit sample keys, including distinct positive and negative zero bit
patterns. The implementation now matches those rules rather than sorting or
using first-seen order. Negative `maxcolors` returns `None`; a requested table
beyond Pillow's supported size raises `MemoryError`. Thirty-six focused input
cases cover these contracts, mode variants, high cardinality, and the table
limit. The pinned [Pillow 12.2.0 `GetBBox.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/GetBBox.c),
[`Unpack.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c),
and [public `getcolors` documentation](https://pillow.readthedocs.io/en/12.2.0/reference/Image.html#PIL.Image.Image.getcolors)
are the behavior references.

All 36 focused cases pass against Pillow under CPU, SIMD, and GPU profile
labels (108 comparisons), and Node/WASM passes all 36 cases as well. The Node
run exposed and fixed an unsigned JS `maxcolors` parameter that converted `-1`
into a large positive limit, so the combined parity evidence is 144 comparisons.
Each of the three measured workloads also passes
its exact-output gate. `perf-getcolors-final.json` records five-sample medians
of 100 calls per subject. Focused parity receipts are
`getcolors-final-cpu.json`, `getcolors-final-simd.json`,
`getcolors-final-gpu.json`, and `getcolors-final-node.json` under
`build/migration-parity/`:

| Workload | Pillow | CPU | SIMD profile | GPU profile |
| --- | ---: | ---: | ---: | ---: |
| RGB 16 × 16, uniform | 0.001959 ms | 0.003625 ms | 0.003417 ms | 0.003500 ms |
| RGB 16 × 16, varied | 0.010083 ms | 0.031521 ms | 0.030625 ms | 0.030500 ms |
| RGB 1,024 × 768, high cardinality | 0.004500 ms | 0.007146 ms | 0.007376 ms | 0.007437 ms |

The CPU path remains 1.59–3.13× slower than Pillow on these rows. `getcolors`
is an eager host-side API; receipts show `actual_backend: null` for all three
target profile labels, so the SIMD and GPU rows prove neither SIMD execution
nor GPU acceleration. The high-cardinality early exit limits pixel visits, but
still costs more than Pillow's C loop. The varied 16 × 16 case must count all
256 colors and build the output. Code inspection leaves the main revisit
candidates: the Rust `HashMap` performs a second hash/probe to represent
Pillow's already computed table slots; native `.pixels()` iteration adds
per-pixel wrapper work; and each returned multiband color is copied into a
small `Vec` before PyO3 builds Python tuples. These are hypotheses from the
current path, not profile attribution. A revisit should measure scan, slot
lookup, and Python result construction separately before changing the table
representation or return layout.

`getcolors` remains incomplete after four attempts. No coverage collection ran.
The generated status pages now use the refreshed manifest and report three
stale historical aggregate artifacts with no compatible project-wide evidence;
the focused receipts above are recorded here but are not ingested into that
aggregate. The manifest refresh records the current enum defaults for resize,
rotate, and thumbnail and does not change runtime behavior.
The next unvisited high-ranked workload is `PIL.ImageFont.FreeTypeFont`, the
direct font-class constructor; work moves there after this checkpoint.

## FreeTypeFont constructor checkpoint — 2026-09-26

The focused parity case `PIL.ImageFont.FreeTypeFont.behavior.default` passes.
Its benchmark workload is `pil-imagefont.freetypefont.standard`, which loads
the 17,572-byte `DejaVuSans.ttf` fixture by path and measures the constructor
workflow. The fresh baseline receipt is
`build/migration-parity/perf-freetypefont-baseline.json`; its exact-output
preflight is `perf-freetypefont-baseline-parity.json`.

| Subject | Median latency | Backend receipt |
| --- | ---: | --- |
| Pillow | 0.022542 ms | Pillow |
| CPU | 0.020688 ms | not proven |
| SIMD profile | 0.020312 ms | not proven |
| GPU profile | 0.028125 ms | not proven |

The CPU observation is about 8% faster than Pillow for this single warm
default-font input. The SIMD and GPU labels do not execute image kernels here:
all target receipts have `actual_backend: null`. Loading a path, parsing font
tables, and creating an independent mutable face are host-side work; sending
those bytes through a GPU would add transfer and dispatch cost without
accelerating the operation. SIMD timing is likewise not proof of SIMD code.
This operation therefore has no eligible SIMD/GPU optimization target, and
the one CPU sample does not cover other font sizes, formats, or source types.

The hot path already uses a thread-local parsed-source cache for fonts up to
256 KiB. It still reads the path and verifies the new bytes before creating a
separate face, preserving visibility of file changes and mutable per-font
state. The fixture fits under that limit, so enlarging the cache would not
help this workload. The diagnostic `cProfile` run was dominated by loading
the complete YAML manifest and Python startup; profiler-instrumented timings
are excluded from the baseline. No code change is justified by this evidence.

The constructor is checkpointed with its backend-eligibility blocker recorded.

## FreeTypeFont.getmetrics checkpoint — 2026-09-26

The original `pil-imagefont-freetypefont.getmetrics.standard` row timed the
whole workflow, so it included opening the font before calling `getmetrics`.
Its initial medians (Pillow 0.031604 ms, CPU 0.022042 ms) mostly reflected
constructor setup and could not rank the getter. This was a benchmark-boundary
defect, not a parity failure or a library speedup. The input generator now
measures only the observed `call` step after `setup-font-1`; it does not change
the font, call arguments, expected output, or correctness gate.

The corrected workload passes exact-output preflight. Across two fresh runs,
the adapter-level medians are:

| Subject | Run 1 | Run 2 | Backend receipt |
| --- | ---: | ---: | --- |
| Pillow | 0.000959 ms | 0.000958 ms | Pillow |
| CPU | 0.001250 ms | 0.001250 ms | not proven |
| SIMD profile | 0.001166 ms | 0.001209 ms | not proven |
| GPU profile | 0.001166 ms | 0.001084 ms | not proven |

Every target receipt has `actual_backend: null`; this scalar metadata getter
does not execute an image kernel, so SIMD and GPU are ineligible. The official
step timer also includes parity-adapter operation lookup and argument
resolution. For this sub-microsecond method those costs dominate and reverse
the direct call ordering. A sequential, warmed diagnostic with nine samples
of one million direct public calls per subject records 36.13 ns for pillow-rs
and 41.81 ns for Pillow; outputs are `(19, 5)` on both. The detailed samples
are retained in
`build/migration-parity/getmetrics-direct-timeit-20260926.json`. Treat this as
a cross-check, not as the official acceptance artifact or a sustained
throughput claim.

No runtime change is justified: the direct call is already faster, while
changing tuple identity or caching Python integer objects to improve the
adapter-level number would risk observable behavior. The benchmark-boundary
repair and this limitation are recorded rather than masking them with a new
threshold.

## ImageFont.truetype checkpoint — 2026-09-26

The focused `pil-imagefont.truetype.standard` workload passes its exact-output
preflight. It constructs the default-size font from the same path in each
implementation. The fresh adapter-boundary receipt is
`build/migration-parity/perf-im-font-truetype-baseline.json`:

| Subject | Median latency | Backend receipt |
| --- | ---: | --- |
| Pillow | 0.022667 ms | Pillow |
| CPU | 0.020958 ms | not proven |
| SIMD profile | 0.028813 ms | not proven |
| GPU profile | 0.020896 ms | not proven |

The public `ImageFont.truetype` path is about 8% faster on CPU in this one
warm path-based case. A separate sequential, warmed direct-call diagnostic
measured medians of 15.53 µs for pillow-rs and 18.77 µs for Pillow, with the
same `(10, 3)` metrics result. Its samples are in
`build/migration-parity/truetype-direct-timeit-20260926.json`. This is a
cross-check only: the adapter receipt remains the official benchmark, and
neither measurement establishes results for other font sources or sizes.

All target receipts have `actual_backend: null`. This API reads and parses a
font on the host and returns a font object; no pixel kernel runs, so these
SIMD/GPU profile timings are not backend evidence. The two host-side font
entry points already use the parsed-source cache, while preserving path
change visibility and independent face state. No runtime change is justified
by these measurements.

## ImageFont.TransposedFont checkpoint — 2026-09-26

The standard workload originally measured font loading and wrapper
construction together. Its generated measurement boundary now times only the
`call` step after `setup-font-1`, keeping `FreeTypeFont` construction out of
this operation's latency. The focused default and explicit-orientation
parity cases pass before and after the code change.

Two bounded implementation attempts were evaluated. The retained change
assigns `None` directly for the default orientation instead of crossing the
PyO3 boundary to call a Rust normalizer whose `None` branch returns `None`
without validation or side effects. Explicit orientations still use the same
Rust normalization and error path. The second attempt stopped storing an
instance `_orientation_name` for `None`; it produced no timing improvement and
was reverted to preserve the existing instance state.

The official observed-step benchmark medians were:

| Run | Pillow | CPU | SIMD profile | GPU profile |
| --- | ---: | ---: | ---: | ---: |
| Baseline 1 | 0.001791 ms | 0.002500 ms | 0.002416 ms | 0.002417 ms |
| Baseline 2 | 0.001917 ms | 0.002438 ms | 0.002250 ms | 0.002542 ms |
| Retained attempt | 0.001792 ms | 0.002250 ms | 0.002375 ms | 0.002291 ms |

The target profile rows have no actual-backend receipt: constructing a Python
font wrapper performs no image kernel, so SIMD and GPU are ineligible. The
official adapter boundary still places CPU about 26% above Pillow. A separate
sequential direct-call diagnostic, with the font loaded before timing, gives
medians of 99.69 ns for pillow-rs and 106.29 ns for Pillow across seven
200,000-call samples. The samples are in
`build/migration-parity/transposedfont-direct-timeit-final-20260926.json`.
This direct measurement is a cross-check, not an acceptance artifact; because
it disagrees with the official adapter result, the CPU goal stays open.

The constructor operation is checkpointed after two attempts with the
benchmark-boundary and backend-eligibility blockers recorded. The next ranked
font function is `PIL.ImageFont.TransposedFont.getlength`.

## ImageFont.TransposedFont.getlength checkpoint — 2026-09-26

The original observed-step measurement included `FreeTypeFont` and wrapper
construction, so its 2.027 ms CPU median was not a `getlength` measurement.
The generated workload now observes only the `call` step after font setup.
The fixture's arguments and correctness oracle are unchanged. The corrected
baseline was Pillow 45.458 µs versus pillow-rs CPU 2,026.687 µs; focused
parity passes for default, positional and keyword arguments, and the rotated
font error.

The first hot-path cost was fontdone's lazy public glyph-to-script map. Every
ordinary glyph load forced the complete Unicode script-range map to be built
and copied into per-face autohint state, even though default hinted TrueType
loads did not use it. The retained fontdone change defers this synchronization
until the map property has been explicitly exposed or x-height has been
configured. Explicit map and x-height behavior keep the full synchronization
path. This reduced the corrected cold operation median to 35.354 µs while
passing the focused map, x-height, default-load, and getlength parity cases.

The next avoidable cost was loading each glyph into an FFI slot and copying its
outline even though BASIC `getlength` uses only the advance. The retained
length-only path requests the same hinted advance with `FT_Get_Advance`, then
applies the same per-pair kerning and 26.6 rounding in the same glyph order.
On a warmed, same-font direct-call diagnostic this changed the median from
15.472 µs to 13.560 µs; Pillow measured 6.180 µs. The benefit was real but not
enough for repeated calls.

The third attempt adds a bounded per-font cache of codepoint-to-glyph,
hinted-advance, and pair-kerning results. It stores at most 512 entries in
each map; variation setters clear the size-dependent advance and kerning
entries before changing the active variation. Glyph lookup entries remain
valid because changing variation coordinates does not change the selected
charmap. Cache invalidation was checked locally with the variable SFNS system
font: after warming the default width, changing axes produced lengths 90.0,
35.0, and 145.0 in both implementations. This system-font check is a local
diagnostic, not a portable fixture. The warmed direct-call median is now
0.479 µs for pillow-rs versus 6.180 µs for Pillow, with both returning 63.0.
This is 12.9× faster on that repeated-input path.

The final official correctness-gated workload also passes. Its observed-step
medians are 44.958 µs for Pillow and 27.000 µs for CPU, about 1.67× the
throughput. This official measurement remains the acceptance result; the
direct-call number isolates repeated same-font cache hits. The CPU is faster
than Pillow in both measurements. The SIMD and GPU profile rows have
`actual_backend: null`: character lookup and font metrics are scalar host-side
work, so this operation cannot provide SIMD/GPU acceleration evidence.

One measurement lesson is to rebuild the installed extension after every Rust
edit before running either parity or benchmarks. A run made before rebuilding
after the advance-only edit still used the prior binary and is excluded from
the evidence above. Do not rank from a workload that includes constructor
setup, and do not use requested SIMD/GPU profiles as proof when their backend
receipt is null. This operation is checkpointed after three implementation
attempts; its CPU parity/performance target is met, with SIMD/GPU applicability
recorded as a blocker. Continue to the next ranked operation.

## ImageFont.TransposedFont.getbbox checkpoint — 2026-09-26

The existing standard workload included both font loading and
`TransposedFont` construction. Its generated boundary now observes only the
`call` step after setup. The default, positional and keyword argument, and
rotated-orientation parity cases pass (4/4). The correctness-gated benchmark
then measured medians of 52.791 µs for Pillow and 33.458 µs for CPU, with
29,888 versus 18,942 operations per second. CPU latency is already below
Pillow, so no runtime optimization was justified for this operation.

The SIMD and GPU profile rows report `actual_backend: null`; the method
computes font bounds on the host and dispatches no pixel kernel. Record SIMD
and GPU as inapplicable here, not as accelerator performance. This is a
baseline checkpoint with no runtime attempts. The next high-ranked font
operation is `PIL.ImageFont.FreeTypeFont.getlength`; the recent shared BASIC
layout change affects it, so refresh its own isolated workload before deciding
whether it needs another implementation attempt.

## FreeTypeFont.set_variation_by_axes four-attempt checkpoint — 2026-09-26

The generated workload isolates the `call` step after font construction and
sets the variable font axes to `[100.0, 600.0]`. Each benchmark replay creates
a fresh default font, so the timed operation changes coordinates. This
distinction exposed why the repeated-coordinate short circuit alone was not a
benchmark optimization: it applies only after a face already has those exact
effective coordinates. The six Python behavior cases and the fontdone
FreeType setter lane remain the parity contract; the latter compared 711
concrete cases against FreeType 2.14.3.

The first attempt detects FreeType's no-change condition after normalized
coordinates exist and returns without rebuilding the face. It checks both the
fully default-filled design coordinate vector and the public variation flag;
partial input and explicit-default semantics therefore remain distinct. A
unit test confirms that repeating a valid setter call keeps the same parsed
face allocation. This helps repeated calls on one face, but the isolated
changed-coordinate workload measured 10.708 µs CPU against 3.834 µs Pillow,
effectively unchanged from its 10.542 µs baseline.

The second attempt reuses parsed SFNT tables for eligible glyf-based variable
faces. It clones the current parsed table model, replaces design and
normalized coordinates, clears coordinate-dependent glyph outlines, and
recomputes active face metrics and autohint state. Unsupported face shapes
continue through the original full parser. The 711-case FreeType parity lane
passed; CPU fell to 8.229 µs.

The third attempt updates the existing SFNT `FT_Face` fields and active size
record in place instead of calling `face_to_ffi`, which reparsed public table
wrappers and replaced size handles. Face identity changes still use the full
refresh path. The same parity lane passed and CPU fell to 6.854 µs.

The fourth attempt converts the common one-to-eight axis fixed-coordinate
input through stack storage, retaining checked heap conversion for larger
inputs. It shaved a further 0.146 µs; the oracle lane again passed. The final
correctness-gated medians were Pillow 4.000 µs and CPU 6.708 µs, so the
operation remains 1.68× slower than Pillow and is checkpointed as a blocker
after the agreed four attempts. SIMD and GPU profile timings are not evidence:
the font setter runs on the host and both accelerator receipts are null.

The remaining known cost is independent mutable variation state. Cloning
`FontData` still copies its parsed table vectors, and rebuilding
`FaceGlobals` remains necessary to bind autohint state to the updated face.
The next meaningful design is to share immutable parsed tables across font
instances while keeping normalized coordinates, glyph caches, size metrics,
and face globals isolated per face. Do not cache or share a mutable varied face
across instances, and do not turn repeated-coordinate timing into a substitute
for the changed-coordinate workload. Reopen this blocker when that ownership
split can be made without changing MVAR metrics, glyph output, named-instance
behavior, or independent-face semantics. The next operation should be selected
from the corrected latency ranking, not by extension of this setter work.

The Windows binding check caught that FreeType `FT_Long` follows native
`c_long` width: 32 bits on Windows and 64 bits on LP64 systems. Keep variation
flag conversion in a width shared by both targets before widening it back to
`FT_Long`; the exact Windows Python binding type-check then passes.

## FreeTypeFont.set_variation_by_name checkpoint — 2026-09-27

The benchmark now measures only the setter call after constructing its variable
font. The previous whole-workflow boundary hid setter costs in setup. Four
setter and six related `get_variation_names` parity cases pass on the rebuilt
target, including malformed instance arrays, non-English name fallback, Type 1
multiple-master behavior, an unknown instance, and named selection. No parity
assertion or benchmark budget changed.

The pre-optimization call-only warm median was 13.417 µs for CPU versus
7.583 µs for Pillow. The first retained change in fontdone reuses the parsed
variable-font tables and existing face record for named-instance selection;
this lowered CPU to 10.271 µs. A per-font name cache alone did not improve the
benchmark because each sample opens a fresh font. The static source-face cache
also deliberately excludes variable faces: their mutable coordinate state
must not be shared.

Attempt three added a separate one-entry, thread-local cache for the
immutable, duplicate-filtered name list, keyed by exact font bytes and
collection-face index. This preserves independent variation coordinates while
letting subsequent handles reuse names. Its correctness-gated warm medians
were 7.021 µs for CPU and 7.542 µs for Pillow. SIMD/GPU profiles have
`actual_backend: null`; named-font metadata and FreeType state are host-side,
so those profiles do not establish accelerator performance.

A cold-process diagnostic then showed the first setter call at 27.042 µs for
CPU versus 24.625 µs for Pillow. Attempt four populates the immutable names
while opening an SFNT variable face, swallowing parse failure there so malformed
font errors remain deferred to the public query or setter. Cold setter medians
then became 20.708 µs CPU and 24.167 µs Pillow. Cold whole-workflow medians were
0.731 ms CPU and 6.650 ms Pillow; the pre-attempt-four sample was 0.685/6.578
ms. These small whole-workflow differences are noisy, so the evidence supports
faster isolated setter latency, not an end-to-end workflow speedup. The names
are now prepared during font setup, and the workload intentionally reports
call-only timing so setup cost is visible as a separate boundary.

The final unchanged warm policy measured 7.292 µs CPU and 7.583 µs Pillow
(receipt `perf-freetypefont-set-variation-by-name-attempt4-20260927.json`).
The correctness-gated cold call receipt is
`perf-freetypefont-set-variation-by-name-cold-attempt4-20260927.json`; cold
whole-workflow diagnostics are in
`perf-freetypefont-set-variation-by-name-whole-cold-attempt4-20260927.json`.

The CPU setter meets the Pillow latency target in both measured cache states.
The 5× SIMD target and GPU comparison are inapplicable to this host-only font
operation and remain unproven by backend receipts. Checkpoint this operation
after four attempts and continue with the next unresolved operation from the
corrected latency ranking.

The later shared metadata cache also preserves the setter win: in the joint
axes/names/setter receipt, its warm call median was 6.146 µs CPU versus 7.500 µs
Pillow.

## FreeTypeFont.get_variation_names checkpoint — 2026-09-27

The workload now measures only `get_variation_names` after opening the
variable font. Its input and parity gate are unchanged. The six getter cases
and four named-setter cases passed together against live Pillow. The final
warm call-only medians were 2.000 µs CPU and 5.959 µs Pillow in the baseline
receipt `perf-freetypefont-get-variation-names-baseline-20260927.json`. After
sharing parsed variation metadata with the axes getter, a joint receipt measured
1.917 µs CPU and 6.041 µs Pillow for this call.

This method already benefits from the immutable name list prepared by the
preceding setter optimization, so no additional runtime attempt was needed.
SIMD/GPU report `actual_backend: null`; enumerating font names is host metadata
work, not an image kernel. CPU meets the latency target; accelerator targets
remain inapplicable or unproven. Continue to the next unresolved operation in
the corrected ranking.

## FreeTypeFont.get_variation_axes checkpoint — 2026-09-27

The benchmark isolates the axes query after opening a variable font. The five
existing parity cases pass for default and named variable fonts, malformed
axis metadata, and Type 1 behavior. Two unchanged-policy baseline receipts
measured CPU at 4.000 and 4.334 µs against Pillow at 3.917 and 3.750 µs, a
small but repeatable CPU latency miss. Profiling samples identified
`variation_tables` and `parse_name`: each query reparsed `fvar` and `name` even
though variable-font construction had already decoded those tables to prepare
named-instance metadata.

Attempt one stores Pillow-shaped axis descriptors beside the existing
duplicate-filtered instance names. One exact-byte and collection-face cache
entry shares only these immutable values; each font keeps its own mutable
variation coordinates. The metadata is parsed once when opening a variable
SFNT face, and each axes query clones the small result vector. All 15 axes,
name, and named-setter parity cases pass. In the correctness-gated joint
receipt `perf-font-variation-metadata-attempt1-20260927.json`, call-only medians
are 1.792 µs CPU versus 4.125 µs Pillow for axes, and 1.917 µs versus 6.041 µs
for names. The setter remains faster than Pillow at 6.146 versus 7.500 µs.

SIMD/GPU report `actual_backend: null`; these methods inspect font metadata and
do not dispatch image kernels. CPU latency targets pass for axes and names;
accelerator targets are inapplicable here. The shared table parse removes the
second `fvar`/`name` parse from workflows that query axes after opening the
font. Continue with the next unresolved operation in the corrected ranking.

## FreeTypeFont.getmask2 checkpoint — 2026-09-27

The benchmark now times only the `getmask2` call after font construction. The
41-case parity slice passes on the rebuilt target, including empty and space
inputs, fractional and negative starts, stroke, anchors, multiline text,
monochrome masks, embedded color strikes, and error behavior.

Before the change, `getmask2_with_start` rendered pixels through
`mask_from_run_with_start`, then called `getbbox(font, text)` to obtain its
returned offset. Both paths performed `glyph_run` shaping and computed the
same BASIC-mode bounds. Attempt one returns that already-computed bbox from
the rendering helper, so mask pixels, dimensions, and offset still derive from
one glyph run. No raster rules, rounding, or public API behavior changed.

The original call-only baseline measured 82.292 µs CPU versus 85.230 µs for
Pillow. After removing the duplicate layout, two correctness-gated repeats
measured 61.500/61.562 µs CPU and 60.271/60.541 µs under the requested SIMD
profile; Pillow measured 80.583 µs in both repeats. This is a stable ~25% CPU
latency reduction from the original target and 1.31× lower latency than Pillow
in the final repeat. Receipts are
`perf-freetypefont-getmask2-baseline-20260927.json`,
`perf-freetypefont-getmask2-attempt1-20260927.json`, and
`perf-freetypefont-getmask2-attempt2-20260927.json`.

The GPU-profile median was 60.520 µs in the final repeat, but both SIMD and GPU
receipts have `actual_backend: null`: this is host-side font shaping and
FreeType rasterization, not an accelerated image kernel. Those figures do not
prove SIMD/GPU throughput or parity on those backends. CPU parity and latency
targets pass, so checkpoint this operation after one implementation attempt
and continue to the next unresolved operation in the corrected ranking.

## FreeTypeFont.getmask checkpoint — 2026-09-27

The standard workload now measures `getmask` after opening its font. Its
previous whole-workflow boundary included font construction and obscured the
method cost. All 26 getmask parity cases pass on CPU, covering default and
optional arguments, byte text, starts, strokes, empty/space text, monochrome
masks, and embedded bitmap strikes.

Two correctness-gated call-only runs measured CPU at 59.709 and 59.541 µs;
Pillow measured 80.146 and 80.042 µs. CPU latency is about 25.7% lower, with
median throughput of 16,748 and 16,795 operations/second versus 12,477 and
12,493 for Pillow. The receipts are
`perf-freetypefont-getmask-baseline-20260927.json` and
`perf-freetypefont-getmask-repeat-20260927.json`.

Code inspection found no duplicated layout pass on this default path: the
renderer shapes once, computes bounds to size its canvas, and rasterizes that
run. The SIMD and GPU profile receipts have `actual_backend: null`; font
shaping and FreeType rasterization stay on the host. No runtime change is
justified by the measured CPU result, and the profile labels do not prove
accelerator execution. Checkpoint getmask with CPU parity/latency met and
SIMD/GPU applicability unproven. The next highest unresolved blocker is
`PIL.ImageFont.FreeTypeFont.font_variant`; its previous checkpoint identifies
path-backed byte ownership and parsed-table cloning as the next costs to
measure.

## FreeTypeFont.font_variant follow-up — 2026-09-27

The follow-up shared the immutable `glyf` and `loca` byte buffers across static
font variants instead of deep-copying them with `FontData::clone`. All eight
maintained `font_variant` parity cases and the live wrapper parity script pass.
The correctness-gated call-phase median was 20.854 µs, compared with 20.626 µs
in the immediately preceding baseline; the whole-workflow CPU medians were
40.187 and 40.730 µs. The call phase did not improve, so the candidate was
removed. SIMD/GPU receipts remain `actual_backend: null` for this host-side
font operation.

The earlier 2,345-sample profile counted 640 samples in OS file opens, 282 in
file reads, 258 in independent-face cloning, and 92 in memory moves inside
`FontData::clone`. Sharing only the large outline byte buffers attacks a small
part of that profile. Reopening a path observes file changes, so a metadata-only
skip would weaken that behavior. Keep `font_variant` blocked until a path-read
optimization preserves changed-file semantics and shows a call-phase win.

## ImageChops.overlay GPU shader checkpoint — 2026-09-27

The selected 1024 × 768 RGB workflow has a fresh successful-execution-gated baseline of
3.171 ms for Pillow, 0.737 ms for CPU, 0.685 ms for SIMD, and 1.288 ms for GPU.
CPU, SIMD, and GPU all completed on their requested backends without fallback.
Attempt four removes the per-channel dynamic branch and source-level integer
division: it selects the low or complemented operands first, then applies the
exact bounded quotient `(n + (n >> 7) + (n >> 14)) >> 7` with `n = product + 1`.
The selected operand is at most 127, so the product is at most 32,385; an
exhaustive check of all 32,386 possible products found no quotient mismatch.
All 174 maintained Overlay cases pass strict GPU parity.

Two correctness-gated repeats measured GPU at 1.264 and 1.183 ms, compared
with 1.288 ms before the change. Their median GPU execution-phase timings were
918,709 and 878,083 ns, against 921,709 ns at baseline. These are modest gains
within a noisy host-bound workflow, not evidence that GPU has caught SIMD. The
repeated GPU throughputs were 794 and 845 workflows/second, still below SIMD at
1,363 and 1,554. The GPU receipt retains 2,359,296 bytes of image upload,
2,359,296 auxiliary input bytes, and 2,359,296 readback bytes per request; the
baseline terminal phase alone was 1.043 ms. Keep the exact shader change, but
checkpoint Overlay as incomplete. The main blocker is transfer, mapping, and
completion latency; the current row remains about 1.8× slower than SIMD.

The first targeted pipeline benchmark exposed a selector bug: `--pipeline`
discarded `--workload-id` and ran 614 pipeline workloads. That output is retained
as `perf-benchmark-pipeline-selection-bug-20260927.json` and is excluded from
Overlay measurements. The runner now composes the filters, and a regression
test checks single-workload selection and rejects a non-pipeline ID under the
pipeline filter. The corrected command selects exactly one workload. No
coverage collection ran.

## ImageFont.load_default checkpoint — 2026-09-27

The refreshed single-workload receipt
`perf-load-default-attempt0-20260927.json` passes its exact-output gate; its
parity sidecar records one passing comparison. The warm default-font workflow
measured 28.750 µs for Pillow and 7.250 µs for CPU, so CPU latency is about 4×
lower on this case. This replaces the stale 2026-09-24 ranking observation,
which reported 0.773 ms for the target and did not prove requested-backend
execution. The current implementation opens the embedded 12,676-byte Aileron
font through the ordinary font path, which already has an exact-byte cached
source face. No profile attributes the remaining call cost, and this one warm
case does not justify another cache or a change to font construction.

The SIMD and GPU profiles report `actual_backend: null`: loading a font is
host-side object construction, not an image kernel. Their profile times do not
demonstrate SIMD or GPU performance. Keep those accelerator goals inapplicable
for this operation; CPU parity and latency pass for the default case. The
non-default size parameter remains a separate parity case, not a timed
workload. No runtime change was made; continue to the next uncheckpointed
operation using refreshed, single-workload evidence. No coverage collection
ran.

## PIL.ImageFont.load_default_imagefont checkpoint — 2026-09-27

The warm baseline measured 425.708 µs for pillow-rs versus 31.000 µs for
Pillow. Native samples placed roughly half of the hot stacks in the ordinary
glyph-image materialization and PNG/DEFLATE path; every call decoded the same
embedded `courb08.png`, then parsed and copied its fixed glyph data. The first
change put a parsed default `PilFont` in `OnceLock` and made its immutable glyph
table, bitmap, and info shareable through `Arc`. That cut the warm call to
1.459 µs, but a fresh target process still paid 482.375 µs for its first PNG
decode. A warm-only benchmark would have hidden this cost.

The next change stores the exact 798 × 20 decoded luma pixels in
`pillow-rs/src/font/courb08.luma` and initializes the parsed default font
directly from them. The 15,960-byte asset was produced with pillow-rs's own
PNG path. A focused Rust test decodes the checked-in PNG through that normal
path and requires its mode, dimensions, and every luma byte to match the
embedded asset; its SHA-256 is
`ff78f1b6d98ed40932e795d3786d0154fb1c2b874c191e3c662676c615846742`. This keeps
the production first call off the general decoder without using Pillow as a
runtime dependency or weakening behavior checks.

Two correctness-gated warm runs measured CPU at 1.458 and 1.500 µs, while
Pillow measured 31.250 and 31.688 µs. That is about 21× lower CPU latency, with
666,667–685,871 calls/second versus 31,558–32,000 for Pillow. Fresh-process
first-call diagnostics measured 15.6–16.9 µs for pillow-rs and 10.187 ms for
Pillow. The public parity case confirms construction, the decoded-asset test
confirms the actual source pixels, and a live render probe matched Pillow's
bounds, advance, mode, dimensions, and all 792 mask pixels. The workload's
SIMD and GPU receipts have `actual_backend: null`: this function constructs a
host font object and dispatches no image kernel, so accelerator latency is not
proven or applicable here.

The warm receipts are `perf-load-default-imagefont-attempt2-20260927.json` and
`perf-load-default-imagefont-attempt2-repeat-20260927.json`, with exact-parity
sidecars. The isolated cold-call receipts are
`perf-load-default-imagefont-cold-attempt2-target.json` and
`perf-load-default-imagefont-cold-attempt2-source.json`; the asset regression
is `default_bitmap_matches_embedded_png_decode`.

Reusable optimization finding: when profiles show repeated decoding of a
small immutable built-in asset, cache parsed state first and share only
immutable storage; then measure a fresh-process call separately. If first-call
decoding remains material, a predecoded static asset can remove it, provided a
test derives the bytes through the canonical decoder and checks the full
content. Keep both cold and warm receipts, and do not interpret unavailable
backend telemetry as SIMD/GPU execution. This operation's CPU latency target
is met; the next operation should come from a refreshed single-workload
ranking. No coverage collection ran.

## PIL.ImageFont.load_path checkpoint — 2026-09-27

The refreshed, correctness-gated 825-workload matrix ranked
`pil-imagefont.load-path.standard` as the largest CPU gap: 475.938 µs for
pillow-rs versus 52.333 µs for Pillow. The adjacent `load` row measured
477.563 µs versus 52.729 µs. Both Python entry points call the same
`load_pilfont_from_path` binding helper; `load_path` adds `sys.path` lookup.
The matrix and focused receipts are in
`perf-standard-ranking-after-load-default-imagefont-20260927.json` and the
`perf-imagefont-load-*` files.

A 12,000-call native sample attributed about 48% of hot stacks to PNG
DEFLATE/Huffman decoding inside `Image::materialize`; the same tiny PNG and
256-entry metric table were decoded and parsed on every call. File reads were
a small part of the profile. Attempt one added a one-entry parsed-font cache
keyed by exact metric and bitmap bytes. It is bounded to source payloads at
most 128 KiB and rasters up to 65,536 pixels. It returns cheap clones backed
by the immutable `Arc` data introduced for `load_default_imagefont`. The
loader still resolves the current path and rereads both files before checking
the cache, so path changes and same-size file edits remain observable. This cut
the warm `load_path` call from 470.541 µs to 21.125 µs, but its first decode
still cost 633.917 µs.

Attempt two recognizes the built-in default font only when the freshly read
metrics and PNG bytes exactly match the embedded sources. It then reuses
`PilFont::load_default`, whose decoded luma bytes are checked against the
canonical PNG by `default_bitmap_matches_embedded_png_decode`; all other
content still follows the normal decoder and exact-byte cache. The cold
`load_path` call fell to 93.916 µs. The standard warm receipts measure CPU at
21.187 µs for `load_path` and 20.000 µs for `load`, against Pillow at 50.292
and 49.250 µs respectively. Their target throughputs are 47,198 and 50,000
calls/second versus 19,884 and 20,305 for Pillow. Both focused workload gates
and both public-operation parity cases pass.

A live mutation probe loaded one path repeatedly, first replacing only the
bitmap with a mode-L copy, then restoring the bitmap, then changing only the
advance metric for `a`. pillow-rs matched Pillow's mode, mask, bounds, and
length at each step; the changed advance altered the length and mask width as
expected. This checks that cache hits depend on the file contents read for
that call, not a path, size, or timestamp shortcut. Cold target/source
diagnostics measured 93.916 µs and 9.127 ms; those cold calls include each
runtime's first-use initialization, so compare them only as a cold-process
pair. The warm call-only result is the main operation acceptance measure.

The `load` and `load_path` SIMD/GPU profile receipts have
`actual_backend: null`; these APIs read files and construct host font objects
without dispatching image kernels. Their requested SIMD/GPU numbers do not
prove acceleration and those backend targets are inapplicable. Reusable
finding: for mutable path-backed inputs, validate caches by rereading exact
source bytes, retain only a bounded amount of immutable parsed state, and
measure first-use separately from cache hits. An exact match to a public
embedded resource can safely select its predecoded representation after the
current file contents have been read. No coverage collection ran.

## PIL.ImageFont.ImageFont.getmask checkpoint — 2026-09-27

The old ranking's 465.583 µs CPU time included loading courb08.pil and its
PNG. After the separate load/load_path improvements, the correctness-gated
whole workflow measured 23.708 µs for CPU and 53.500 µs for Pillow. I changed
the standard workload to observe only its call step after setup; the parity
gate still executes and compares the complete loaded-font case. This avoids
charging the load operation to getmask while the independent loader workloads
keep constructor cost visible.

The isolated baseline measured CPU at 3.667 µs (272,702 calls/second) and
Pillow at 3.834 µs (260,824 calls/second). The renderer already returned an
expanded byte-per-pixel mask. PilFontMask::to_image then packed mode-1 rows
only for Image::frombytes("1") to unpack them into the same expanded raster;
for mode L, borrowed frombytes copied the mask into its owned image buffer.
Attempt one added a consuming image path: mode L transfers the vector through
frombytes_owned, and mode 1 maps each nonzero byte to 255 in place before
moving the expanded raster into grayscale storage with the explicit logical
mode tag. This keeps the old packed conversion's behavior even for
noncanonical mode-1 samples. The Python binding uses that path for getmask;
the existing borrowed to_image API remains intact. The new Rust regression
compares mode, size, and exported bytes between both conversion paths for
noncanonical mode 1 and L samples.

The final receipt measured CPU at 2.916 µs (342,936 calls/second) and Pillow at
4.250 µs (235,294 calls/second), with exact parity. The original CPU call
measured 3.667 µs (272,702 calls/second), so the retained implementation is
about 20% lower in latency and 26% higher in throughput for this input. Pillow
latency varied between runs; the target remained faster in each comparison.
The change removes an output allocation and copy for L, and the mode-1
pack/unpack work while preserving the nonzero-to-255 rule in-place.

Attempt two tried copying complete glyph rows with copy_from_slice when a
glyph was wholly inside the output, retaining the old pixel loop for clipped
glyphs. Two public-call samples were 2.875 and 2.917 µs, overlapping the
simpler path's 2.916 and 3.167 µs samples amid small-call variation, so the
branch was reverted. Do not keep a more complex inner loop based on an isolated
kernel intuition when the public-call result does not repeat.

The benchmark reports actual_backend: null for CPU, SIMD, and GPU. PILfont
mask generation is host-side text conversion and raster construction, and this
tiny “Hello” case does not dispatch an image kernel. Its CPU path now beats
Pillow, while a five-times SIMD result or GPU throughput claim is not supported
by execution receipts. Treat accelerator use as inapplicable for this call;
spend the next optimization on an operation that performs enough pixel work to
amortize dispatch. Reusable finding: trace the buffer representation across
each API boundary before tuning the pixel loop. If the producer already owns
the exact expanded raster consumed by the next stage, transfer that allocation
and preserve its logical mode tag instead of encoding and decoding it again.
No coverage collection ran.

## PIL.ImageDraw.ImageDraw.multiline_text checkpoint — 2026-09-27

The original `multiline_text.standard` benchmark called the method with
`"Hello"`, so it timed one rendered line and never exercised line stepping.
I changed only the benchmark input to the existing parity-backed
`"hello\nworld"` case, kept setup outside the measured `call` step, and kept
the benchmark's `parity_pass` gate. No parity case was edited.

Profiling showed that each nonempty line rendered a mask through `getmask2`,
then ran `text_bbox` over the same glyphs solely to advance `line_y`. For
default mask options on nonbinary destination modes, both paths use the same
BASIC `TGT_NORM` glyph run and bbox, so the mask's returned height is already
the required line-step height. The change returns `(width, height)` from the
private render-and-compose helper and reuses that height. It retains the plain
`text_bbox` fallback for nondefault mask options and binary modes, where mask
flags or bounds can differ. Empty lines still advance by `spacing + 10`.

The pre-change two-line baseline measured Pillow at 290.8 µs and 3,439 calls/s;
CPU at 365.9 µs and 2,733 calls/s; SIMD at 370.5 µs; and GPU at 341.6 µs. Two
correctness-gated optimized runs measured CPU at 259.0 and 262.8 µs, or about
28–29% lower latency than the baseline and 9–11% lower latency than Pillow.
CPU throughput rose to 3,807–3,861 calls/s. The benchmark parity outcome was
`pass` on all runs. The complete `multiline_text` operation parity selection
also passed 25/25 cases, including binary modes, anchors, stroke, empty lines,
and byte text.

SIMD/GPU-labelled rows report `actual_backend: null`; the measured draw path
rasterizes glyphs through the CPU font engine and does not prove accelerator
execution. Their timing remains unproven for the 5× SIMD and GPU-throughput
goals. Revisit acceleration only with a real execution receipt and enough
mask/compositing work to amortize dispatch. This operation's CPU latency target
is met, so the next visit should rank the remaining operations from fresh
call-only benchmark evidence. No coverage collection ran.

## PIL.ImageDraw.ImageDraw.multiline_textbbox checkpoint — 2026-09-27

The old standard workload used a one-line default case, bypassing the repeated
per-line work in this method. I redirected its benchmark to the reviewed
three-line `A\nBB\nC` parity case and timed only the `multiline_textbbox` call.
The original operation parity corpus remains unchanged.

The baseline call-only receipt passed parity and measured CPU at 116.6 µs
(8,573 calls/s) and Pillow at 163.2 µs (6,129 calls/s). Profiles and code
inspection showed that multiline bounding boxes called `getlength` for every
line, then called `getbbox` for every line. Default `getlength` uses cached
advance queries; `getbbox` loads the same glyphs again to collect their
advances and boxes. I added a BASIC-layout combined path that loads a line's
glyphs once, derives its 26.6 final pen and bbox from that `GlyphRun`, and uses
those values for width aggregation and alignment. Nondefault layout, color
mask, and special-option paths keep the previous calls; single-line behavior
also keeps its existing direct bbox path.

After the change, all 17 multiline-textbox parity cases passed, including
center/right alignment, stroke, modes, and byte text. Two parity-gated
call-only runs measured CPU at 94.5 and 84.4 µs (10,580–11,843 calls/s), about
19–28% lower latency than baseline and 42–52% lower than Pillow's measured
160.4–174.2 µs. The two optimized benchmark parity gates passed.

SIMD/GPU profile rows report `actual_backend: null`; this font-metric path has
no proven accelerator dispatch. CPU is already faster than Pillow, so retain
the change and move to the next call-only-ranked operation. No coverage
collection ran.

## PIL.ImageDraw.ImageDraw.textbbox checkpoint — 2026-09-27

The old `textbbox.standard` row timed the full image/draw workflow. I changed
the benchmark boundary to the observed `textbbox` call, leaving default-font
construction inside that call because the public method creates a font when
none is supplied. The existing `textbbox` parity cases are unchanged; all
15 passed, and both benchmark parity gates passed.

Two call-only runs measured CPU at 81.1 and 88.6 µs, versus Pillow at 134.2
and 133.5 µs. CPU is about 1.5–1.65× faster on this input, so no runtime change
was needed. SIMD/GPU-labelled profiles report `actual_backend: null`; this
font-bounds call has no proven accelerator dispatch. Use the current residual
single-workload ranking for the next operation. No coverage collection ran.

## PIL.Image.Image.thumbnail checkpoint — 2026-09-27

The old small thumbnail row used a 16 × 16 source, targeted 2 × 2, and mixed
source setup and `putpixel` into the measured workflow. I added one larger,
parity-gated RGB workload: 1024 × 768 to 256 × 192 using BICUBIC and the public
`thumbnail` call followed by receiver `tobytes`. Image construction stays
outside the timed steps; the result includes deferred execution and pixel
export so a lazy target is compared with Pillow's eager mutation. The updated
thumbnail selection passes 53/53 CPU parity cases. The material workload passes
exact CPU, SIMD, and GPU parity, with 100 actual backend executions per target
and no fallbacks.

Three changes remove clones that were immediately discarded. First,
`thumbnail` read source dimensions through `materialize()`, which cloned the
entire materialized image. Reading dimensions through the shared materialized
image preserves eager decode and error timing while avoiding that pixel copy.
On the first paired sample, the CPU pipeline phase fell from 215.9 to 94.6 µs,
SIMD from 216.2 to 116.7 µs, and GPU from 1.233 to 1.118 ms. Second, the CPU
reducing-gap path initialized `work_img` by cloning the source, then replaced it
with the newly allocated reduced image. It now clones only when no reduction
occurs; float/integer reduction helpers still read the original source in their
native scalar domain. Third, SIMD had the same clone-then-replace path and now
branches to the reduced result directly.

The final parity-gated receipt is
`thumbnail-material-1024x768-after-simd-copy-removal.json`. Median observed
latency and reciprocal-latency throughput across 100 executions were:

| Subject | Latency | Throughput | Actual execution |
| --- | ---: | ---: | --- |
| Pillow | 925.1 µs | 1,081 ops/s | Pillow |
| CPU | 643.4 µs | 1,554 ops/s | CPU, 100/100 |
| SIMD | 769.3 µs | 1,300 ops/s | SIMD, 100/100 |
| GPU | 2.101 ms | 476 ops/s | GPU, 100/100 |

CPU is about 1.44× faster than Pillow on this boundary. SIMD is only about
1.20× faster, far short of 5×; GPU is 2.73× slower than SIMD, so its throughput
also misses the goal. The CPU and SIMD terminal-export phases alone measured
530.5 and 661.7 µs, respectively, making host buffer creation and export the
next CPU/SIMD ceiling for this workflow. The GPU receipt records three
dispatches, a 3,145,728-byte upload, a 196,608-byte readback, one full-frame
copy, one mode conversion, and 12,544 auxiliary bytes. Its transfer, conversion,
and dispatch structure is the next GPU target; timings do not yet isolate their
individual costs. These concurrency-one reciprocal rates are not sustained
throughput measurements.

Retain the three copy removals and move on after this checkpoint. When
revisiting thumbnail, profile terminal pixel export separately from resize,
then inspect whether the GPU can avoid its full-frame copy or fuse the reducing
and resampling passes without changing filter, rounding, mode, or error
semantics. The tiny 16 × 16 workflow still loses to Pillow and remains a
separate small-work dispatch/adapter blocker. No coverage collection ran.

## Colorize follow-up checkpoint — 2026-09-27

This follow-up checkpoints four attempts on `PIL.ImageOps.colorize`: balanced
SIMD LUT selection, exact RGB output construction, packed GPU input, and a
benchmark-input correction. The previous large fixture queued `putpixel` on a
newly created image, so the supposedly single-operation case replayed a
two-operation `PutPixel → Colorize` pipeline. A uniform-fill correction removed
that setup operation but overrepresented one cached LUT entry, so it was
replaced with seeded, deterministic high-entropy `L` bytes. The final 1024 ×
768 case constructs the image with `frombytes` before timing and measures only
Colorize plus result export. Its exact parity gate passed on CPU, SIMD, and GPU;
receipts report 100/100 actual executions per requested backend and no
fallback. This fixture lesson applies to all lazy backends: setup must be
materialized before timing, and pointwise LUT workloads should include varied
indices as well as any separately measured uniform case.

The SIMD Colorize path uses a Colorize-local balanced lookup select tree. It
still performs sixteen low-nibble byte swizzles per channel and input block,
but reduces the high-nibble selection dependency depth from fifteen serial
steps to four. Keeping this change local avoids changing code generation for
other users of the shared LUT helper. RGB output is built as exact `[u8; 3]`
pixels in reserved storage and flattened without a second copy; all active
tail pixels are initialized before append. Focused parity-gated SIMD and GPU
checks passed, including an odd 37 × 29 `L` image for packed GPU input.

For a standalone `L → RGB` GPU Colorize, the source is uploaded as four luma
bytes per `u32`; the shader selects the byte by linear pixel index. This
reduces the 1024 × 768 source transfer from 3,145,728 to 786,432 bytes. The
eligibility check excludes chains with preceding GPU operations, whose
intermediate transport is RGBA. It does not shrink the four-byte-per-pixel
output or its 3,145,728-byte readback.

The latest correctness-gated, concurrency-one receipt is
`perf-colorize-large-after-packed-input-noise-20260927.json`; its exact parity
gate passed 3/3 backend comparisons. Median full-call latency and
reciprocal-latency throughput were:

| Subject | Latency | Throughput | Actual backend |
| --- | ---: | ---: | --- |
| Pillow | 1.311 ms | 763 ops/s | Pillow |
| CPU | 0.722 ms | 1,386 ops/s | CPU, 100/100 |
| SIMD | 0.776 ms | 1,289 ops/s | SIMD, 100/100 |
| GPU | 1.113 ms | 898 ops/s | GPU, 100/100 |

CPU is about 1.82× faster than Pillow, so this operation meets the CPU
requirement. SIMD is about 1.69× faster, far below 5×. GPU latency is about
1.43× SIMD latency and its reciprocal-latency throughput is about 30% lower,
so it does not match SIMD throughput. The GPU's packed source transfer is a
useful 4× reduction, but the output readback, output mode conversion,
completion and host result creation remain; the single-request timing does not
identify their individual costs. These reciprocal rates are not sustained
concurrent-throughput measurements.

The remaining Colorize work is therefore SIMD LUT/output bandwidth and GPU
output transport. For SIMD, inspect emitted instructions and register spills
for the sixteen-swizzle lookup before trying more select rearrangements; compare
it against scalar indexed LUT and supported architecture-specific byte
permutation paths, then profile output packing separately. For GPU, profile
completion mapping and host conversion; a compact RGB readback could save
25% of output bytes, but requires race-free packing (for example, four pixels
writing three aligned words) and exact tail trimming. Do not revisit shader
arithmetic while transfer and completion are unmeasured. Move to the next
ranked operation after this checkpoint; the SIMD and GPU goals remain open.
No coverage collection ran.

## PIL.ImageFont.ImageFont.getlength checkpoint — 2026-09-27

The first `getlength` workload was misrouted: `ImageFont.load_default()` returns
a `FreeTypeFont` here, so its label did not mean the base bitmap-font method
was being measured. I changed the fixture chain to load the bundled
`courb08.pil` through `ImageFont.load`, observe the returned
`ImageFont.ImageFont.getlength`, and gate the call-only benchmark on that exact
parity workflow. All four operation cases pass (default, positional arguments,
keyword arguments, and the loaded bitmap font). The Rust regression also checks
ASCII, Latin-1, NUL termination, and the requirement that unsupported text
after NUL still raises the same Latin-1 encoding error.

The baseline call-only receipt measured CPU at 2.188 µs and Pillow at 2.084 µs;
CPU was about 5% slower. Inspection found two avoidable conversions for Python
strings: the binding extracted an owned `String`, then the Rust input adapter
allocated a second Latin-1 byte vector. The retained path borrows a Python
`str`, uses its bytes directly for ASCII, and validates/measures other
Latin-1 text without a temporary vector. It keeps full-input encoding checks
before NUL-terminated width measurement, preserving error order and length
validation.

Two post-change parity-gated runs measured CPU at 1.875 µs each. Pillow measured
2.125 and 2.417 µs, so CPU was 12–22% lower in these runs. The baseline and
Pillow samples are noisy at this scale; repeat the same call-only workload
before treating small deltas as stable. The CPU call is now faster than Pillow
in both post-change samples. This reports concurrency-one reciprocal latency,
not sustained throughput.

The SIMD and GPU rows report `actual_backend: null` and `not_proven`: a bitmap
font advance is scalar text processing, not image-kernel work. Their 5× SIMD
and GPU-throughput goals are inapplicable to this call and are not claimed as
met. Keep this optimization, then rank the next operation using a fresh
call-only workload. No coverage collection ran.

## PIL.ImageDraw.ImageDraw.textlength checkpoint — 2026-09-27

The stale 825-workload matrix showed CPU at 0.785 ms and Pillow at 0.130 ms.
After the preceding font-construction and cache work, a fresh call-only run
with the same parity gate measured CPU at 70.9 µs and Pillow at 116.9 µs; CPU
was about 1.65× faster. The target completed about 14,101 calls/second versus
8,558 for Pillow. This confirms the earlier gap came from a stale dependency
path, not a current `textlength` kernel regression. The benchmark parity gate
passes.

SIMD and GPU profiles report `actual_backend: null` and `not_proven`. This
default-font measurement is text layout, not image-kernel work. No runtime
change is needed; refresh dependent-call benchmarks after changing shared font
loading or caches, and do not carry old ranking ratios forward. No coverage
collection ran.

## PIL.ImageFont.ImageFont.getbbox baseline — 2026-09-27

The previous standard workload called `ImageFont.load_default()`, which
returns a `FreeTypeFont` in this environment and did not exercise the named
base bitmap-font method. I redirected the workload to a parity case that loads
the real `courb08.pil` bitmap font, observes `ImageFont.ImageFont.getbbox`, and
gates timing on the complete setup-and-call result. All four getbbox parity
cases pass.

The call-only benchmark measured CPU at 1.917 µs and Pillow at 2.125 µs, about
10% lower target latency in this sample. SIMD and GPU rows have no actual
backend receipts, so they do not establish accelerator execution. The CPU
target already beats Pillow for this input; retain the current implementation
and move on. No coverage collection ran.

## PIL.Image.Image.getdata checkpoint — 2026-09-27

The original 16 × 16 call-only workload measured CPU at 15.94 µs against
Pillow's 2.04 µs, about 7.8× slower. The hot path allocated one nested Rust
vector per multiband pixel and then built a Python tuple for each pixel before
returning `ImagingCore`. A compact interleaved-byte result removed those
allocations, but its first parity pass was incomplete: the generic mask
serializer converted failed `bytes(core)` and `core.tobytes()` calls into empty
bytes on both sides. That hid every RGB/LA/RGBA pixel. The serializer now
records the public tuples when byte conversion is invalid, and a focused test
protects that distinction.

The stronger parity observation exposed another contract detail: Pillow's
returned `ImagingCore` remains live across `putpixel` and other in-place writes.
A compact snapshot therefore failed retained-view parity despite beating the
latency target. The final binding returns a light Python sequence backed by the
same Rust image handle, calls `load()` before returning to preserve eager
decode/error timing, and obtains compact bytes only when a caller consumes the
sequence. When `thumbnail` changes the image size, the wrapper shallow-clones
the handle and replaces only its own reference; retained sequences continue to
read the old core. A no-op thumbnail keeps the handle shared. The retained-view
case covers both paths, including an intervening pixel write.

The focused parity run passed 26/26 cases across RGB, LA, RGBA, scalar and
palette modes, `I`/`F`/`I;16`, band selection, invalid bands, loaded images, and
retained views. The generated-input check also passes. Three correctness-gated
call-only runs measured CPU at 1.667, 1.666, and 1.750 µs; Pillow measured
2.209, 2.042, and 2.083 µs. CPU latency was 16–25% lower in every paired
sample, with median throughput ranging from 571,000 to 600,000 calls/second
versus Pillow's 453,000 to 490,000. This boundary measures returning the
sequence; consuming all pixel tuples is a separate workload and remains
unmeasured here.

SIMD and GPU labels report `actual_backend: null` and `not_proven`. Returning
host pixel data does not execute an image kernel, so these measurements do not
claim SIMD or GPU speedup. Keep the CPU path, keep the serializer regression
test, and rank the next operation using a fresh call-only measurement. No
coverage collection ran.

### WASM getdata parity harness follow-up — 2026-09-27

The pushed checkpoint's Node and browser parity jobs exposed the same false
empty-byte encoding in the JavaScript serializer. Once the Python serializer
preserved multiband tuples, 16 WASM cases became visible; fixing that encoding
left one retained-view mismatch. The JS workflow adapter had captured
`getdata()` at call time, while Pillow's `ImagingCore` reads live storage until
a size-changing `thumbnail()` replaces that storage.

The adapter now serializes the actual sample sequence when byte conversion is
invalid. Its no-band getdata descriptor reads the current Rust image when
observed. A size-changing thumbnail shallow-copies the old Rust image handle
before mutation and retargets existing views to it; a no-op thumbnail leaves
them live. The focused Node and browser runs now pass all 16 affected cases.
This was a serializer and parity-workflow modeling defect, not a pixel-result
change in the Python implementation. Keep pixel tuples and retained-view
behavior in both host adapters; never replace failed multiband byte coercion
with an empty-byte value. No coverage collection ran.

## PIL.Image.Image.get_flattened_data checkpoint — 2026-09-27

The original whole-workflow row was not a clean call comparison. I changed its
measurement boundary to time only `get_flattened_data()` after image setup.
The 16 × 16 RGB baseline then measured CPU at 21.042 µs and Pillow at 5.709 µs.
The target constructed a live `ImagingCore` proxy, then iterated it through a
Python generator that sliced one byte string per pixel to create the eager
tuple result. That proxy is needed by `getdata()`, but this method returns an
eager immutable tuple.

The optimized path keeps the existing band-selection branch. With no band, it
preserves `load()` and the wrapper's getdata-view bookkeeping, asks the Rust
binding for the same formatted data, and materializes scalar byte output with
`tuple(bytes)` or multiband pixels with `struct.iter_unpack`. The latter
removes the per-pixel Python generator and temporary byte slices. Numeric
`I`, `F`, and `I;16` lists still become tuples directly. Existing default,
band, and L-mode parity cases pass; a source/target cross-process check also
matches Pillow for modes 1, L, P, LA, RGB, RGBA, I, F, and I;16.

Two correctness-gated call-only runs measured CPU at 5.167 and 4.917 µs;
Pillow measured 5.833 and 5.708 µs. CPU is 4.1–4.3× faster than the prior
implementation and 11–14% faster than Pillow in these paired runs. Median
throughput was 193,536 and 203,376 calls/second versus 171,438 and 175,193 for
Pillow. Keep this change and proceed to the next operation.

The SIMD/GPU profile timings are the same host-side tuple construction, with
`actual_backend: null` and `not_proven`; this API launches no image kernel.
These numbers do not establish SIMD or GPU acceleration. No coverage collection
ran.

## PIL.Image.Image.getcolors direct-slot revisit checkpoint — 2026-09-27

`getcolors` had already reached a four-attempt checkpoint. This revisit made
two bounded implementation attempts, bringing the total beyond the campaign's
three-to-four-attempt limit. Keep the direct-slot table and checkpoint the
remaining latency gap rather than continue tuning without phase evidence.

The retained `PillowColorCounts` uses a contiguous table when Pillow's slot
mask is at most 1,023. Each `u64` stores the packed pixel key and count, so the
reference-compatible probe is the only lookup; enumerating those slots also
preserves Pillow's observable result order without a final sort. Larger tables
stay sparse so an extreme `maxcolors` request does not allocate a vector in
proportion to the requested limit. The prior early exit at the `(maxcolors +
1)`th unique value remains in place. The small-table path reduced the fresh
standard CPU call from 3.625 µs to 2.125 µs, while retaining exact Pillow output.

A second attempt replaced `.pixels()` traversal with raw-byte chunk scans and
inline packing. It regressed the direct-table candidate on both the standard
call (2.334 µs versus 2.167 µs) and varied RGB 16 × 16 (20.417 µs versus
19.062 µs), with no change for high-cardinality early exit (2.375 µs). That
attempt was reverted. Contiguous source bytes alone do not guarantee a faster
loop when the scan must decode and pack every pixel; compare the complete call,
including key construction and output, before changing traversal.

The final correctness-gated run is
`build/migration-parity/perf-getcolors-revisit-final.json`, with matching
parity artifact `perf-getcolors-revisit-final-parity.json`. Five-sample medians
were:

| Workload | Pillow | CPU | SIMD profile | GPU profile |
| --- | ---: | ---: | ---: | ---: |
| RGB 16 × 16 standard | 1.625 µs | 2.125 µs | 2.167 µs | 2.041 µs |
| RGB 16 × 16 varied | 9.792 µs | 19.709 µs | 19.041 µs | 19.458 µs |
| RGB 1,024 × 768 high cardinality | 2.208 µs | 2.375 µs | 2.375 µs | 2.416 µs |

All three benchmark exact-output gates pass. The full focused all-backend
artifact is `build/migration-parity/perf-getcolors-revisit-final-all-backends.json`:
all 36 cases pass in each selected lane, including CPU, SIMD, GPU, Python, Node,
and browser. GPU full-request validation passed. SIMD/GPU receipts for this
eager host-side API do not establish accelerator execution; reported `actual_backend`
is null for the timing rows. CPU remains 1.31× slower than Pillow on the
standard call, 2.01× slower on varied RGB, and 1.08× slower on large early exit.
The blockers are the remaining per-call and per-pixel core cost plus output
construction; no phase profile currently attributes that cost. Profile those
phases before another implementation attempt. This operation remains incomplete.
No coverage collection ran.

The optimization skill now records the reusable decision: use packed direct
slots only when the reference table is bounded, preserve its exact probe and
enumeration order, retain sparse storage for large limits, and reject raw-byte
iteration unless complete-call measurements improve representative entropy
and cutoff cases.

## PIL.Image.Image.getchannel revisit checkpoint — 2026-09-27

Three bounded attempts revisited the existing getchannel blocker. First, the
GPU extract shader now packs four L samples into each u32 output word and maps
only the active one-byte-per-pixel result, with four-byte alignment padding.
This quartered readback for 1024 × 768 images from 3,145,728 to 786,432 bytes.
The isolated GPU call improved by about 7–12% on RGB, LA and RGBA, but upload,
dispatch, map completion and host output still leave it far slower than CPU or
SIMD. The transfer reduction is useful for composed GPU-resident pipelines; it
does not make a standalone extraction competitive.

Second, the SIMD path replaced scalar indexed gathers with 16-byte vector
shuffles and masks, precomputed per-channel lane selectors, then handled the
final partial block scalarly. Two correctness-gated runs put 1024 × 768 SIMD
medians around 100–105 µs for RGB, LA and RGBA, versus the prior 157–227 µs
path. CPU measured 46–107 µs and Pillow 147–194 µs across those modes. SIMD is
still only about 1.5–1.9× Pillow, not the 5× target; small RGB remains slower
than Pillow because fixed wrapper and allocation cost dominates. The shuffle
path proves vector execution but does not beat the simpler CPU kernel
consistently: interleaved RGB gathers require several shuffles and masks, so
inspect emitted instructions before adding more vector complexity.

Third, splitting the cheap vector gather into parallel rows was rejected. Its
correctness-gated run measured SIMD at 172–197 µs for the large modes, with
high sample variance and a longer backend phase than the prior vector-only
repeat. This work is a cheap byte extraction over a bandwidth-limited buffer;
row task overhead and memory contention erased the benefit. Do not parallelize
this loop again without a repeatable isolated crossover measurement.

The final parity artifact, `build/migration-parity/perf-getchannel-revisit-final-all-backends.json`,
passes all 130 selected cases in CPU, SIMD, GPU, Python, Node and browser lanes;
the full-request GPU gate also passes. Benchmark exact-output gates pass in
the vector-only artifacts. Keep compact GPU output and sequential vector
gather; revert row parallelism. The remaining work is a lower-instruction
packed-channel permutation for SIMD, reducing per-call cost for tiny images,
and keeping GPU input/output resident across a longer fused pipeline. Standalone
GPU transfer floors mean this operation alone cannot meet GPU-equals-SIMD.
Artifacts are `build/migration-parity/perf-getchannel-revisit-compact-output.json`,
`perf-getchannel-revisit-vectorized-repeat.json`, and
`perf-getchannel-revisit-parallel-vector.json`. No coverage collection ran.

## PIL.ImageEnhance.Sharpness checkpoint — 2026-09-28

Four bounded performance passes are checkpointed. Factor one returns an
independent copy on CPU and SIMD; GPU removes that exact identity before
upload and records zero dispatches. For active filtering, the integer weighted
sum is bounded by 3315 and `(weighted + 6) / 13` exactly rounds the Pillow
3×3 smoothing result. CPU writes the filtered and blended bytes into one output
buffer. SIMD fuses filtering and blending into one output pass, keeps the
smoothed-byte narrowing between those stages, and evaluates the blend with
fused float32 difference arithmetic. On targets where `wide::f32x8::mul_add`
is not fused, it uses scalar fused lane arithmetic so x86 without FMA keeps the
same byte result. The GPU fixed-point factor is admitted only after proving it
matches that float32 blend for every byte pair.

Two parity regressions now pin down the blend contract. Factor 0.3 with a
nonzero border sample catches the old weighted-f64 expression truncating an
unchanged value by one. Factor 1.1 uses a 3×3 image whose center blends samples
2 and 12; it catches separate multiply/add rounding to 1 where Pillow's fused
operation truncates to 0. GPU factor 1.1 is not a supported profile because
the fixed-point shader cannot prove exact output for every byte pair.

The corrected benchmark is
`build/migration-parity/sharpness-e9fa-corrected.json`, with six exact-output
gates in `sharpness-e9fa-corrected-parity.json`. The 1024 × 768 RGB input is
created with `frombytes` outside the timer; the measured steps are Sharpness
construction, `enhance`, and terminal `tobytes`. Each workload has five
warmups and 100 measured observations (five samples × 20 iterations) at
concurrency one. Both workloads passed Pillow parity on CPU, SIMD, and GPU.

| Workload | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Factor 1.5, active filter | 13.545 ms | 7.647 ms | 3.069 ms | 2.156 ms |
| Factor 1.0, identity | 4.733 ms | 0.246 ms | 0.253 ms | 0.385 ms |

For the active filter, CPU is 1.77× faster than Pillow and SIMD is 4.41×
faster, still below the 5× goal. GPU median latency is 29.8% lower than SIMD,
with 100/100 actual GPU executions, exactly one Sharpness operation and one
dispatch per request, and no fallback. This benchmark reports serial
concurrency-one latency; its reciprocal throughput field does not demonstrate
sustained throughput. For identity, GPU executes on every request with zero
dispatches, but its latency remains 52.0% higher than SIMD. The measured
non-identity CPU workload is faster than Pillow; these two material inputs do
not prove every Sharpness shape or mode meets that goal.

The former `sharpness-e9fa-final4.json` active comparison is superseded. Its
timed materialization also flushed a deferred setup `putpixel`, so the target
ran two operations and GPU ran two dispatches while Pillow had performed that
setup eagerly. Do not use its latency ratios. The corrected active and identity
input hashes are `9a77b4da8c7019debbad18a50fa8e9923425ba770b4a04b3fddd95a15694d152`
and `9cc08d7196415e3d179b0b2b02b5e78f31f1a59fe32642aaa198e6d215848bc9`;
the shared RGB bytes hash to
`c840db423eb756ccc880049237174a89a504f4e0699ecbf65835abc376ede4f9`.
Strict operation parity passed 42/42 CPU cases, 4/4 SIMD cases, and 3/3 GPU
cases. The six benchmark-specific parity gates also passed. No coverage
collection ran.

At this checkpoint the campaign selected `PIL.Image.Image.reduce` for a fresh
material, parity-gated baseline. The old top-ranked `ImageOps.invert` row was a
one-sample 16 × 16 diagnostic without parity proof, so it did not justify
reopening that completed operation or choosing the next optimization.

## PIL.Image.Image.reduce checkpoint — 2026-09-28

The first Reduce campaign added a seeded, varied RGB workload at 1,024 × 768
with factor `(3, 5)`. Its 342 × 154 result exercises full blocks and the right,
bottom, and corner tails. Separate 64 × 48 `L`, `LA`, and `RGBA` cases exercise
single-band and alpha handling. The benchmark times `reduce` and materializes
the result with `tobytes`; input creation is outside the timed steps. Each run
uses five warmups and 100 observations at concurrency one, with exact Pillow
output as the benchmark parity gate.

Four bounded implementation attempts are checkpointed. The SIMD path now
precomputes the full/right/bottom/corner block geometry and fixed-point average
parameters once, instead of dividing and rebuilding reciprocal parameters for
each output pixel. It passes output-row coordinates into the SIMD block kernel,
removing per-lane global-index division and remainder. Its common full-block
path also accumulates into `u32` directly; construction proves the sample-count
bound before this path is admitted, so it avoids checked `u64` accumulation
and narrowing in the hot loop. The exact edge path retains checked sums and
per-pixel geometry.

| Run | Pillow ms | CPU ms | SIMD ms | GPU ms |
| --- | ---: | ---: | ---: | ---: |
| Baseline | 0.405417 | 0.247750 | 0.312229 | 1.040167 |
| Attempt 1: hoist geometry and average parameters | 0.391625 | 0.271896 | 0.295146 | 1.039188 |
| Attempt 2: vectorize source accumulation (reverted) | 0.425520 | 0.264771 | 0.317458 | 1.009041 |
| Attempt 3: row-aware SIMD lanes | 0.411271 | 0.262563 | 0.291479 | 1.020188 |
| Attempt 4: direct `u32` full-block sums | 0.405063 | 0.272042 | 0.275396 | 1.032334 |

Attempt 2 passed parity but made SIMD slower than baseline and was removed.
Attempts 1 and 3 improved SIMD median latency modestly; attempt 4 brought it to
0.275 ms, about 1.47× faster than Pillow on this material workload. The CPU
median is about 1.49× faster than Pillow. These per-run medians vary with host
load, so they establish a useful workload-specific result, not universal
performance for every shape or mode. SIMD still misses the 5× target by a wide
margin.

GPU is about 2.55× slower than Pillow and 3.75× slower than SIMD. Its receipt
shows 100/100 real GPU executions, one dispatch per call, and no fallback, but
also a 3,145,728-byte upload, 210,672-byte readback, one full-frame copy, and
one mode conversion per request. The next GPU design needs to remove staging or
retain the source in a native layout; shader arithmetic is not the first
attack while these transfers dominate.

Strict final parity passed 4/4 cases on each CPU, SIMD, and GPU lane, covering
the varied RGB input and `L`, `LA`, and `RGBA`. The benchmark exact-output gate
also passed on all three actual backends. Receipts are
`build/migration-parity/reduce-e9fa-attempt4-cpu.json`,
`reduce-e9fa-attempt4-simd.json`, and `reduce-e9fa-attempt4-gpu.json`; the
benchmark and its gate are `reduce-e9fa-attempt4.json` and
`reduce-e9fa-attempt4-parity.json`. Every benchmark artifact reports base
revision `73d52050346cbc6ee595306f3ed7ef5ff8b8136e` with `dirty: true`; the
runner does not record a working-tree hash, so the receipts do not independently
identify each intermediate attempt's exact diff. The final attempt was
revalidated on all three backends above. No coverage collection ran.

The next SIMD revisit should reduce scalar source loads and per-sample work;
naively vectorizing source accumulation already regressed, so inspect the
generated loop and measure the complete call before retrying a vector layout.
The CPU goal is met only for this 1,024 × 768 RGB workload. SIMD and GPU goals
remain open; stop this operation at four attempts and proceed to the next
ranked, uncheckpointed operation.

## PIL.ImageFilter.GaussianBlur checkpoint — 2026-09-28

Three bounded implementation attempts are checkpointed. The retained change is
the GPU kernel rewrite. Each horizontal or vertical pass now assigns output
pixels across a bounded 2D grid of 16 × 16 workgroups instead of letting one
invocation walk a complete row or column. The dispatch planner uses the
device's per-dimension workgroup limit; shader grid-stride loops cover pixels
when the image is larger than the planned grid. The six internal box passes,
fractional edge weights, replicated borders, intermediate byte rounding, and
channel layout remain intact. A focused planner unit checks grid bounds and
exact single coverage of each pixel.

A post-checkpoint code review found that GPU admission still budgeted the old
radius-independent rolling-window work. The new per-pixel stencil repeats its
window for every output, so the guard now counts the four channel samples per
tap and the two fractional-edge samples, multiplied by the actual radius and
number of passes. Blur preflight also uses the bounded planner instead of
rejecting a dimension merely because its pixel count exceeds the workgroup
limit; the shader grid-strides the remaining pixels. Focused tests cover
radius-scaled rejection at the maximum image size and dispatch dimensions
larger than the device grid. Large-radius, maximum-size blurs can still be
routed to CPU by the watchdog budget; a rolling or hierarchical GPU kernel is
needed before those workloads can stay on GPU safely.

Attempt 2 made SIMD radius-1 accumulation calculate each three-tap output
independently. Its SIMD median regressed from 4.020 ms to 4.568 ms, so that
change was removed. Attempt 3 routed CPU and SIMD through a shared cache-tiled
interleaved transpose. It regressed CPU from 4.792 ms to 5.344 ms and SIMD from
4.020 ms to 4.595 ms; it too was removed. Both attempts passed the focused
parity cohort, but neither improved the measured complete call. Do not retry
either design without new phase or generated-instruction evidence.

The first benchmark artifact named `gaussianblur-e9fa-baseline.json` is
superseded. The timing parser appended explicitly requested step IDs to a
default `call` step, which caused setup before `apply-filter` (including the
large `frombytes`) to enter the measured interval. The parser now defaults to
no explicit steps and adds `call` only when the caller supplies none. The
focused `scripts.test_migration_parity_timing` test exercises both CLI cases.
`gaussianblur-e9fa-corrected.json` is the valid pre-kernel baseline: it times
filter application and terminal output observation, with image construction
outside the timer.

The final exact-output benchmark is
`build/migration-parity/gaussianblur-e9fa-guardfix.json`, with its gate in
`gaussianblur-e9fa-guardfix-parity.json`. It uses varied RGB noise at 1,024 × 768,
radius 2, warm cache, five warmups, 20 iterations per sample, five samples,
and concurrency one. The measured steps are `apply-filter` and
`observe-filter-result`; filter-object construction and image creation are
outside the timer. The final benchmark input asset is
`86cffa627633d9bf0b8b7ddfb2826c433a8b3841361a06087770b989876595ed`; the
manifest hash is `c3c3c6c6c25ca8bbd51f7b37cf9dccdf86bc983bf2768cd8ed45ac75cb8860d`.
It ran on macOS 15.7.7 arm64 with Pillow 12.2.0 and target revision
`b38c616d0eb91402dd4bdf5a73e93e7719fffc38` (`dirty: true`).

| Run | Pillow ms | CPU ms | SIMD ms | GPU ms |
| --- | ---: | ---: | ---: | ---: |
| Corrected baseline, before GPU rewrite | 5.355 | 4.778 | 4.105 | 6.545 |
| Attempt 1: output-parallel GPU blur | 5.625 | 4.792 | 4.020 | 2.955 |
| Attempt 2: direct three-tap SIMD (reverted) | 5.651 | 4.718 | 4.568 | 2.905 |
| Attempt 3: shared tiled transpose (reverted) | 5.658 | 5.344 | 4.595 | 2.962 |
| Final radius-aware GPU admission | 5.710 | 4.780 | 4.121 | 2.974 |

After the guard correction, `gaussianblur-e9fa-guardfix.json` supersedes the
earlier `gaussianblur-e9fa-final.json`. Its run ID is
`migration-benchmark-99b5ac7ddb84459d971f4bc4470d2858`, on target revision
`b38c616d0eb91402dd4bdf5a73e93e7719fffc38` (`dirty: true`). CPU is 1.19×
faster than Pillow and SIMD 1.39× faster, still far short of the 5× SIMD goal.
GPU is 1.39× faster than SIMD by median latency. All 100 measured GPU
executions used the GPU, with one GaussianBlur operation, six dispatches, no
fallback, 3,145,728 bytes uploaded and the same amount read back. The listed
throughput metric is collected at concurrency one; it does not establish
sustained throughput. Differences between attempts include host/run variance,
so use these results as workload-specific measurements, not universal ratios.

Strict exact-output parity passed 5/5 selected cases in each CPU, SIMD, and GPU
lane (15/15 total) after the guard correction: odd-width RGB BoxBlur plus
GaussianBlur on varied RGB 1,024 × 768 and varied `L`, `LA`, and `RGBA`
images. Each selected backend was required to run without fallback. The
benchmark's own exact-output gate also passed on CPU, SIMD, and GPU. These
five cases do not establish parity for every image size, radius, or border
shape. The planner/grid-budget Rust tests, `make fmt`, `make clippy`,
`make migration-parity-inputs-check`, `make build-parity`, and the
timing-parser unit test passed. No coverage collection ran.

The GPU remains transfer-bound for standalone calls: each request stages a
3 MiB input and output and performs six dispatches. First investigate keeping
intermediate GPU data resident or fusing compatible blur passes before tuning
shader arithmetic further. For CPU, profile each of the six passes, the two
vertical transposes, buffer allocation/copy, and Rayon scheduling separately.
For SIMD, measure a direct vectorized column kernel against the current
transposed row kernel; the cache-tiled wrapper alone did not help. Require a
stable complete-call win and exact output before retaining another attempt.
The global CPU, SIMD, and GPU goals remain open. Proceed to the next
uncheckpointed operation after this three-attempt visit.

## PIL.Image.Image.crop checkpoint — 2026-09-28

The old standard crop row exercised `crop(None)` on a 16 × 16 image. That
shares/copies the source and does not execute the explicit-box crop kernel, so
it was not useful evidence for crop performance. The standard row now times an
explicit in-bounds crop of deterministic RGB noise at 1,024 × 768, box
`(97, 65, 928, 704)`, followed by `tobytes`. Setup is outside the measured
steps (`call`, `observe-result`); the run uses five warmups, 100 observations,
and concurrency one. A separate full-width vertical crop uses box
`(0, 65, 1024, 704)`. Small varied `L`, `LA`, and `RGBA` cases check channel
packing and row tails. These cases do not cover signed/out-of-bounds boxes,
empty outputs, palette modes, or typed `I;16` paths.

Three bounded attempts were made. The first removes the CPU serial native-byte
crop's zero-fill-then-overwrite: it reserves the checked output size, records
the allocation, and appends each source row. The existing 4 MiB parallel
threshold remains. The threshold experiment at 1 MiB was reverted: the RGB
crop below 4 MiB entered the parallel branch and its CPU median regressed from
0.174 ms to 0.298 ms, despite passing output checks. The third attempt added a
contiguous-span copy for full-width crops. Its CPU half was removed after a
repeat did not reproduce the first measured win; the SIMD native-copy path
keeps the span fast path, with row copying for other boxes.

| Workload / run | Pillow ms | CPU ms | SIMD ms | GPU ms |
| --- | ---: | ---: | ---: | ---: |
| Interior RGB, before serial allocation change | 0.432250 | 0.173812 | 0.173187 | 1.469542 |
| Interior RGB, serial append path | 0.420792 | 0.171834 | 0.172458 | 1.417333 |
| Interior RGB, 1 MiB parallel threshold (reverted) | 0.408749 | 0.298021 | 0.164104 | 1.466542 |
| Full-width RGB, before contiguous-span path | 0.440875 | 0.151084 | 0.203625 | 1.500667 |
| Full-width RGB, initial CPU + SIMD span trial | 0.443229 | 0.145500 | 0.160271 | 1.480708 |
| Final code: CPU row copy, SIMD span copy | 0.427105 | 0.143583 | 0.183709 | 1.465417 |

On the matched full-width workload, the initial span trial reduced CPU median
by 3.7% and SIMD by 21.3%. That CPU result did not reproduce: the final code
keeps CPU row copying, and the SIMD span path measured 0.184 ms versus 0.204 ms
before the change, about a 9.8% median reduction. Run-to-run variance is
material, so treat this as a modest workload-specific SIMD improvement, not a
universal speedup. The final code's CPU measurement was 0.144 ms versus 0.151
ms in the earlier full-width run, but that difference cannot be attributed to
the CPU implementation. The interior row's first-attempt CPU change was only
about 1%. The lower parallel threshold clearly regressed CPU. These receipts
share the same base commit and are marked `dirty`; they do not contain
source-tree hashes, so per-attempt numbers are observations tied to the run
sequence, not reproducible code fingerprints. The full-width runs used the
same input and benchmark policy on macOS 15.7.7 arm64 with Pillow 12.2.0 and
CPython 3.12.

The benchmark's exact-output gate passed for all subjects. Receipts report 100
actual CPU, SIMD, and GPU executions, without fallback. The GPU used one
dispatch but uploaded 3,145,728 bytes and read back 2,617,344 bytes for each
call, plus a full-frame copy and mode conversion. GPU latency is about 8×
SIMD latency in the final verified run. Host-to-device staging, synchronization,
and readback dominate the one-copy shader; shader instruction tuning is not
the first GPU target. SIMD uses native row copies rather than arithmetic SIMD;
the typed `I;16` path still materializes bytes and reconstructs typed samples.

Strict exact-output parity passed 5/5 selected cases independently on CPU,
SIMD, and GPU (15/15 total), including the SIMD full-width RGB fast path and
the interior RGB plus varied `L`, `LA`, and `RGBA` cases. Every backend was
required to execute without fallback. The final receipts are
`build/migration-parity/crop-checkpoint-cpu.json`, `crop-final-simd.json`, and
`crop-final-gpu.json`; the matched full-width benchmarks and gates are
`crop-full-width-baseline.json`, `crop-full-width-baseline-parity.json`, and
`crop-checkpoint.json` with `crop-checkpoint-parity.json`. `make build-parity`,
`make migration-parity-inputs-check`, and `make fmt` passed. No coverage
collection ran.

This visit stops after three bounded attempts. CPU is faster than Pillow for
the measured byte crops, but the 5× SIMD goal and GPU-versus-SIMD goal remain
open. Next crop work would need to reduce typed-sample materialization for
SIMD and remove full-frame staging/readback for GPU; the current standalone
GPU execution model cannot amortize those transfers. Move to the next ranked
operation and revisit crop only with a resident/batched GPU design or a direct
typed-sample implementation to measure.

## PIL.Image.Image.point checkpoint — 2026-09-28

The default Point workload is a 16 × 16 identity lookup and measures dispatch
overhead rather than pixel mapping. This visit adds a deterministic 1,024 × 768
`L` image with varied bytes and a non-identity LUT, then observes `tobytes`.
Image and LUT setup remain outside the timed steps (`call`, `observe-result`).
The policy is five warmups, 100 observations across five samples, warm state,
and concurrency one. Small varied `L`, `LA`, `RGB`, and `RGBA` cases exercise
channel tables and tails; the odd pixel count in `L` exercises the packed
shader's padded final word. Each exact parity lane selected all five cases and
forced CPU, SIMD, or GPU independently. The benchmark also ran an exact-output
gate for all three target profiles. No coverage was collected.

The baseline and attempt receipts use Pillow 12.2.0, CPython 3.12.13, and
macOS 15.7.7 arm64. They share parent revision `028295a0cd831aa0a249a260e175c5835f676832`
and mark the source worktree dirty; the receipt IDs below distinguish the
measured runs, while the final checkpoint commit anchors the retained source.
Whole-workflow medians are milliseconds:

| Run | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| Baseline (`7c9370e1`) | 0.342229 | 0.256271 | 0.269208 | 1.877604 |
| Attempt 1: larger parallel threshold (`ec73c14b`, reverted) | 0.352333 | 0.252187 | 0.497917 | 1.909979 |
| Attempt 2: direct-output mapping (`c87ce0b2`) | 0.343583 | 0.440084 | 0.256438 | 1.888542 |
| Attempt 2 refinement: byte collect (`5d580b52`) | 0.335688 | 0.274625 | 0.277729 | 1.951896 |
| Attempt 2 unchanged repeat (`dd01c4aa`) | 0.346021 | 0.263209 | 0.298167 | 2.033229 |
| Attempt 3: balanced SIMD selects (`a0a3c3cf`) | 0.334500 | 0.239167 | 0.248854 | 1.908626 |
| Attempt 3 unchanged repeat (`ac66617b`) | 0.349771 | 0.241146 | 0.268813 | 1.874333 |
| Attempt 4: compact L GPU transfer (`b7ce40f7`) | 0.336167 | 0.257437 | 0.265604 | 0.776791 |
| Attempt 4 unchanged repeat (`9c554abf`) | 0.337105 | 0.252521 | 0.257250 | 0.728792 |
| Final receipt after odd-L tail parity (`bced6b7e`) | 0.376083 | 0.290520 | 0.274938 | 0.796979 |

Attempt 1's higher parallel threshold nearly doubled SIMD latency and was
reverted. Attempt 2 maps from immutable source bytes into one output allocation
instead of cloning and rereading the clone; its first CPU builder regressed
badly, so the `L` builder was changed to iterator collection. The unchanged
repeat shows no stable SIMD improvement from direct-output mapping alone.
Attempt 3 keeps that mapper but uses a Point-specific balanced selection tree:
the sixteen LUT swizzles stay independent, while the dependent selection depth
falls from fifteen serial selects to four levels. The two whole-call runs put
CPU at 0.239–0.241 ms and SIMD at 0.249–0.269 ms. CPU improvement repeats;
SIMD is neutral-to-slightly faster, not a demonstrated large gain. The general
LUT lowering remains unchanged for other operations.

Attempt 4 packs four `L` samples per storage word, maps those bytes in one
Point shader invocation, and reads compact `L` output directly. A checked
planner uses the adapter's workgroup limit and declines this specialization if
the packed grid cannot fit. Its unit test exercises the 4,096 × 4,096 grid and
the exact workgroup boundary without allocating those images. Both repeated
benchmarks show a GPU median of 0.729–0.777 ms, about 2.4–2.6× faster than the
baseline, with 100/100 actual GPU executions and no fallback. Upload and
readback each fall from 3,145,728 bytes to 786,432 bytes, and the reported
mode-conversion count falls from one to zero. The combined `backend_ns` phase
falls from 1.817 ms to 0.652–0.697 ms; it includes upload, dispatch, polling,
mapping, and readback, so it does not isolate transfer time.

The matched attempt-4 benchmark gates passed on CPU, SIMD, and GPU. Strict exact
parity then passed 5/5 selected cases independently on each backend (15/15
total), including the odd-count packed `L` tail and unchanged `LA`, `RGB`, and
`RGBA` channel semantics. The final input-hash-matched benchmark is
`build/migration-parity/point-final.json` (run
`migration-benchmark-bced6b7edfa843da848c5d404b337fc3`); its exact-output
sidecar is `point-final-parity.json`. The strict final lanes are
`point-final-cpu-parity.json`, `point-final-simd-parity.json`, and
`point-final-gpu-parity.json`. The two attempt-4 runs remain available as
`point-attempt4.json` and `point-attempt4-repeat.json` for the matched-change
comparison.

CPU is 1.29–1.34× faster than Pillow on these runs, meeting the CPU latency
goal for this workload. SIMD is only 1.26–1.37× faster than Pillow, far short of
5×. GPU remains 2.8–2.9× slower than SIMD, so neither the GPU latency target nor
the higher-throughput target is met. Throughput here is reciprocal latency at
concurrency one; it does not establish sustained request throughput. The
remaining GPU gap points to per-call upload, synchronization, and materialized
readback overhead after the compact transfer, rather than a slow LUT alone.
Further progress likely needs resident GPU data or batched calls; measure those
costs before tuning shader arithmetic. SIMD still needs a lower-level
lookup/vectorization design that complies with the workspace's `unsafe_code =
deny` policy. Point is checkpointed incomplete after four bounded attempts.

## PIL.Image.Image.putdata checkpoint — 2026-09-28

The measured boundary is `putdata` followed by receiver `tobytes`, on a varied
1,024 × 768 native-L image with a complete byte payload. Setup is outside the
timed steps. The standard policy runs five warmups and 100 observations across
five samples at concurrency one. Runs used Pillow 12.2.0, CPython 3.12.13, and
macOS 15.7.7 arm64. The source snapshot was `810fa8557` with local changes;
each receipt records its input hashes and actual backend samples. The parity
input was corrected during this visit, so later receipt input hashes differ,
while the benchmark-selected L asset and timed boundary remain the same.
No coverage was collected.

The whole-call median latency and median throughput were:

| Run | Pillow ms / ops/s | CPU ms / ops/s | SIMD ms / ops/s | GPU ms / ops/s |
| --- | ---: | ---: | ---: | ---: |
| Baseline (`390c26a4`) | 1.410 / 709 | 3.003 / 333 | 3.011 / 332 | 6.435 / 155 |
| Attempt 1: byte fast path (`c59dcb41`) | 1.354 / 738 | 1.471 / 680 | 1.470 / 680 | 4.818 / 208 |
| Attempt 2: full-write CPU/SIMD paths (`fc76f071`) | 1.403 / 713 | 1.397 / 716 | 1.383 / 723 | 4.805 / 208 |
| Attempt 3: skip old-image GPU upload (`96819204`) | 1.425 / 702 | 1.484 / 674 | 1.428 / 700 | 4.133 / 242 |
| Attempt 4: packed-L GPU path (`15c16261`) | 1.457 / 686 | 1.361 / 735 | 1.406 / 711 | 2.815 / 355 |
| Attempt 4 unchanged repeat (`56a57593`) | 1.331 / 751 | 1.245 / 803 | 1.323 / 756 | 2.636 / 379 |
| Final source/input confirmation (`0805acd7`) | 1.306 / 766 | 1.251 / 799 | 1.250 / 800 | 2.535 / 394 |

Attempt 1 passes exact one-byte samples directly from the bytes input instead
of widening each byte into `PutDataValue` and encoding it back. CPU and SIMD
median latency fell from about 3.0 ms to 1.47 ms. Attempt 2 constructs a
complete native-layout replacement directly on CPU, avoiding a clone and
overwrite of the old frame; SIMD likewise copies the complete payload directly
instead of first copying the old image. These brought CPU and SIMD close to
Pillow, but the CPU p95 remained slightly slower in that run. The SIMD full
copy is a native copy operation; current telemetry proves SIMD backend
selection but does not report vector blocks for that copy.

Attempt 3 skips uploading the old L frame on complete replacement and makes
the generic shader load old pixels only when a write is partial. That removed
the 3 MiB primary-image upload, but still transferred a 3 MiB auxiliary
payload and read back 3 MiB of expanded RGBA. GPU median improved from 4.805 to
4.133 ms. Attempt 4 handles native-L samples four per `u32`, packs replacement
bytes in the same layout, and reads compact L output. It keeps a compact source
upload for partial writes, skips the source upload only for a complete write,
preserves the exact byte prefix, and zeroes only unused transfer padding. The
full-write benchmark now transfers 0 bytes as the primary image, 786,432
auxiliary bytes, and 786,432 readback bytes per sample; mode conversions fell
from one to zero. Total measured GPU image traffic fell from 9 MiB before the
specialization to 1.5 MiB. The adapter-backed shader still ran one dispatch.

The attempt-4 run and unchanged repeat show GPU medians of 2.815 and 2.636 ms,
with 355 and 379 operations/s. Both receipts report 100/100 actual GPU
executions and no fallback. In the same runs, GPU remains about 2× slower and
produces about half the throughput of SIMD. CPU beats Pillow on the measured L
workload in both runs (1.361 vs 1.457 ms, then 1.245 vs 1.331 ms); this does
not establish CPU performance for every PutData mode. SIMD is roughly tied
with Pillow, far below the 5× target. Thus the CPU result is good for this
workload, but the SIMD and GPU goals remain open. The remaining GPU cost is
per-call setup, dispatch, completion, and materialized readback after compact
transfers; shader instruction tuning is not the next high-return change.

The first generated LA/RGB/RGBA “full” cases used byte assets sized as
`pixels × channels`. `putdata` counts each byte as a sequence entry and
rejects those inputs before writing, so identical errors on both sides had
looked like parity while `observe-receiver` never ran. Replacing those with
component tuples made the cases valid, but a backend audit then found that
tuples take the callback-preserving `putdata_value_at` path and do not queue
`PipelineOp::PutData`. The tuple cases remain CPU-only binding-semantic checks.
The backend cohort now uses exact integer lists to queue full LA/RGB/RGBA
payloads and a partial RGB prefix, plus exact-length byte payloads for mode 1
and P. The partial L case has a 1,257-sample prefix over a 1,273-pixel image;
its final packed word replaces one byte and preserves the other lanes. All
cases observe receiver `tobytes` output.

Strict exact parity passed 11/11 CPU cases: eight queued operations recorded
actual CPU execution, while the three tuple checks were correctly classified
as having no deferred backend work. SIMD and GPU each passed all 8/8 queued
cases. Their execution sidecars show eight terminal-complete receipts from
the requested backend and no fallback. Receipts are
`putdata-final-{cpu,simd,gpu}-parity.json` with matching
`putdata-final-{cpu,simd,gpu}-execution.json` sidecars. Input generation and
contract checks passed with `make migration-parity-inputs` and
`make migration-parity-inputs-check`.

The corrected GPU resource estimate counts the packed L auxiliary payload as
four samples per `u32` and adds no aligned arena range for empty data. Its
focused unit test covers full and partial packed lengths, the generic channel
layout, and empty input. The final matched benchmark receipt is
`putdata-final-benchmark.json` (run
`migration-benchmark-0805acd7b6c1404fac8eef1ee4b559a6`) with its exact-output
gate `putdata-final-benchmark-parity.json`; it records 100 actual samples per
subject on the same 1,024 × 768 material-L input.

The reproducible benchmark command shape is:

```sh
MIGRATION_BENCHMARK_PROFILE=standard \
MIGRATION_BENCHMARK_ARGS='--workload-id pil-image-image.putdata.standard' \
MIGRATION_BENCHMARK_OUTPUT=build/migration-parity/putdata-final-benchmark.json \
MIGRATION_BENCHMARK_PARITY_OUTPUT=build/migration-parity/putdata-final-benchmark-parity.json \
make migration-parity-benchmark
```

The final source-confirmation run measures CPU at 1.251 ms, about 4% faster
than Pillow at 1.306 ms. SIMD is effectively at parity with Pillow at 1.250
ms, far short of the 5× goal. GPU is about 2× slower than both SIMD and Pillow
at 2.535 ms. Its per-call reciprocal-latency throughput is 394 operations/s,
versus about 800 for CPU and SIMD; concurrency-one rates do not establish
sustained throughput. All benchmark correctness gates passed. The isolated
`make build-parity` build preserved the Pillow oracle and built the final
source. The compact resource estimate unit test and strict execution receipts
are from the same source snapshot. Attempt 4 is checkpointed after four bounded
optimization attempts. `putdata` remains incomplete: later work must target
SIMD lookup/copy behavior and remove GPU per-call device round-trip cost.
## PIL.ImageOps.mirror checkpoint — 2026-09-28

The measured workload is
`pil-imageops.mirror.materialized-rgb-noise-1024x768`, generated from
`PIL.ImageOps.mirror.nuanced.performance-material-rgb-noise-1024x768`. It uses
seeded varied RGB bytes, mirrors once, then calls `tobytes`; input construction
is outside the timed boundary. The standard policy is warm release, five
warmups, 20 iterations × five samples (100 observations per subject), and
concurrency one. The focused cohort also covers default behavior, L/LA/RGBA,
odd-width L/LA/RGB/RGBA, and the materialized pipeline smoke case.

The final benchmark is `build/migration-parity/mirror-final-benchmark.json`,
run `migration-benchmark-71fae23e8b124d729a42668b8947d3e6`; its exact-output
gate is `mirror-final-benchmark-parity.json`, evidence
`migration-parity-benchmark-gate-e37098511c924be09b2071e35d7e3cce`. The exact
command uses `MIGRATION_BENCHMARK_PROFILE=standard` and
`MIGRATION_BENCHMARK_ARGS='--workload-id pil-imageops.mirror.materialized-rgb-noise-1024x768'`
with `make migration-parity-benchmark`. The receipt records 100/100 actual
CPU, SIMD, and GPU executions with no fallback; GPU completed one dispatch per
sample. Strict final parity outputs are
`mirror-final-{cpu,simd,gpu}.json`, each 11/11. All receipts are from the dirty
worktree at base revision `85c3acb6d5d67d0f4c1ed9a5210d35c11bc1fd25`; their
manifest and parity input hashes match the generated case. Treat the figures
as evidence for this local source snapshot, not a clean committed revision.

| Subject | Median latency | Reciprocal latency rate | Result |
| --- | ---: | ---: | --- |
| Pillow | 0.873 ms | 1,146 ops/s | reference |
| CPU | 0.227 ms | 4,398 ops/s | 3.84× faster than Pillow |
| SIMD | 0.230 ms | 4,352 ops/s | 3.80× faster; below 5× goal |
| GPU | 0.955 ms | 1,047 ops/s | 4.16× slower than SIMD |

These rates are reciprocals of concurrency-one latency, not sustained request
throughput. GPU backend time still includes synchronous submission, completion,
and materialized readback; the shader's device time is not isolated.

Four bounded implementations were tried. The first raw-row version called
`copy_from_slice` once per small pixel and regressed CPU latency to 2.391 ms.
Its diagnostic sample showed hundreds of `_platform_memmove`/`memcpy` frames:
removing generic pixel access does not help if it replaces it with one tiny
copy call per pixel. The retained CPU path specializes 1/2/3/4-channel rows
and writes each channel directly, after `CheckedDims` validates the allocation.
This cut the matched 1024 × 768 CPU median from 1.314 ms to 0.213 ms. Typed,
empty, unsupported, or invalid layouts keep the original `fliph` path; whole
pixel groups preserve LA alpha and RGB/RGBA channel order.

The second SIMD attempt parallelized independent rows at a 256 KiB threshold.
It regressed SIMD median latency from 0.201 ms to 0.362 ms, so the parallel
branch was removed. For this light row kernel, scheduler overhead outweighed
the work saved; the next SIMD investigation should reduce the RGB shuffle or
copy cost before adding workers.

The fourth attempt admits only a single RGB8 Mirror in `None`/`RGB` mode on
little-endian targets and reuses the packed RGB transpose shader's
`FlipLeftRight` mapping. This avoids RGB↔RGBA expansion and reduces upload and
readback from 3,145,728 bytes each to 2,359,296 bytes each; the final receipt
reports zero full-frame copies, zero mode conversions, and one dispatch. GPU
median improved from 1.501 ms on the same workload before this path to 0.955
ms, but remains over four times slower than SIMD. Resident device data or a
batched host-visible API is the next GPU-sized opportunity; the remaining
round-trip floor cannot be solved by shader instruction tuning alone.

Mirror is checkpointed incomplete after four attempts. CPU meets its latency
goal on this workload; SIMD misses the 5× target, and GPU misses the SIMD
latency and throughput goals. The ranked matrix must retain those blockers and
revisit Mirror after other operations receive their first optimization pass.

## Native-format conversion audit and GetChannel checkpoint — 2026-09-28

The source scan `rg -n 'to_rgba8\\(|into_rgba8\\(' pillow-rs/src` found 84
textual call/declaration sites, including tests and the `DynamicImage`
conversion helpers. These are not 84 runtime image conversions. The Python
Qt bridge has two explicit `convert("RGBA")` calls; JavaScript has none in
the binding source. The ledger below separates runtime operations from helper
definitions, wrappers, test-only matches, comments, and a one-pixel color
helper. SIMD has two runtime conversions in P/PA result normalization; its
image kernels otherwise operate on native bytes. Runtime CPU call sites are concentrated in
[`pool_cpu/ops`](../pillow-rs/src/compute/pool_cpu/ops):

| Caller | Native-format opportunity and semantic boundary |
| --- | --- |
| [`imageops.rs`](../pillow-rs/src/compute/pool_cpu/ops/imageops.rs): Pad, Expand | Pad and Expand CPU paths admit exact L/LA/RGB/HSV/RGBA storage. Expand also has native three-byte SIMD rows and a guarded GPU native-input/native-output path when the adapter's bounded word grid fits. Keep P/PA tuple-index semantics and CMYK's four active samples. |
| [`enhance.rs`](../pillow-rs/src/compute/pool_cpu/ops/enhance.rs): Brightness, Sharpness | LA Brightness now scales byte 0 directly and preserves byte 1 on CPU; its one-op GPU path transfers native LA bytes. Sharpness still widens and repeats luma work before restoring alpha. CMYK's fourth stored byte is K, unlike RGBA's alpha. |
| [`effects.rs`](../pillow-rs/src/compute/pool_cpu/ops/effects.rs): Spread, Paste, Composite, Eval, PutData, PutAlpha | Spread now borrows native 1/2/3/4-byte storage for relocation, preserving raw CMYK/RGBX/RGBa/I/F samples; typed fallbacks retain their existing numeric conversion. Avoid widening for same-layout copies, validated channel extraction, and LA alpha replacement. Keep mixed-mode paste/composite conversions where blending semantics require them; RGB→RGBA and explicit alpha composition change output semantics. |
| [`filter.rs`](../pillow-rs/src/compute/pool_cpu/ops/filter.rs), [`geometry.rs`](../pillow-rs/src/compute/pool_cpu/ops/geometry.rs) | I-mode Filter3x3 now borrows the matching four-byte scalar carrier. Filter5x5 I and F rank-filter paths still have same-size accessor copies; operate on raw/typed samples without treating them as color channels. |
| [`color.rs`](../pillow-rs/src/compute/pool_cpu/ops/color.rs), [`draw.rs`](../pillow-rs/src/compute/pool_cpu/ops/draw.rs) | Explicit `convert(..., "RGBA")` and fallback drawing canvases have a canonical RGBA output contract. Avoid only after proving the caller's requested format and palette/alpha semantics allow it. |

Additional references occur in `color.rs`, `ops/analysis.rs`, `ops/convert.rs`,
`ops/pil_resize.rs`, `ops/quantize.rs`, `image.rs` (`preserve_mode`), and
`raster/dynamic.rs` (the conversion API itself). In GPU code,
[`pool_gpu/mod.rs`](../pillow-rs/src/compute/pool_gpu/mod.rs) contains generic
image upload through RGBA, auxiliary-image packing, and output readback helpers.
The generic input path widens L/LA/RGB to four bytes, except for guarded
operation-specific paths such as one-op LA Brightness, ExtractBand, and
Expand; typed I/F and 16-bit inputs must stay on their typed contracts.
Four-byte storage is not necessarily
RGBA: CMYK's fourth byte is K, RGBX's fourth is padding, RGBa is premultiplied,
and I/F are scalar samples. LA alpha is byte 1; RGBA alpha is byte 3. These
distinctions rule out a universal raw four-byte path.

### GetChannel: native input bytes for the one-op GPU path

The pure GPU `ExtractBand` batch now uploads the source's native 1/2/3/4-byte
L/LA/RGB/RGBA storage and selects channel bytes in the shader. The compact L
output and one-dispatch schedule are unchanged. LA selects byte 1 in this
native layout; the generic multi-op RGBA transport retains its existing LA
alpha-in-byte-3 contract. The fast path checks the physical image variant,
logical mode, selected band, checked byte count, and little-endian packed-word
layout; typed or mismatched representations keep the existing generic route.
This removes the temporary host expansion and cuts GPU input transfer for L,
RGB, and LA. It does not hide GPU launch, synchronization, or readback latency.

The added `L 1024 × 768` workload complements RGB, LA, and RGBA at the same
size. Before/after receipts are
`getchannel-native-{before,after}.json` (100 samples per subject, five warmups,
20 iterations × five samples, materialized output). Exact parity gates passed
for all four measured workloads. Focused strict parity also passed 31/31 cases
on each CPU, SIMD, and GPU backend; the GPU Rust tests cover L/LA/RGB/RGBA and
4096 × 4096 LA extraction. No coverage collection was run.

| Mode, 1024 × 768 | Pillow ms | CPU ms | SIMD ms | GPU before → after ms | GPU upload before → after |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 0.067709 → 0.067583 | 0.044500 → 0.048292 | 0.054541 → 0.052750 | 0.878584 → 0.412354 | 3,145,728 → 786,432 B |
| RGB | 0.132667 → 0.139438 | 0.051854 → 0.055751 | 0.106230 → 0.105251 | 0.729396 → 0.580605 | 3,145,728 → 2,359,296 B |
| LA | 0.134375 → 0.139125 | 0.091250 → 0.095312 | 0.104438 → 0.104646 | 0.965709 → 0.520500 | 3,145,728 → 1,572,864 B |
| RGBA | 0.134604 → 0.138729 | 0.074541 → 0.089292 | 0.095834 → 0.101021 | 0.635792 → 0.641042 | 3,145,728 → 3,145,728 B |

All four CPU medians remain faster than Pillow. SIMD is slower than Pillow on
all four and misses the 5× target. GPU latency improved by 53% for L, 20% for
RGB, and 46% for LA, but did not improve RGBA. It remains 4.2×–7.8× slower than
SIMD. The after receipt reports actual GPU execution for 100/100 samples, one
dispatch, no fallback, zero mode conversions, and native input bytes. GPU
backend time still dominates the 0.4–0.64 ms calls; the remaining opportunity
is reducing round-trip/synchronization or batching requests, not changing the
channel-selection arithmetic. These are concurrency-one reciprocal-latency
rates, not sustained throughput.

GetChannel is checkpointed incomplete. The retained attempt removes avoidable
GPU input widening and has strict parity, but no backend meets the full targets
on every selected mode. The Expand checkpoint below addresses the next
native-layout opportunity. The broader scan also exposed
[`pil_resize.rs`](../pillow-rs/src/ops/pil_resize.rs)'s `pixel_at` fallback:
nearest resize can materialize a full RGBA copy while sampling output pixels.
It is reachable only for typed `DynamicImage` variants that current public
constructors and decoder lanes do not produce, so it lacks a public parity and
benchmark route. Unmasked, same-mode RGB Paste is now checkpointed with a
native CPU row-copy route. Pad was the next reachable target and now has native
CPU L/LA/RGB/HSV/RGBA paths plus a direct SIMD full-width vertical-pad route.
Its remaining gaps are SIMD versus the 5× goal and GPU's four-byte transport.
Expand's native CPU/SIMD/GPU paths now cover L/LA/RGB/HSV/RGBA. The HSV
follow-up and compact native GPU output are recorded below.

### Explicit RGBA callsite ledger — 2026-09-28

The current `rg -n 'to_rgba8\(|into_rgba8\(' pillow-rs` scan finds 84 Rust
source matches: 74 runtime operation calls, one GPU test, two wrapper
delegations, one single-color conversion, two conversion method definitions,
and four comments. The Python Qt bridge has two explicit `convert("RGBA")`
calls at `pillow-rs-py/python/pillow_rs/image.py:655,663`; the JavaScript
binding has none. The table lists every Rust match, grouped by what the call
does. “Widening-capable” includes fallback sites that widen L/LA/RGB but only
copy an existing four-byte carrier for other modes.

| Classification | Count | Rust callsites |
| --- | ---: | --- |
| Widening-capable or mixed-format fallback | 38 | `compute/pool_cpu/ops/color.rs:279`; `compute/pool_cpu/ops/draw.rs:43`; `compute/pool_cpu/ops/effects.rs:176,772,848,849,862,1197,1516,3363`; `compute/pool_cpu/ops/enhance.rs:332`; `compute/pool_cpu/ops/filter.rs:370,535`; `compute/pool_cpu/ops/imageops.rs:1281,1574`; `compute/pool_gpu/mod.rs:3748,3769,4187,7856,7876,10895`; `compute/pool_simd/mod.rs:143,158`; `draw/mod.rs:1151,1462,2135`; `image.rs:3833,3848,5383,5416,6082,6427,6575`; `ops/analysis.rs:323,401,588`; `ops/pil_resize.rs:272`; `ops/quantize.rs:2317` |
| Same-layout clone or four-byte reinterpretation | 22 | `color.rs:353,776,1082,1099,1118,1134,1146,1158`; `compute/pool_cpu/ops/effects.rs:988,1348,1349`; `compute/pool_cpu/ops/enhance.rs:80,138,283`; `compute/pool_cpu/ops/filter.rs:1823`; `draw/mod.rs:1285,1410,2330,2440`; `ops/convert.rs:657,1021`; `ops/quantize.rs:2297` |
| Requested output or mode-restoration conversion | 14 | `compute/pool_cpu/ops/color.rs:55`; `compute/pool_cpu/ops/effects.rs:3305,3383,3456`; `compute/pool_gpu/mod.rs:10733,10860,10872`; `image.rs:7034,7043,7052`; `ops/convert.rs:365,382,701`; `ops/pil_resize.rs:1956` |
| Definition, wrapper, test, comment, or color-only | 10 | `color.rs:118`; `compute/pool_cpu/ops/geometry.rs:304`; `compute/pool_gpu/mod.rs:10556,19327`; `ops/pil_resize.rs:282,2257`; `raster/dynamic.rs:327,420,423,1016` |

Do not treat fallback callsites as guaranteed conversions. `to_rgba8()`
expands L/LA/RGB, clones RGBA, and may copy four-byte storage that actually
means CMYK, RGBX, premultiplied RGBa, I, or F. Those bytes are not interchangeable:
CMYK byte 3 is K, RGBX byte 3 is padding, LA alpha is byte 1, and RGBa stores
premultiplied color. I/F bytes encode scalar samples. The GPU also has a named
RGB upload expansion outside the method-call scan:
`pool_gpu/mod.rs:4169` calls `expand_rgb_into_rgba` (`:4566`) for a shader that
still consumes four-byte pixels. The Qt conversions are host-display formats,
not core image algorithms.

Several common paths already avoid these conversions: native LA Brightness and
PutAlpha, native L/LA/RGB/RGBA/CMYK masked Paste, native RGB bitmap/text
composition, and native CPU/SIMD/GPU Pad and Expand. `ops/pil_resize.rs:272`
still converts a whole typed source inside its per-pixel fallback, but current
public constructors and decoder lanes do not produce the typed layouts that
reach it. `ops/quantize.rs` now keeps logical RGB backed by `ImageRgb8` in its
three-byte layout through FASTOCTREE; other modes and storage variants retain
the conversion fallback until their channel semantics are proven separately.

The standard byte-mode LA `ImageStat.Stat` path already enters `histogram()`
before this RGBA fallback and counts L and alpha at their native byte offsets.
Its remaining match is for unusual typed storage; it is not a reachable LA8
conversion to optimize.

### F boxed nearest resize: retain scalar words and skip identity work — 2026-09-28

`DynamicImage::ImageRgba8` is also the internal four-byte carrier for Pillow
mode F. Those bytes are little-endian `f32` samples, not color channels. The
boxed F resize implementation now borrows `img.as_bytes()` directly, copies
selected four-byte words for nearest sampling, and decodes to `f32` only for
filtered resampling. Filtered accumulation remains in Pillow's existing
f64-kernel/f32-store order. The CPU boxed-nearest router keeps logical mode F
so this format-specific implementation is admitted instead of the generic
byte-channel path.

When source and destination dimensions match, a bounded scan runs Pillow's
float32-narrowed, cumulative nearest-coordinate recurrence on each axis. Only
if every selected index is unchanged does public `Image.resize` return
`Image.copy()`, which shares immutable materialized storage while preserving
copy semantics. The source box is validated before this shortcut. This avoids
both a pixel loop and a new image buffer for fractional boxes that are
pixel-identical.

The parity-backed workload is a deterministic 512 × 512 F image with a
fractional box and nearest resampling. The measured boundary is resize plus
`tobytes()`, five warmups, 20 iterations × five samples, concurrency one.
Strict CPU parity passed 19/19 F boxed-resize cases. Normal SIMD and GPU-requested
parity each passed 19/19; strict SIMD exposed existing unsupported
`reducing_gap` and tall-F routes, which fall back in normal mode. The final
benchmark run passed its live-Pillow correctness gate:

| Subject | Median latency | Median operations/s | Backend evidence |
| --- | ---: | ---: | --- |
| Pillow | 0.236 ms | 4,233 | Pillow |
| CPU request | 0.046 ms | 21,878 | no backend dispatch; shared-copy shortcut |
| SIMD request | 0.046 ms | 21,739 | no backend dispatch; shared-copy shortcut |
| GPU request | 0.036 ms | 31,158 | no backend dispatch; shared-copy shortcut |

These timings establish a 5.1× public-call latency win for the identity-map
workload, not SIMD or GPU kernel speed. The GPU logical-mode gate still rejects
F resize, and non-identity boxed F resize remains a separate performance
blocker. A first direct-byte-only attempt did not beat run-to-run noise; the
measured win came from preserving logical F through CPU routing and recognizing
the exact identity map. No coverage ran.

The audit found and fixed one correctness bug in `Image.getprojection`: LAB is
stored in RGB bytes with logical A/B zero represented by 128. Its old fallback
expanded to RGBA and marked every nonempty LAB pixel because the synthesized
alpha is 255. A native LAB scan now checks `L != 0 || A != 128 || B != 128`.
Four generated cases cover neutral zero and each logical band independently;
the complete 25-case getprojection parity lane passed. L-mode Brightness has
since been moved to native one-byte CPU and GPU paths, with a direct SIMD shift
for exact factors; measurements and the remaining GPU bottleneck are recorded
below. Keep RGB→RGBA conversions when adding alpha is the requested result, and
retain mode-mismatched conversions until their semantics have separate
parity-backed paths.

### RGB FASTOCTREE: keep three-channel pixels native — 2026-09-28

The old RGB method-2 route called `to_rgba8()`, moved the four-byte image into
a raw buffer, then allocated a second `Vec<[u8; 4]>` for octree insertion and
index mapping. The fast route admits only logical mode `None`/`RGB` with
concrete `ImageRgb8`; it reads the three stored samples directly on each pass.
RGB bucket offsets and sums now touch only R/G/B. The alpha-aware path still
normalizes fully transparent pixels to the first transparent RGB and uses the
same four-channel cube; all other modes retain the prior conversion path.
Checked dimensions and buffer length guard the new raw accessor.

Strict CPU parity passed 152/152 FASTOCTREE cases, including RGB distributions,
palette/index tie ordering, the 256 × 256 noisy RGB input, and RGBA transparency
controls. The correctness-gated benchmark measured the public quantize call,
palette retrieval, and output bytes for a 256 × 256 random RGB image quantized
to 32 colors with five warmups and 100 samples. The initial CPU median was
0.660 ms against Pillow's 0.649 ms; after removing the staging allocations but
before specializing the inner loop it was 0.642 ms against 0.618 ms. Three-
channel bucket updates and lookup then measured 0.519 ms CPU against 0.606 ms
Pillow: 21% lower CPU latency than the initial pillow-rs result and 1.17×
Pillow throughput for this workload. The final correctness gate passed. The
three `python-*` benchmark profiles share this eager CPU implementation;
receipts contain no actual backend dispatch counts, so these are not SIMD or
GPU measurements. No coverage was run.

### RGB `ImageDraw.bitmap`: keep the canvas three bytes per pixel — 2026-09-28

The normal RGB drawing context previously entered the shared RGB/RGBA bitmap
branch, widened the canvas, blended the mask, and narrowed the result back to
RGB. A guarded `RGB` logical mode plus concrete `ImageRgb8` path now clones the
materialized destination once and updates native three-byte rows. It handles
mode-1 masks as binary coverage, L masks as coverage, RGBA masks from byte 3,
and RGBa as the fully opaque bitmap mask Pillow uses. Signed clipping is
precomputed; partial coverage uses Pillow's rounded divide-by-255 blend. An
RGBA context over RGB storage stays on the existing path because it has
different compositing semantics.

The parity-backed 1024 × 768 workload measures bitmap plus receiver
materialization. Strict CPU parity passed 7/7 focused cases, including each
mask form and clipped coordinates. CPU median fell from 3.322 ms to 1.390 ms
(2.39× faster than the original implementation and 1.16× faster than Pillow's
1.608 ms). The first native-kernel version was 2.288 ms; removing a second
destination copy and mask materialization brought it to 1.390 ms. SIMD/GPU
profile labels report no actual backend receipt because this public draw call
does not dispatch through those executors; no SIMD/GPU speedup is claimed.

### Masked `Image.paste`: native rows and measured blockers — 2026-09-28

Same-mode L/LA/RGB/RGBA/CMYK paste now blends from the native image rows and
reads only the selected mask band. RGBA masks use byte 3; LA masks use byte 1;
RGBa uses Pillow's premultiplied PREBLEND rule. The CPU loop precomputes clipped
row spans and parallelizes independent destination rows only when the clipped
area reaches 512 × 512 pixels. SIMD's L-to-L case reads its contiguous mask
bytes directly instead of computing a scalar gather for each vector lane. The
general paths remain for mixed modes, palette storage, and unproven layouts.

The standard benchmark uses noisy 1024 × 768 images and measures `paste` plus
receiver `tobytes()`: five warmups, 20 iterations, five samples (100 timed
calls). The final-code receipt is
`paste-masked-native-final.json`; every timed subject reported its requested
backend for all 100 calls. Medians are milliseconds; this is concurrency-one
latency, not sustained throughput.

| Mode | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.203 | 0.319 | 0.439 | 4.597 |
| LA | 0.714 | 0.475 | 1.154 | 5.167 |
| RGB | 0.996 | 0.591 | 1.715 | 4.226 |
| RGBA | 0.893 | 0.603 | 2.263 | 4.131 |
| CMYK | 0.929 | 0.587 | 2.306 | 4.055 |

The first native pixel loop measured 3.455–5.535 ms on CPU. Clipping once and
blending row slices reduced that to 1.126–1.997 ms. Parallel rows then reduced
CPU latency to 0.319–0.603 ms, beating Pillow in four modes; L remains 1.57×
slower. The direct one-byte SIMD mask path improved L from 0.551 to 0.439 ms,
still 2.16× slower than Pillow and far short of the 5× target. SIMD is slower
than Pillow in the other four modes as well. GPU is 1.76×–10.48× slower than
SIMD, so the direct GPU paste path misses the latency goal. This run is slower
than the previous sample across all subjects, consistent with machine/run
variation; keep both receipts when comparing future attempts. Strict parity for
the RGBA-alpha-mask case passed on CPU, SIMD, and GPU; the five measured
workloads passed all 15 backend parity comparisons. These blockers are
checkpointed after four bounded implementations; do not keep changing the same
loop without a new profile showing a different dominant cost.

### Scalar `Image.putalpha` on LA: replace byte 1 in native storage — 2026-09-28

The old CPU LA branch widened two-byte samples to RGBA, replaced alpha, then
allocated LA output. CPU now clones the concrete `ImageLumaA8` and writes the
new alpha to byte 1. SIMD uses the same native two-byte layout and avoids its
generic vector-block staging for this low-arithmetic operation. GPU admits only
a nonempty logical LA image backed by `ImageLumaA8`; its native-byte shader
updates alpha bytes 1 and 3 in each packed word, returns LA directly, and
reports no mode conversion. Upload and readback each fell from 3,145,728 to
1,572,864 bytes for the 1024 × 768 workload. Other modes retain their existing
mode-specific paths.

The standard correctness-gated workload uses noisy 1024 × 768 LA pixels,
alpha 192, and measures `putalpha` plus receiver `tobytes()` at concurrency one
(five warmups, 20 iterations × five samples). Every target reported the
requested CPU/SIMD/GPU backend. The final receipt is
`putalpha-la-checkpoint.json`; an additional strict 5 × 3 LA case verifies the
GPU's final partially occupied packed word. Exact parity passed 3/3 backends
for both the large benchmark and the odd-tail case.

| Subject | Before ms | Final ms | Final effective ops/s |
| --- | ---: | ---: | ---: |
| Pillow | 0.411188 | 0.373750 | 2,676 |
| CPU | 0.294292 | 0.239812 | 4,170 |
| SIMD | 0.647729 | 0.209417 | 4,775 |
| GPU | 2.217021 | 0.669813 | 1,493 |

The final run puts CPU 1.56× and SIMD 1.79× ahead of Pillow. SIMD improved
3.09× against its own baseline. GPU improved 3.31× and now moves half as many
bytes, but remains 3.20× slower than SIMD and 1.79× slower than Pillow; its
single-concurrency reciprocal latency is not sustained-throughput evidence.
The SIMD 5× goal and GPU/SIMD target remain blockers. Stop this LA scalar
specialization after the bounded CPU, GPU, and SIMD paths; reopen it only with
a profile that isolates a new cost.

### Brightness LA: native CPU and GPU processing — 2026-09-28

For logical LA backed by `ImageLumaA8`, brightness scales only each pixel's
luma byte and copies its alpha byte unchanged. The prior CPU path widened each
two-byte pixel to RGBA, multiplied three duplicate luma channels, then narrowed
back to LA. SIMD already used a native two-byte layout. The GPU's generic
transport sent four bytes per pixel in both directions; the new one-op LA path
packs the original sample bytes, transforms the luma lanes, preserves alpha,
and reconstructs `ImageLumaA8` directly. GPU admission requires the LA storage
variant, a supported exact fixed-point factor, nonempty dimensions, and the
existing little-endian packed-word path. Other modes and unsupported factors
keep their established routes.

The parity-gated workload is a deterministic noisy 1024 × 768 LA image at
factor 0.5, with operation plus `tobytes()` in the measured boundary. The
before/after runs each measured 100 samples per subject. Strict parity passed
for CPU, SIMD, and GPU, and the benchmark's three live-Pillow checks passed.
No coverage was run.

| Subject | Before ms | After ms |
| --- | ---: | ---: |
| Pillow | 1.757208 | 1.860917 |
| CPU | 1.390312 | 0.334708 |
| SIMD | 0.450750 | 0.494792 |
| GPU | 2.278500 | 0.756876 |

CPU latency improved 4.15× against its own baseline and is 5.56× faster than
the after-run Pillow median. GPU improved 3.01×. Its upload and readback each
fell from 3,145,728 bytes to 1,572,864 bytes; it still completes one dispatch
with no fallback. These measurements use concurrency one, so reciprocal latency
is not evidence of sustained throughput. The full SIMD target is not met
(3.76× Pillow), and GPU latency remains 1.53× SIMD latency; Brightness stays
checkpointed incomplete until those gaps are addressed.

The mode guard in `Image.getbbox()` now treats only logical RGBA/RGBa byte 3
as alpha. CMYK and RGBX can also use `ImageRgba8`, but Pillow checks all four
stored bands for those modes when `alpha_only=True`; K is ink, and RGBX's fourth
byte participates in this byte-oriented scan. A cyan-only CMYK pixel exposed a
real mismatch: Pillow returned its one-pixel box while pillow-rs returned
`None`. The generated parity inputs retain that C-only regression alongside
K-only CMYK, RGBX, and transparent/visible RGBA controls. The focused five-case
CPU run passes after the logical-mode guard; no coverage was run.

### Native-format attack order

Use this order for reachable operation paths; do not replace all conversions
with a shared RGBA-shaped byte loop. Specialize the operation and mode whose
input, math, and output contracts are proven, then keep the general route for
everything else.

| Rank | Conversion family | First native implementation | Semantic boundary |
| --- | --- | --- | --- |
| 1 | CPU Paste | For exact matching byte layouts with no mask, clone the native destination and copy clipped source rows at that layout's bytes per pixel. | Require logical mode and concrete storage to agree. Masked Paste blends only according to Pillow's selected L or alpha mask band; mixed-mode RGB inputs are converted before queuing and stay on the fallback. |
| 2 | CPU/SIMD Pad | Keep exact native L/LA/RGB/HSV/RGBA storage, borrow the identity-contain source, repeat a fill row, and copy full-width source spans contiguously. | Preserve logical mode with concrete storage checks; LA fill alpha is byte 3 in the color tuple but destination byte 1. SIMD is 3.48× Pillow and GPU still uses four-byte transport on HSV. |
| 3 | CPU/SIMD/GPU Expand | Preserve native L/LA/RGB/HSV/RGBA bytes. Build SIMD 3-byte fill rows once; on GPU, assign each invocation one packed output word and read back the native byte count. | Match logical mode and concrete storage. P/PA remain index-specific; CMYK's fourth byte is K. Use the adapter's real workgroup limits, never one writer per 3-byte pixel. |
| 4 | RGB drawing and read-only analysis | Draw to native RGB storage where the raster primitive supports the same blend; scan requested bands directly for stats, projections, bounds, and data exports. | Preserve antialiasing, masks, palette mapping, and logical band order. Read-only paths should borrow; mutating paths must own their output. |
| 5 | L/LA brightness and related enhancement | For L, scale each native byte; for LA, scale byte 0 and retain byte 1. On SIMD, compare exact byte maps in the adapter's quantized factor domain before building a LUT. | L CPU/SIMD beat Pillow on the measured case; GPU still trails SIMD because transfer, completion, and readback dominate. Sharpness has a four-attempt checkpoint. CMYK's fourth component is K. |
| 6 | GPU input/output staging | Add per-operation native packed layouts when the shader can consume them; measure upload, output, readback, and synchronization separately. | Generic packed RGBA remains shared by many operations. Native RGB readback must handle three-byte pixels spanning 32-bit words; a smaller upload alone is not an end-to-end result. |
| 7 | F boxed nearest resize | Preserve the four-byte scalar words; copy selected words directly and return `Image.copy()` only when both cumulative nearest maps select the same source coordinates. | Validate the narrowed box first. Retain logical F at CPU dispatch; decode to f32 only for filtered resampling and preserve f64 accumulation/f32 stores. The identity workload is 5.1× faster end-to-end but bypasses all backends; GPU F resize remains unsupported. |
| 8 | Remaining typed scalar paths | Keep I/F samples in their native numeric representation instead of treating their four bytes as color channels. | F boxed-nearest identity, ordinary F/I CPU resize, F/I thumbnail reduction, and I Filter3x3 now use their native carrier. Filter5x5 I and F rank-filter still copy through the accessor; preserve exact rounding, byte order, and sample evaluation. |

Treat a four-byte physical buffer as its real format: CMYK's fourth byte is K,
RGBX's is padding, RGBa is premultiplied, LA alpha is byte 1, and I/F are scalar
samples. A raw byte path must gate both the public logical mode and the
`DynamicImage` storage variant; matching only a generic mode code can admit
aliases such as `1`, P, RGBX, or premultiplied RGBa with different semantics.

### Expand: native CPU rows and GPU source bytes — 2026-09-28

The benchmark boundary is public `ImageOps.expand` plus terminal bytes on noisy
1024 × 768 L/LA/RGB/RGBA inputs, border 7, warm cache, 5 warmups, 20 calls × 5
samples, concurrency one. The refreshed run is
`migration-benchmark-f99b7f35a5204291af848ef404d6c36e`, with artifacts
`expand-attempt4-final2-benchmark.json` and `expand-attempt4-final2-parity.json`.
Every measured case uses the `parity_pass` gate.
CPU admits exact L/LA/RGB/RGBA storage and builds each output row once. LA
source alpha is byte 1, while the fill tuple's alpha remains byte 3. P/PA keep
their index-specific path and unmatched layouts keep the RGBA fallback. Checked
dimension arithmetic rejects overflow before allocation.

Attempt 1 built a native-byte canvas but initialized the full destination and
then overwrote its center. It cut CPU L from 0.925 ms to 0.213 ms, but L still
missed Pillow at 0.120 ms. Attempt 2 assembled top/bottom and left/source/right
segments once with `Vec::with_capacity` and row slices. CPU medians then beat
Pillow in every mode: L 0.074 ms, LA 0.155 ms, RGB 0.273 ms, RGBA 0.368 ms.
The useful lesson is to remove redundant initialized bytes before tuning the
copy loop.

Attempt 3 applied the same single-write border layout to SIMD's native byte
path. It improved that backend substantially, but custom per-16-byte lane
construction still did not approach the 5× target. The final attempt added a
little-endian, exact-mode, singleton-GPU route that uploads native input bytes
and decodes those channels in `expand.wgsl`; that four-mode checkpoint kept an
RGBA destination/readback contract. LA alpha is moved from byte 1 to the packed
alpha channel. The exact logical-mode gate rejects aliases and multi-op
batches. The follow-up below replaces the four-byte destination/readback for
supported layouts.

Final correctness-gated medians (ms):

| Mode | Pillow | CPU | SIMD | GPU |
| --- | ---: | ---: | ---: | ---: |
| L | 0.1009 | 0.0761 | 0.0854 | 1.4330 |
| LA | 0.5551 | 0.1371 | 0.1555 | 2.0171 |
| RGB | 0.8045 | 0.2195 | 0.3995 | 1.4516 |
| RGBA | 0.8941 | 0.3568 | 0.3995 | 1.5335 |

`expand-attempt4-final2-parity.json` passes all **12/12** measured backend
comparisons. Receipts show CPU/SIMD/GPU actual execution for 100/100 samples,
no fallback, and one GPU dispatch. GPU input uploads are 786,432 / 1,572,864 /
2,359,296 / 3,145,728 bytes for L/LA/RGB/RGBA; readback remains 3,246,864
bytes per mode. The resource receipt reports zero source mode conversions for
all four native layouts; L/LA still narrow the RGBA readback at the output
boundary, which this input-conversion counter does not measure. Against the
preceding GPU run, native input bytes
reduced latency by roughly 22% L, 12% LA, and 10% RGB; RGBA was unchanged by
the path and its timing varied between runs.

This historical four-mode checkpoint passed its selected parity gate but did
not meet the SIMD or GPU goals. Its remaining four-byte output/readback cost
was addressed by the native-output follow-up below.

### Expand: native HSV rows and compact GPU output — 2026-09-28

The follow-up admits HSV only when the logical mode is HSV and physical storage
is exactly `ImageRgb8`; its three stored samples remain raw HSV bytes through
CPU, SIMD, GPU upload, shader output, and readback. CPU fill bytes follow the
native three-byte row layout. SIMD precomputes the three-byte fill pattern and
assembles each padded row once; per-lane modulo vector construction and
zero-filling then overwriting the full canvas cost more on this workload.
Existing RGB behavior uses the same physical layout without treating HSV as
RGB color math.

The GPU path retains native input packing and adds compact native output for
singleton Expand on L/LA/RGB/HSV/RGBA. It dispatches one writer per packed
32-bit output word, so adjacent 3-byte pixels never race on a shared word. A
2D word grid is planned against the adapter's actual workgroup limit; if the
full word count does not fit safely, the existing generic output path remains
available. The final partial word is padded and trimmed at readback. This
reduces GPU transfer bytes for L/LA/RGB/HSV, while RGBA naturally stays at four
bytes; mode-conversion telemetry is zero on admitted routes.

The correctness-gated 1024 × 768 noisy workloads use border 7, five warmups,
20 calls × five samples, and concurrency one. The final run is
`migration-benchmark-274937dbcc704fbba55d1555ffcb9caa`; benchmark and parity
artifacts are `expand-native-final-all.json` and
`expand-native-final-all-parity.json` under `build/migration-parity/`.

| Mode | Pillow ms | CPU ms | SIMD ms | GPU ms | GPU upload / readback bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 0.111396 | 0.071166 | 0.091208 | 0.850270 | 786,432 / 811,716 |
| LA | 0.553041 | 0.137458 | 0.186562 | 1.216521 | 1,572,864 / 1,623,432 |
| RGB | 0.783312 | 0.203458 | 0.254563 | 1.609917 | 2,359,296 / 2,435,148 |
| RGBA | 0.731375 | 0.276104 | 0.465688 | 1.934437 | 3,145,728 / 3,246,864 |
| HSV | 0.868563 | 0.238667 | 0.229730 | 1.594730 | 2,359,296 / 2,435,148 |

All five strict parity cases passed on each CPU, SIMD, and actual GPU backend;
the live benchmark parity gate passed 15/15 comparisons, with no fallback.
Planner tests cover exact and exceeded grid boundaries, zero/invalid cases,
and prove one owner for every word in a split 2D grid. GPU readback saves 25%
for each 3-byte output compared with the former RGBA transfer. CPU beats Pillow
in all five selected cases, but SIMD reaches only 1.22×, 2.96×, 3.08×, 1.57×,
and 3.78× Pillow speed for L/LA/RGB/RGBA/HSV. GPU remains 4.15×–9.32× slower
than SIMD, so neither accelerator goal is met. Compact transport reduced bytes
but did not remove dispatch, map, and synchronization latency. These are
single-workload latency measurements, not sustained-throughput proof; no
coverage collection ran.

### Paste: native same-mode CPU rows — 2026-09-28

The unmasked path no longer widens exact same-mode source and destination
buffers to RGBA before copying. It admits `1`/L/P in `ImageLuma8`, LA/La/PA in
`ImageLumaA8`, RGB/HSV/YCbCr/LAB in `ImageRgb8`, and RGBA/RGBa/RGBX/CMYK/I/F in
`ImageRgba8`. The explicit mode must match the source's logical mode; the
mode-less route remains limited to RGB. P/PA use index materialization. The
routine checks native byte lengths and strides, clips once with saturating
signed offsets, clones the original destination variant, then copies only the
intersecting row spans. This keeps CMYK K, RGBX padding, premultiplied RGBa, and
I/F scalar words as their original bytes. Masked and mixed-mode paste retain
the general path; their channel and blending rules remain open for a separate
parity-backed change.

The new L and LA inputs use deterministic noisy 1024 × 768 receiver/source
images, an unmasked paste at `[2, 2]` clipped at the right and bottom, and
receiver `tobytes()`. Setup is outside the timed boundary. Runs use warm cache,
5 warmups, 20 iterations × 5 samples, concurrency one, and the live parity
gate. The baseline is `paste-native-before.json`; the final native-row run is
`paste-native-final.json` with parity evidence in
`paste-native-final-parity.json`, all under `build/migration-parity/`.

| Mode | Pillow before → final ms | CPU before → final ms | CPU improvement | Final SIMD ms | Final GPU ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 0.101312 → 0.106791 | 1.969375 → 0.079479 | 24.8× | 0.091166 | 3.022646 |
| LA | 0.292354 → 0.255625 | 2.491167 → 0.152541 | 16.3× | 0.183812 | 3.428791 |

CPU and SIMD beat Pillow in both final workloads. The benchmark parity gate
passed all 6/6 comparisons across CPU, SIMD, and GPU. The focused CPU lane also
passed 14/14 cases for logical modes, negative offsets, clipping, mismatch,
and material L/LA/RGB; strict SIMD and GPU lanes each passed 4/4 selected
material and clipping cases. A Rust regression copies every supported storage
layout byte-for-byte, and the existing RGB regression covers negative clipping
and `i64::MIN`.

The final GPU receipt confirms actual GPU execution, but latency remains
3.0–3.4 ms—roughly 19–33× the measured SIMD latency. The generic GPU path still
expands each L/LA input into four-byte transport and transfers the output back;
this CPU change cannot remove those costs. Treat GPU Paste as an open transport
blocker, not an accelerator win. These concurrency-one rates are latency
measurements, not sustained throughput.

The earlier RGB-only checkpoint remains in the campaign history above. This
checkpoint covers unmasked same-mode CPU copies; masked paste, GPU native input
packing, and the original throughput goal remain open.

### Pad: native CPU L/LA/RGB/HSV/RGBA — 2026-09-28

`op_pad` previously resized to contain, cloned the resized image into RGBA,
filled a four-byte canvas, copied source rows, reconstructed RGBA storage, and
then narrowed back to the requested mode. The CPU native path admits matching
logical and physical L, LA, RGB, HSV, or RGBA layouts. It borrows the original
image when contain dimensions are unchanged, fills an output in the requested
native byte pattern, then copies either the entire vertical source span
contiguously or the clipped horizontal rows. HSV uses the same three-byte
physical layout as RGB but copies and fills raw HSV samples. P/PA keep their
existing index paths; CMYK, RGBX, RGBa, I, F, and other layouts retain the
general route. LA output stores fill as `[fill_luma, fill_alpha]`, while source
LA alpha stays at byte 1.

The benchmark cases use deterministic noisy 1024 × 768 input and pad to
1024 × 1024, which isolates the canvas-and-copy phase from resampling. Setup
is outside the timer; the boundary is `pad` plus `tobytes`, with 5 warmups, 20
iterations × 5 samples, concurrency one, and a parity gate. Baseline run
`migration-benchmark-1450a5d9148247d782dc51e30d8139a9`; native run
`migration-benchmark-b0b5450f636349298977a6e92a84edfc`.

| Mode | CPU before ms | CPU native ms | Pillow ms in final run | CPU speedup vs before | CPU vs Pillow |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 1.416396 | 0.041042 | 0.177646 | 34.5× | 4.33× faster |
| LA | 2.033542 | 0.164209 | 0.811271 | 12.4× | 4.94× faster |
| RGB | 1.381584 | 0.246396 | 1.133417 | 5.61× | 4.60× faster |
| RGBA | 1.129125 | 0.330646 | 0.991271 | 3.41× | 3.00× faster |

The earlier four-mode benchmark gate passed 12/12 comparisons across CPU,
SIMD, and GPU, with 100 samples per subject and no fallback. After the shared
vertical-pad path and HSV case were added, strict parity over all five material
modes plus the named-color HSV case passed 6/6 on each of CPU, SIMD, and GPU.
The HSV benchmark gate passed 3/3 before and after optimization, with each
backend executing all 100 samples. No coverage collection ran.

The useful tuning result was to avoid a per-pixel fill loop and per-row
parallel copies for the common vertical pad: repeated fill-row initialization
plus one contiguous source copy reduced the L case from 0.215 ms to 0.041 ms.
Borrowing the identity-contain source also avoids an otherwise redundant full
source clone. Keep the row-wise path for horizontal pads and preserve the
mode-and-storage gate so CMYK's K byte and scalar I/F words never become color
channels.

The HSV follow-up added a 1024 × 768 native-three-byte noise input, padded to
1024 × 1024 with a distinct HSV fill tuple. Baseline run
`migration-benchmark-a25da0c8059d44fd97ca1fdee3b18f83` and final run
`migration-benchmark-dc00396a3f204ba58165dd5446ce4a00` used the same 100-sample
`pad` plus `tobytes` boundary. The baseline exposed two separate costs: CPU's
RGBA round trip, and SIMD's per-lane fill construction plus a redundant source
clone. The SIMD path now checks the identity-contain/full-width case before
materializing a resized image, repeats one native fill row, and copies source
bytes directly into their contiguous destination span.

| Backend | Before ms | Final ms | Speedup vs before | Final vs Pillow |
| --- | ---: | ---: | ---: | ---: |
| Pillow | 1.236042 | 1.128271 | — | reference |
| CPU | 1.271271 | 0.253833 | 5.01× | 4.45× faster |
| SIMD | 2.232562 | 0.324438 | 6.88× | 3.48× faster |
| GPU | 1.718271 | 1.701646 | 1.01× | 1.51× slower |

All three backends reported their requested backend for all 100 samples, and
the correctness gate passed 3/3 before and after the change. Strict parity for
the existing named-color HSV case also passed 1/1 on CPU, SIMD, and GPU. CPU
now beats Pillow, but SIMD is still short of the 5× target. GPU still expands
HSV into its four-byte transport and reads back 4 MiB for a 3 MiB output; it
also records three dispatches. A native three-byte GPU Pad result path remains
the blocker to matching SIMD latency and improving throughput.

### ImageDraw.text: native RGB glyph compositing — 2026-09-28

The RGB text fast path now blends the FreeType mask into `ImageRgb8` directly.
For monochrome glyphs it reads one mask coverage byte and applies the fill RGB
to three destination bytes. For color glyphs it reads source RGB and uses the
mask's fourth byte as coverage. Both preserve the existing rounded
`(src * coverage + dst * (255 - coverage) + 127) / 255` arithmetic and signed
clipping. The route requires logical RGB, physical `ImageRgb8`, and no explicit
RGBA-on-RGB blend context; other layouts retain their existing compositor.
This removes the full-frame RGB→RGBA widening/narrowing and avoids constructing
an RGBA buffer for grayscale glyph masks.

The measured case is deterministic noisy RGB at 1024 × 768, DejaVuSans at
20 px, drawing text and observing `tobytes()` in the timed boundary. Font/image
setup is excluded; the correctness-gated benchmark has five warmups, 20 calls ×
5 samples, and concurrency one. Baseline run
`migration-benchmark-2dd005bcebcd49c69faab532b2ddaf85`; native run
`migration-benchmark-7d8baab35a0642f6adffdc5f416848b0`; both use the same input
hash and policy.

| Subject | Baseline ms | Native RGB ms |
| --- | ---: | ---: |
| Pillow | 0.669729 | 0.676375 |
| CPU | 0.813563 | 0.408104 |
| SIMD-labeled profile | 0.848104 | 0.450542 |
| GPU-labeled profile | 0.961375 | 0.431084 |

The CPU path improved about 2.0× and is 1.66× faster than Pillow on this
workload. The focused CPU parity lane passed 6/6 cases, including the
material-sized RGB input, monochrome and embedded-color glyphs, clipping, and
RGBA fallback. The benchmark parity gate passed 3/3 profile comparisons. The
SIMD and GPU figures are profile timings only: receipts had `actual_backend =
null`, so neither accelerator is proven to execute text drawing. The current
text path rasterizes and composites on the host and does not enqueue a text
pipeline operation; the GPU draw route also copies a host-rendered preview. GPU
text remains a structural backend blocker until mask compositing is represented
as device work. Do not use these profile timings to claim SIMD or GPU speedup.

The useful kernel rule is to preserve the coverage mask in its narrowest form:
grayscale glyphs need one coverage byte per pixel, while color glyphs need RGB
source bytes plus alpha coverage. Expand only the channels the destination
stores, and only after proving the logical mode and concrete raster layout.

### Brightness L: preserve one-byte storage across backends — 2026-09-28

The native route is admitted only for `ImageLuma8` with logical mode absent or
`L`. CPU clones that native image once and applies Pillow-compatible `f64`
multiply, clamp, and truncation directly to its bytes. GPU uses the existing
channel-aware brightness shader with one active channel and constructs L from
the native readback; the old four-byte transport is no longer needed. SIMD
already kept L native, but its fixed-point adapter built and shuffled a 256-byte
LUT for each operation. In that adapter's effective factor domain, factors
0.5, 0.25, and 0.125 map exactly to byte shifts by 1, 2, and 3. The L-only
shortcut applies that shift directly and leaves every other factor and format
on the LUT path. Its proof domain is the effective fixed-point factor, not an
assumption that arbitrary floating-point multiplication equals a shift.

The correctness workload uses deterministic noisy 1024 × 768 L input at
factor 0.5, and measures `enhance` plus `tobytes`. The standard run used five
warmups, 20 iterations × five samples, concurrency one, and 100 measured calls
per subject. Baseline run `migration-benchmark-5e0e6e159b6f4d9f99fccac499cd5e51`
used the generic CPU/GPU routes. Native CPU/GPU run
`migration-benchmark-9ff26cf3757b4e9b9790fb7dd050618b` introduced the native
layout. Final run `migration-benchmark-7a47df30ee094af5b8232894e0070068`
measured the SIMD shift shortcut. Input pixel hashes matched across the three
runs. The final Pillow median moved between runs, so compare each target to the
Pillow median from its own run.

| Subject | Generic baseline ms | Native-format ms | Final ms | Final median ops/s |
| --- | ---: | ---: | ---: | ---: |
| Pillow | 0.474854 | 0.458292 | 0.465125 | 2,150 |
| CPU | 0.943292 | 0.245167 | 0.240854 | 4,152 |
| SIMD | 0.240792 | 0.230687 | 0.082104 | 12,180 |
| GPU | 1.871250 | 0.564062 | 0.514479 | 1,944 |

The final receipt confirms actual CPU, SIMD, and GPU execution for all 100
samples per backend. Strict parity passed the large factor-0.5 workload on all
three backends. A separate 257-byte L input covers every byte value plus a
vector tail at factors 0.5, 0.25, and 0.125; all three factor cases passed on
CPU, SIMD, and GPU (9/9), with no coverage run. Together, the four focused
cases passed 12/12 backend comparisons.

CPU improved 3.92× over its generic baseline and is 1.93× faster than Pillow in
the final run. SIMD is 2.93× faster than its generic baseline and 5.66× faster
than final-run Pillow, meeting the 5× SIMD target for this operation and input.
GPU improved 3.64× over baseline but remains 1.11× slower than Pillow and 6.27×
slower than SIMD. The GPU sends 786,432 bytes each way, makes one dispatch, and
reports no mode conversion; its measured backend phase is still about 0.48 ms.
The native representation removed wasted bandwidth, but device completion and
readback now set the latency floor. Do not infer sustained throughput from this
single-concurrency run. This operation is checkpointed: CPU and SIMD goals are
met on the measured L case; GPU parity and native-byte reduction are proven,
while GPU/SIMD latency and throughput remain open blockers.

### F/I `Image.resize`: borrow the scalar carrier instead of cloning RGBA — 2026-09-28

The source inventory identified three misleading `to_rgba8()` calls in the
ordinary F resize, ordinary I resize, and I `reducing_gap` boxed-resize paths.
These modes are held internally in `ImageRgba8`, but their four bytes encode a
single little-endian `f32` or `i32`; they are not color channels. The accessor
therefore made a redundant full-frame clone before the existing typed decode.
The CPU paths now match the concrete four-byte storage, borrow its raw bytes,
and decode sample words directly into the same typed vectors used by the
resampler. The interpolation order, coefficient precision, integer rounding,
and output packing are unchanged. I resize exits for empty output and exact
identity before allocating the decoded sample vector.

Focused regression workloads resize deterministic noisy 1024 × 768 F and I
images to 512 × 384 with bicubic filtering, and measure public `resize` plus
`tobytes()`. The I `reducing_gap=2` parity case covers the boxed route. Each
benchmark uses five warmups and 100 timed samples at concurrency one; final
benchmark correctness gates passed separately for CPU, SIMD, and GPU; each
backend reports 100/100 actual executions without fallback. Public CPU parity
passed both new noisy F/I cases and the existing I reducing-gap case. The
parity-input generator check also passed (16,868 parity cases and 857 benchmark
workloads); no coverage was run.

| Mode | Run | Pillow ms | CPU ms | SIMD ms | GPU ms |
| --- | --- | ---: | ---: | ---: | ---: |
| F | Before → final | 3.006313 → 2.735937 | 1.412354 → 0.848292 | 6.277125 → 6.158500 | 128.164271 → 126.657229 |
| I | Before → final | 2.031396 → 1.965104 | 1.259646 → 0.856751 | 4.717541 → 4.759146 | 36.887251 → 36.727854 |

The before/final runs use the same generated pixel assets and resize recipes.
F CPU improved about 40% in absolute latency and is 3.22× faster than the
final-run Pillow median. I CPU improved about 32% and is 2.29× faster than
final-run Pillow. SIMD remains slower than
Pillow in both modes, and GPU is much slower than SIMD; these changes affect
the CPU scalar routes only. Treat those misses as separate backend blockers,
not as evidence that eliminating a redundant carrier clone helps those
executors. These are concurrency-one latency measurements, not sustained
throughput. The conversion audit has removed five same-layout clone sites across typed
resize and thumbnail reduction; scalar samples are never treated as RGBA
channels.


### F/I Image.thumbnail: borrow scalar storage during reducing-gap prepass — 2026-09-28

F and I thumbnail reduction used the RGBA accessor only to clone the existing
four-byte scalar carrier before reading every sample with get_pixel. The
reducer now admits only the matching ImageRgba8 carrier and borrows that image
directly. It still decodes each little-endian word as one f32 or i32 sample,
preserving F's float pair/quartet addition order and I's wrapping 32-bit
pair/quartet arithmetic before double accumulation. The following resample
stage and output packing are unchanged. This removes a full source buffer
clone without changing color or channel semantics.

The material workload uses noisy 1024 × 768 F/I sources, thumbnail to a
256 × 256 bound with bicubic resampling and reducing_gap=2, and observes the
mutated image through materialization. It uses five warmups and 100 measured
calls per subject. Both benchmark parity gates passed for Pillow, CPU, SIMD,
and GPU-requested profiles. CPU and SIMD each executed 100/100 calls; GPU
requests fell back to CPU on 100/100 because typed reducing-gap arithmetic
is not proven in the GPU executor. Separate 17 × 13 F/I CPU parity cases cover
partial right and bottom reduction blocks. No coverage ran.

| Mode | Run | Pillow ms | CPU ms | SIMD ms | GPU-requested ms | GPU evidence |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| F | Before → borrowed storage | 1.009000 → 1.085291 | 1.570292 → 1.533416 | 1.644062 → 1.836729 | 2.361104 → 2.397146 | CPU fallback, 100/100 |
| I | Before → borrowed storage | 0.871750 → 0.883000 | 1.820042 → 1.679688 | 1.612125 → 1.663687 | 2.420084 → 2.393000 | CPU fallback, 100/100 |

CPU latency fell about 2% for F and 8% for I versus the initial run, while
Pillow and SIMD medians also moved between runs. Those gains are small and do
not establish a robust speedup beyond noise. CPU remains 1.4× slower than
Pillow for F and 1.9× slower for I in the final run; SIMD remains slower than
Pillow, and the GPU-requested profile never reaches GPU execution. The source
clone is removed, but the operation stays checkpointed with these backend
blockers. Further work should profile reduction versus resampling and terminal
materialization before another implementation attempt.

### EffectSpread: native-byte relocation and exact distance-one fast path — 2026-09-28

The source inventory found that CPU spread converted or cloned the source to
L, LA, RGB, or RGBA before relocating complete samples. Relocation does not
interpret color channels, so the CPU path now borrows native ImageLuma8,
ImageLumaA8, ImageRgb8, and ImageRgba8 storage and copies each selected pixel's
full native sample group. Four-byte ImageRgba8 storage remains raw bytes for
RGBA, RGBa, CMYK, RGBX, I, and F. LA16 and other typed layouts retain their
existing conversion because their numeric narrowing/output contracts differ.

Distance one has a stronger exact simplification: `rand() % 1` always returns
zero, so the result is an image copy. The process-global Park–Miller stream
still must advance twice per pixel because later spread/noise calls share it.
`DarwinRand::advance` applies the generator's multiplicative recurrence with
modular exponentiation, preserving that state in O(log(pixel_count)) work.
CPU and SIMD return the native image copy after advancing the stream. A
singleton GPU request now does the same before auxiliary-map creation, so it
skips the identity-map allocation, upload, shader, and readback while recording
zero dispatches; mixed GPU batches and nonzero distances retain the existing
map-and-shader path. A Rust regression compares jump-ahead with sequential
draws at six lengths, including 1,000,003 values.

Seven new parity inputs use varied 3 × 2 pixels in modes 1, L, LA, P, RGB, RGBA,
and CMYK at distance one. Since the displacement is exactly zero, the byte
observations remain stable even when the lazy image handle is observed too;
the test verifies every band's stored order and the returned mode. The focused
spread lane passed 21/21 cases on CPU, 21/21 on strict SIMD, and 21/21 on strict
GPU, covering the existing mode, palette, single-pixel and nonzero-distance
inputs alongside the new layouts. GPU used the selected backend with no
fallback; the seven distance-one requests produced zero shader dispatches. The
focused Rust jump-ahead test passed. No coverage was run.

The maintained 1024 × 768 RGB pipeline workload uses distance one and measures
one warmup plus six timed calls per subject. Its benchmark correctness gate is
`successful_execution`; parity is reported separately above. Before the
identity path, medians were Pillow 9.722 ms, CPU 9.112 ms, SIMD 8.864 ms, and
GPU 8.029 ms. Three exact-source reruns measured six calls per subject after
one warmup. Taking the median of the three run medians gives Pillow 10.314 ms,
CPU 0.544 ms, SIMD 0.359 ms, and GPU 0.379 ms. All GPU samples used the GPU
backend without fallback and recorded zero dispatches. The CPU and SIMD results
are 19.0× and 28.7× faster than Pillow; GPU remains 5.8% slower than SIMD, so
this pass narrows but does not close the latency gap. These are
single-concurrency reciprocal-latency measurements for the distance-one
identity case, not sustained-throughput evidence or a result for nonzero
spread distances. The source-borrow gather path has parity coverage, but its
standalone latency benefit is not isolated by this workload. The remaining
conversion and GPU/SIMD parity gaps stay visible in the native-format attack
order.

### I-mode Filter3x3: borrow native scalar storage — 2026-09-28

I pixels use the `ImageRgba8` carrier as one little-endian i32 word per pixel.
The I-mode Filter3x3 CPU path used `to_rgba8()` only to clone those same bytes,
then cloned them again to seed the output's unmodified borders. It now borrows
the raw bytes from a matching `ImageRgba8` and allocates only the output buffer;
the old accessor remains a fallback for a mismatched internal variant. The
SIMD and GPU implementations are unchanged.

The existing `PIL.Image.Image.filter.nuanced.i-mode-find-edges-negative` parity
case passed 1/1 on CPU, strict SIMD, and strict GPU. The retained 1024 × 768
I-mode convolution benchmark is `pipeline-chain.convolution-i.3x3-1024x768`;
it measures one warmup and six timed calls, and its benchmark correctness gate
is `successful_execution` (parity is reported separately). One pre-change run
measured Pillow 2.134 ms, CPU 2.894 ms, SIMD 1.617 ms, and GPU 1.405 ms. Three
post-change runs had median-of-run medians of Pillow 2.036 ms, CPU 2.623 ms,
SIMD 1.563 ms, and GPU 1.325 ms. The CPU run medians varied from 1.672 to
2.741 ms, so the single baseline does not establish a robust latency gain; CPU
remains 1.29× slower than Pillow on the post-change median. The source clone
is removed for the concrete I carrier, but CPU still misses its speed goal.
SIMD/GPU did not change, and those noisy benchmark deltas are not attributed to
this edit. No coverage was run.

### I-mode Filter5x5: borrow native scalar storage — 2026-09-28

I pixels use the `ImageRgba8` carrier as one little-endian i32 word per pixel.
The Filter5x5 CPU path previously called `to_rgba8()` to clone those bytes, then
cloned the source again to preserve the output border. It now borrows the raw
bytes for the matching carrier and makes only the output copy; the old accessor
remains the fallback for a mismatched internal variant. The SIMD and GPU
implementations are unchanged. The existing
`PIL.Image.Image.filter.nuanced.i-mode-smooth-more-fused-row` case passed 1/1
on CPU, strict SIMD, and strict GPU after this change.

The benchmark `pipeline-chain.convolution-i.5x5-1024x768` measures the full
workflow with one warmup, six timed samples, warm cache, and concurrency one;
its benchmark gate is `successful_execution`. A paired baseline and native
run are `migration-benchmark-ea296922185940d2b26bef9be63f06c3` and
`migration-benchmark-c33bdce8d3aa4e2caaaec5b55a0ac0ea`:

| Subject | Baseline ms | Native-byte ms |
| --- | ---: | ---: |
| Pillow | 4.042438 | 3.851375 |
| CPU | 4.987584 | 4.678917 |
| SIMD | 3.189500 | 3.206917 |
| GPU | 1.621291 | 1.800271 |

CPU latency improves 6.2% in this paired sample and whole-workflow throughput
rises from 200.5 to 213.7 operations/s. CPU remains 1.21× slower than Pillow,
so this copy reduction does not close the CPU target. The SIMD/GPU implementation
was not changed; their small independent deltas are measurement variation, not
effects attributed to this edit. The per-callsite ledger above now classifies
this accessor as a guarded fallback and the I-mode rank-filter accessor as the
remaining same-layout filter site. No coverage was run.
