# Benchmarking protocol

This is the contributor guide for collecting and interpreting measurements.
For comparisons, start with [benchmark results](https://appunni-m.github.io/pillow-rs/benchmarks/).
The [Pillow-SIMD x86 comparison](PILLOW_SIMD_BENCHMARKS.md) is a separate
version-matched cohort for the operations where the repository has suitable
full-size, parity-backed workloads.

This page defines the benchmark contract used by pillow-rs. It separates
correctness, timing, backend, and resource evidence so a fast but incorrect or
partially executed workload cannot look like a performance result.

## What is measured

The indexed workloads live in
`pillow-rs/tests/fixtures/inputs/benchmark/` and are generated from
`pillow-rs/tests/fixtures/manifest.yaml`. A workload names its public
requirement, input case or workflow, target subjects, measurement boundary,
cache state, and repeat policy. The reference standard policy is:

| Field | Value |
| --- | --- |
| warmup iterations | 5 |
| measurement iterations | 20 |
| samples | 5 (100 timed executions) |
| concurrency | 1 |
| boundary | whole workflow unless a phase is explicitly named |
| metrics | latency and throughput |
| correctness gate | `parity_pass` for parity-backed inputs; `successful_execution` for benchmark-only workflows |
| target subjects | Pillow oracle, `python-cpu`, `python-simd`, `python-gpu` |

The CPU, SIMD, and GPU subjects use the Python and core default Cargo features;
the core default excludes Rayon `parallel`. These profiles differ by the
runtime backend request, not by a separately compiled feature set. Rayon work
is measured separately as Parallel CPU:

```sh
MIGRATION_BENCHMARK_PROFILE=quick make migration-parity-benchmark-parallel-cpu
```

That target rebuilds the facade with the explicit `parallel` feature, requests
only the CPU executor, verifies the built feature at runtime, parity-gates the
same CPU-applicable workloads, and writes separate `python-parallel-cpu`
receipts under `build/migration-parity/`. Its runtime backend is `cpu`; its
profile identity records both opt-in feature flags. Do not compare those
receipts as serial CPU or SIMD results.

Each workload keeps its own declared policy in the generated input. The fixed
11-workload release-acceptance cohort intentionally uses one warmup, three
measurement iterations, and two samples (six timed executions per subject),
so its receipts must not be described as five warmups, 20 iterations, and five
samples. The reference policy above applies to the standard rows that declare
it; the runner never silently overrides a workload's input policy.

The benchmark runner records setup, pipeline, terminal, dispatch, fallback,
resource, and timing data where the adapter exposes them. Resource receipts
include checked host pixel-buffer allocation count and bytes; they are not a
process-wide allocator trace, so a zero count does not prove that dependencies
or bindings made no allocations. A result is usable only when its manifest/input
hashes, runtime identity, requested/actual backend, and terminal receipts are
compatible with the comparison.

Backend timing adapters activate one requested backend before measurement.
Keep target-only backend locking, telemetry, receipt retrieval, and JSON output
outside timed samples. An untimed proof run must execute the same case with the
same active backend and dispatch policy; require a terminal-complete receipt,
matching requested and actual backends, and no fallback. Proof receipt counts
are separate from latency sample counts. If the proof cannot establish the
timed route, mark that profile unverified instead of counting a fallback as
accelerated execution. Strict parity audits may still lock lazy pipelines
outside benchmark timing.

The default image-font `getlength` workload measures the `getlength` call as
an observed step; `load_default()` setup has its own workload. The loader still
runs to create the font, but its time is excluded from call-only latency. The
default image-font `getbbox` and `ImageDraw.textlength` rows isolate their
method calls from font, image, and draw setup. The `ImageDraw.textbbox` row
isolates its call too; when font is omitted, default-font creation remains
part of that public call. The standard multiline draw workloads use reviewed
line-bearing cases: `ImageDraw.multiline_text` uses `hello\nworld`, and
`ImageDraw.multiline_textbbox` uses `A\nBB\nC`. Their ordinary default inputs
contain no newline and skip line stepping or per-line aggregation. Use the full
workflow when comparing end-to-end construction plus measurement. Font metrics
and text rasterization do not necessarily enter the image SIMD/GPU kernels, so
interpret those rows with
their actual-backend receipts.

The dedicated `thumbnail` material-size workload uses a parity-backed
1024 × 768 RGB image, downsizes it to fit 256 × 256 with BICUBIC, and times the
public mutation plus receiver `tobytes`. It keeps image creation outside the
timed steps and includes terminal export so eager Pillow execution and deferred
target execution cover the same observable work. Its workload ID is
`pipeline-op.thumbnail.material-rgb-1024x768`; filter it with the pipeline
profile when investigating thumbnail:

```sh
MIGRATION_BENCHMARK_PROFILE=pipeline \
MIGRATION_BENCHMARK_ARGS="--workload-id pipeline-op.thumbnail.material-rgb-1024x768" \
make migration-parity-benchmark
```

The small thumbnail row remains useful for dispatch and adapter overhead, but
must not stand in for this material workload.

The public benchmark page presents two tables: individual operation workloads
and composed pipeline workloads. The published 2026-10-01 snapshot (source
revision `8f40b3e6`; [benchmark run and artifact](https://github.com/appunni-m/pillow-rs/actions/runs/36873206491))
contains 639 workload rows: 183 individual-operation workloads and 456
composed, matrix, lifecycle, and quick pipeline workloads. Its artifact records
3,195 measurements across five subjects: ordinary Pillow, CPU, SIMD, GPU, and
Parallel CPU. The 636 entries in `pipeline-operations.json` comprise 180
individual operations and 456 pipelines. Three more material filter workloads
are declared in `pil-imagefilter.json`: BoxBlur on L, GaussianBlur on L, and
UnsharpMask on RGB. They are active rows in the full benchmark profile, not
historical-only evidence. The recorded contexts include 12 mode labels,
including mixed-mode `I+F`. Each row carries its own mode, operation type,
dimensions, boundary, and repeat policy; the page filters by type, mode,
runner, implementation, or workload text, and sorts by type, mode, workload
name, or backend latency. These counts do not claim that every operation/mode
combination is supported or measured by every backend. The benchmark
completeness check verifies that every active `PipelineOp` and maintained
eager operation has a workload specification.

The scheduled and manual GitHub benchmark runs use three separately identified
runner cohorts: macOS 15 arm64 measures CPU, SIMD, GPU, and Parallel CPU;
Ubuntu 24.04 x86_64 and Ubuntu 24.04 arm64 each measure CPU, SIMD, and Parallel
CPU. The Linux jobs do not request GPU, so their missing GPU cells mean “not
measured,” not zero time or a CPU fallback result. All jobs run the same full
workload profile from the same source revision. Their artifacts are joined only
after that revision and runner identities validate. The report records each
runner's OS, architecture, CPU model, Python version, Rust toolchain, and
GitHub image version.
See GitHub's
[hosted-runner specifications](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
for the runner classes; physical VM instances are ephemeral, so the labels
identify runner classes rather than a persistent computer.

The results page opens with an “At a glance” summary. It defaults to CPU versus
Pillow when that comparison is measured. Each runner has its own row, with
individual operations and complete pipelines shown side by side. Each result
starts with a plain count such as “Faster in 8 of 10 matched cases,” followed
by a thick bar showing faster, tied, and slower cases. “Typical case” is the
middle per-case speed ratio; the slowest and fastest cases include their
measured times. The “Compare” selector chooses one profile and baseline; “All
comparisons” shows separate results for each pair. Runner rows are never
pooled. Every case counts once, so this is not a production-traffic-weighted
speedup or a performance guarantee.

An expandable ratio plot shows every parity-verified workload in the selected
comparison. The ratio is the baseline median latency divided by the pillow-rs
profile median latency: 1× means equal median time, values above 1× are faster,
and values below 1× are slower. Rows are sorted slowest first within each
runner, implementation, baseline, and workload type. The shared log₂ axis is
centered on the 1× parity line and clips at 1/16× and 16×; each row still shows
its speed factor, and hovering a point shows both measured medians. Its group
header also includes a geometric mean as a secondary multiplicative summary.
Execution-only measurements remain in the tables but are excluded from the
summary and plot. Workload, mode, runner, and comparison filters update the
summary, plot, and both tables. Ratios are paired inside one runner cohort;
absolute timing across macOS and Linux is not compared.

The x86 Pillow-SIMD run is an independently parity-gated cohort covering all
29 current full-size `pipeline-op` workloads, the established RGB GaussianBlur
case, and getchannel in four modes (34 individual-operation cases total).
The main results plot includes it only when the Pillow-SIMD and full benchmark
artifacts identify the same source revision and the same Ubuntu runner class,
OS, architecture, CPU model, Rust toolchain, and image version. Its own run
measures ordinary Pillow, Pillow-SIMD, and pillow-rs together on one x86
runner. The identity check matches runner profiles across the two runs; it does
not claim that GitHub reused the same physical VM. If the source revision or
runner identity differs, those measurements stay on their separate page and
are omitted from the overall plot until a matching run is available.

The published page uses the full pipeline profile, not the four-workload quick
smoke profile. The workflow also runs the opt-in Rayon feature as a separate
Parallel CPU build and times only `python-parallel-cpu`; its parity preflight
still compares outputs with Pillow. The public tables reuse the ordinary
Pillow timing from the standard run, after matching source revision, host,
workload, input mode, and measurement policy. That single ordinary Pillow
measurement is the baseline for Parallel CPU too; there is no threaded Pillow
run or separate `pillow-parallel-cpu` baseline. This avoids timing Pillow twice
and keeps Parallel CPU distinct from default serial CPU, architecture-specific
SIMD, and GPU. Quick results remain useful during local development:

```sh
MIGRATION_BENCHMARK_PROFILE=quick make migration-parity-benchmark
MIGRATION_BENCHMARK_PROFILE=quick make migration-parity-benchmark-parallel-cpu
```

`ImageOps.posterize` also has parity-gated material workloads at
`pipeline-op.posterize.material-l-noise-1024x768` and
`pipeline-op.posterize.material-rgb-noise-1024x768`. They use seeded nonuniform
bytes, keep image setup outside the measured boundary, and time Posterize plus
`tobytes` across Pillow, CPU, SIMD, and GPU. Use these rows when investigating
native L/RGB masks; the 16×16 materialized smoke row measures mostly dispatch
overhead.

`Image.Image.filter(MedianFilter(3))` has a parity-gated varied-L workload at
`pipeline-op.medianfilter.material-l-noise-1024x768`. It uses the same seeded
1024×768 input for Pillow, CPU, SIMD, and GPU, excludes image/filter setup,
and measures filter execution plus `tobytes`. Select it with:

```sh
MIGRATION_BENCHMARK_PROFILE=pipeline \
MIGRATION_BENCHMARK_ARGS="--workload-id pipeline-op.medianfilter.material-l-noise-1024x768" \
make migration-parity-benchmark
```

The native-LA `MedianFilter(3)` workload is
`pipeline-op.medianfilter.material-la-noise-1024x768`. It uses seeded
1024×768 LA pixels and the same parity gate and measured boundary. This row is
useful for confirming alpha-channel parity and checking that GPU execution
keeps the two native bytes per pixel instead of expanding them to RGBA:

```sh
MIGRATION_BENCHMARK_PROFILE=pipeline \
MIGRATION_BENCHMARK_ARGS="--workload-id pipeline-op.medianfilter.material-la-noise-1024x768" \
make migration-parity-benchmark
```

The native-RGB `MedianFilter(3)` workload is
`pipeline-op.medianfilter.material-rgb-noise-1024x768`. It uses seeded
1024×768 RGB noise and the same Pillow parity gate and full-result timing
boundary. The GPU path keeps RGB triples native through upload, filtering, and
readback. Use this row to compare RGB channel medians independently from L and
LA:

```sh
MIGRATION_BENCHMARK_PROFILE=pipeline \
MIGRATION_BENCHMARK_ARGS="--workload-id pipeline-op.medianfilter.material-rgb-noise-1024x768" \
make migration-parity-benchmark
```

To measure the opt-in Parallel CPU profile separately, compare its result with
the ordinary single-threaded Pillow subject from the standard workload above:

```sh
MIGRATION_BENCHMARK_PROFILE=pipeline \
MIGRATION_BENCHMARK_ARGS="--workload-id pipeline-op.medianfilter.material-la-noise-1024x768" \
make migration-parity-benchmark-parallel-cpu
```

For RGB, select
`pipeline-op.medianfilter.material-rgb-noise-1024x768` with the same command.
The Parallel CPU target runs only the opt-in CPU profile; compare it with the
ordinary Pillow result from the standard RGB workload.

## Correctness gate and budget gate

Run the correctness gate before interpreting timing:

```sh
make migration-parity-inputs-check
MIGRATION_BENCHMARK_PROFILE=quick make migration-parity-benchmark
make migration-parity-pipeline-report
make migration-parity-pipeline-roadmap-status
```

The scheduled/manual GitHub Actions workflow accepts an optional
`baseline_run_id`. When supplied, it downloads that exact prior benchmark
artifact, checks manifest/input/backend/terminal compatibility, and records
the unchanged five-percent budget comparison in the new artifact. A timing
violation is marked review-needed and retained in the summary; correctness or
schema failures still fail the job. Leave the input empty for a standalone
benchmark run.

For a complete standard run:

```sh
MIGRATION_BENCHMARK_PROFILE=standard make migration-parity-benchmark
```

For a single pipeline workload while debugging one operation, combine the
pipeline profile with its workload ID:

```sh
MIGRATION_BENCHMARK_PROFILE=pipeline \
MIGRATION_BENCHMARK_ARGS="--workload-id pipeline-matrix.expanded.overlay.1024x768" \
make migration-parity-benchmark
```

The runner applies the workload filter inside the pipeline selection and tests
that behavior before each benchmark run.

To retain every declared public operation against the CPU ≤ Pillow, SIMD ≥ 5×
Pillow, and GPU ≤ SIMD latency goals, generate the diagnostic matrix:

```sh
.venv/bin/python scripts/report_optimization_goals.py \
  --result build/migration-parity/benchmark-result.json \
  --parity build/migration-parity/benchmark-parity-result.json
```

This writes `build/migration-parity/optimization-goals.json` and `.md`. Missing
workloads, missing backend-specific parity, fallback, and dirty provenance stay
visible. Successful execution alone does not establish matching output. The
benchmark runner's reciprocal-latency throughput values do not establish
sustained concurrent throughput; that goal requires separate completed-work
windows with changing inputs. The matrix retains all manifest rows, while
public exports outside that selected contract still require inventory review.

For equalize throughput after `make build-parity`, reuse the completed-request
window diagnostic with the equalize selector:

```sh
.venv/bin/python scripts/run_transpose_throughput.py \
  --operation equalize --size 1024 768 \
  --output build/migration-parity/equalize-throughput.json
```

It measures fresh L/RGB requests over 16 changing inputs at queue depths 1, 2,
and 4. Each request includes construction, equalize, and terminal bytes; each
output is compared exactly with live Pillow outside the timing window. GPU
receipts must include the histogram, LUT, and remap compute dispatches and
complete transfers. The histogram is cleared with a command-encoder buffer
clear, which is device work but not a compute dispatch.
`--check-only` verifies one window per depth without emitting timing summaries.
The transpose selector remains the default with its existing workload policy.
Use `--operation invert` and a separate output path for fresh L/RGB inversion
under the same window policy. Inversion requires one GPU dispatch and complete
transfers; it uses the original full-range input tile.
Use `--operation getchannel` to measure band 0 from fresh L images and the
green band from fresh RGB images. The output is an L image, and every request
is compared byte-for-byte with live Pillow. This reports independent host
requests sharing the backend queue; it is not a multi-image GPU batch.

```sh
.venv/bin/python scripts/run_transpose_throughput.py \
  --operation getchannel --mode RGB --size 1024 768 \
  --output build/migration-parity/getchannel-rgb-throughput.json
```

Use `--operation blend` for two fresh images blended at alpha 0.3. The second
image uses the next changing input frame; construction of both images is timed,
and GPU receipts must include its auxiliary image transfer.
Use `--operation add` or `--operation subtract` for the same two-image boundary
with default scale 1 and offset 0. Their input sequence, window policy, and
auxiliary-transfer checks are identical; each operation uses a distinct output
path.

Use `--operation composite` for fresh L/RGB/RGBA `ImageChops.composite`
requests. It constructs two same-mode operands and an independent changing L
mask, times all three constructions plus the composite and terminal bytes, and
checks every output exactly against live Pillow. GPU receipts must account for
image2 and mask transfers as well as the primary upload and result readback.
Select `--mode RGB` or `--mode L` for separate channel-width comparisons; use a
distinct output path for each run.

Use `--operation autocontrast` for fresh L/RGB `ImageOps.autocontrast` requests
at cutoff zero. The changing-input corpus spans 32 levels with distinct frame
offsets so each output exercises a nonidentity LUT. The diagnostic checks every
output against live Pillow and checks complete input/readback accounting. The
unmasked native L/RGB path derives the exact LUT on the host and requires one
GPU remap dispatch; masked or unsupported layouts retain the four-dispatch
native histogram/cutoff/remap path.

Use `--operation grayscale` for fresh L/RGB inputs producing L output under
the same window policy. GPU receipts check the full multichannel input upload
and the smaller output readback separately. For exhaustive RGB arithmetic
parity after `make build-parity`, run:

```sh
.venv/bin/python scripts/test_grayscale_rgb_domain.py \
  --output-dir build/migration-parity/grayscale-rgb-domain
```

This diagnostic compares all 16,777,216 RGB triples with isolated live Pillow,
requires native CPU/SIMD/GPU execution, and retains actual output bytes and
binary identities. It does not measure throughput or collect coverage.

For the fixed release-acceptance cohort:

```sh
MIGRATION_BENCHMARK_PROFILE=release make migration-parity-benchmark
```

This profile selects the maintained 11 workload IDs from the root `Makefile`
and leaves each generated workload's warmup, iteration, and sample policy
unchanged. Use the same profile and source checkout for both runs in a budget
comparison.

On macOS, rerun the fixed cohort with the maintained low-load entry point when
the host scheduler is noisy:

```sh
MIGRATION_BENCHMARK_PROFILE=release make migration-parity-benchmark-low-load
```

This applies Darwin utility QoS and background I/O policy through `taskpolicy`
when that command is available. Other hosts use their native scheduler. The
workload IDs, repeat policy, timing budget, and receipt contract are unchanged.

Compare two compatible result artifacts with an explicit baseline. The budget
checker uses the repository's five-percent policy; timing variance is recorded
as a violation rather than hidden by changing the threshold:

```sh
MIGRATION_BENCHMARK_OUTPUT=build/migration-parity/current.json \
MIGRATION_BENCHMARK_BUDGET_BASELINE=build/migration-parity/baseline.json \
make migration-parity-pipeline-budget-check
```

Retained comparison artifacts may live outside the checkout (for example under
`/tmp` while a run is being reviewed). The performance, workload-coverage, and
roadmap report commands preserve those external paths and accept them directly;
they do not require copying a result into `build/migration-parity/`.

The [public pipeline roadmap](PIPELINE_ROADMAP.md) retains all 64 item IDs and
their reviewed statuses. Its generated status report combines that index with
execution evidence; timings never automatically close a work item.

## Interpretation rules

- Report median and spread with the environment and commit; do not publish a
  single timing as a universal speed claim.
- Keep cold setup, warm resident work, and terminal readback in separate
  columns. A full-process number is not an operation number.
- Compare like-for-like input shape, mode, build profile, cache state, and
  requested backend. A CPU fallback is evidence of routing, not native GPU
  performance.
- Keep outliers visible. Re-run a noisy cohort on the same machine before
  deciding whether a code change caused a regression.
- Never use a profiler-instrumented run as an acceptance timing sample.
- Keep the input JSON free of expected values, hashes, and run status; those
  belong to result artifacts.

These rules follow the Rust Performance Book's advice to use realistic
workloads and to treat benchmarking as an empirical process, and Criterion's
warmup/measurement/analysis/comparison model. See the
[Rust Performance Book](https://nnethercote.github.io/perf-book/benchmarking.html)
and [Criterion analysis documentation](https://bheisler.github.io/criterion.rs/book/analysis.html).

## Published results

The [GitHub Pages benchmark view](https://appunni-m.github.io/pillow-rs/benchmarks/)
renders validated snapshots with per-workload policies, medians, percentiles,
correctness gates, and requested/actual backends. It retains failed subjects and
missing measurements. It does not aggregate unrelated operations into a headline
speedup. Hosted runners are useful observations, not controlled laboratory hosts.

### Complete published optimization matrix

The [published full benchmark JSON](https://appunni-m.github.io/pillow-rs/assets/benchmark.json)
and the separate [Pillow-SIMD JSON](https://appunni-m.github.io/pillow-rs/assets/pillow-simd-benchmark.json)
can be flattened into the complete per-workload record at
[`docs/evidence/performance-optimization-matrix.csv`](evidence/performance-optimization-matrix.csv).
The companion [public API census](evidence/performance-optimization-public-api.csv)
enumerates the loaded `PIL` facade, the native extension, each module's explicit
`__all__` exports or locally defined public functions and classes, public class
methods and properties, and the selected manifest mapping. Imported aliases
aren't repeated as new API paths in every module. The [per-operation matrix](evidence/performance-optimization-operation-matrix.csv)
has four profile rows for each of its 330 paths: 209 paths map to the selected
manifest and 121 outside paths remain explicit as `not_mapped_to_benchmark`.
Those inventory-only rows are unmeasured gaps, not evidence that a profile
supports or has run the operation.
The matrix keeps successful, failed, not-run, fallback, and dirty-snapshot rows.
It includes source hashes and revisions, runner and workload identity, mode and
size, full-call policy, exact-parity status, actual backend, observed and
eligible speed ratios, and separate CPU, SIMD, GPU-latency, and GPU-throughput
goal fields. A GPU throughput status of `not_measured` is deliberate: the
published one-request latency cannot establish sustained throughput.

Rebuild it from the raw published assets with:

```sh
curl --compressed -fsSL https://appunni-m.github.io/pillow-rs/assets/benchmark.json -o /tmp/pillow-rs-benchmark.json
curl --compressed -fsSL https://appunni-m.github.io/pillow-rs/assets/pillow-simd-benchmark.json -o /tmp/pillow-rs-pillow-simd.json
PYTHONPATH=pillow-rs-py/python .venv/bin/python scripts/build_public_api_inventory.py
python3 scripts/build_performance_optimization_matrix.py \
  --benchmark-snapshot /tmp/pillow-rs-benchmark.json \
  --pillow-simd-snapshot /tmp/pillow-rs-pillow-simd.json \
  --public-api-inventory docs/evidence/performance-optimization-public-api.csv
```

The latest published workload matrix records 8,655 rows total: 8,619 full-
benchmark rows across 663 workloads and 36 Pillow-SIMD rows across 9 workloads.
The full snapshot is clean at revision
`ff0009b71755d6f1cebae63c96b96d39aa183797`, measured
`2026-10-09T07:20:52.003305Z`, with JSON SHA-256
`f34d8cff7d36fb1a605da4d382b2cb5818becb200057410cfc749d97df516c00`, from the [published benchmark JSON](https://appunni-m.github.io/pillow-rs/assets/benchmark.json).
It contains 201 `pipeline-op` workload IDs. The operation matrix has 1,320 rows
for 330 public paths and four profiles; 209 paths map to the benchmark manifest
and 121 remain explicit inventory gaps. Only rows with exact parity and
completed backend receipts are eligible for speed claims.

Across the 120 verified serial CPU workload-runner pairs, none is slower than
Pillow and 53 are below 2×. The corresponding SIMD counts are 22 slower than
Pillow, 37 below 2×, and 81 below 5×. The separately built Parallel CPU profile
uses the opt-in Cargo `parallel` feature; 15 of its 120 verified pairs are
slower than Pillow, 39 are below 2×, and 83 are below 5×. GPU latency is slower
than SIMD in 22 of its 38 verified pairs; sustained GPU throughput remains
unmeasured. The operation matrix's GPU latency baseline is SIMD; its GPU
`below_baseline_comparisons` field and per-operation ratios must not be read as
Pillow comparisons.

The clean ff0009 hosted snapshot confirms the pre-sized native-F thumbnail
reducer and leaves no verified serial CPU pair below Pillow. The weakest CPU
result is the 1024×768 YCbCr grayscale pipeline on Ubuntu x86_64 at 1.000848×
Pillow. The Pillow-SIMD snapshot remains dirty and partial (9 of 34 declared
cases), so its ratios are excluded from the optimization ranking.

### Rejected CPU alpha-composite-plus-mirror fusion trial

A local release-mode A/B/A benchmark used the exact 1024×768 RGBA public
workflow, five samples of 20 timed iterations, Pillow parity, and complete
actual-backend receipts. The unfused CPU medians were 1.8744 ms and 1.8065 ms;
the fused candidate measured 1.8486 ms. Pillow medians were 2.1751, 2.1611,
and 2.2048 ms, respectively. The candidate's latency sits within the
unfused-run spread and does not show a repeatable end-to-end speedup. It did
reduce CPU resource accounting from 3 host buffers / 9,437,184 allocated bytes
to 2 / 6,291,456, while peak live bytes remained 6,291,456. Both paths passed
exact parity with 100/100 actual CPU samples and no fallback. Reject the fusion
until an implementation demonstrates lower end-to-end latency; a lower buffer
count alone does not meet the latency goal. The GPU run was actual GPU on all
100 samples but measured 1.907 ms against SIMD at 0.649 ms, so GPU latency
remains below target and sustained throughput is still unmeasured. No product
or existing-test defect was found and no coverage was run.

### Rejected CPU alpha-composite coefficient lookup

A lazy 256×256 `u16` table replaced the per-pixel integer quotient, keyed by
source and destination alpha. It preserved native LA/RGBA processing and left
SIMD/GPU dispatch untouched. The exhaustive Rust test passed all 65,536 alpha
pairs in both modes; 164 selected public Pillow-reference cases passed for
`Image.Image.alpha_composite` and `Image.alpha_composite`.

A first release comparison used a temporary environment branch at the operator
call site and appeared to improve the 1024×768 RGBA composite/mirror workflow.
That branch changes the production call graph and can alter inlining. I
therefore rebuilt direct lookup and direct division sources without the toggle.
For the parity-gated 100-sample CPU full workflow, including setup and
materialization, the A/B/A medians were lookup 2.0421 ms, division 2.0588 ms,
and lookup 2.1213 ms. Same-run Pillow medians were 3.0694, 3.1309, and 3.3298
ms. The direct runs do not show a stable lookup win. Reject the 128 KiB table
and keep the original quotient. The temporary branch and table code were
removed; this experiment does not count toward the 33-item push batch.

Four standalone LA/RGBA size workloads showed lower lookup medians in the
screening run, but their correctness gate was `successful_execution`, not exact
Pillow parity. Treat those timings as exploratory and exclude them from
parity-backed speed claims. The exact-parity workload used CPU, SIMD, and GPU
100/100 times with complete terminal receipts and zero fallback. In its final
lookup run SIMD measured 0.654 ms and GPU 1.969 ms; GPU latency still misses the
SIMD target, and sustained throughput remains unmeasured. No genuine product or
test defect was found, and no coverage was run.

### Rejected CPU BoxBlur radius-one integer average

A serial CPU fast path replaced the radius-one fixed-point multiply with the
equivalent rounded three-byte average and used a direct sliding recurrence for
both image axes. Focused Rust tests matched the existing fixed-point kernel for
line lengths 1, 2, 3, 4, 7, and 33 across six component widths, and verified all
766 possible three-byte sums. The selected 1024×768 RGB BoxBlur pipeline passed
its exact parity gate for Pillow, CPU, SIMD, and GPU. CPU, SIMD, and GPU each
reported 100/100 executions on their requested backend with no fallback; the
CPU retained the same two host buffers and 4,718,592 allocated bytes.

The CPU observed-step median was 2.7794785 ms in the baseline run and 2.922104
ms with the candidate, with p95 increasing from 2.871584 to 3.0295 ms. The
candidate did not demonstrate a latency win, so the fast path and its tests were
removed. Pillow medians differed substantially between those separate runs
(3.342021 ms and 2.655771 ms), so their ratio is not suitable for an A/B claim.
The hosted full-matrix snapshot remains the ranking source. This experiment
does not count toward the 33-item push batch. No product or existing-test defect
was found, and no coverage was run.

### Accepted GPU L BoxBlur(1) fused path

The local 1024×768 `L` BoxBlur(1) workload uses a mapped shared-memory upload
on supported integrated Metal devices and a fused one-dispatch shader. The
shader performs the exact rounded horizontal average before the rounded
vertical average, so Pillow's byte rounding remains unchanged while the
full-frame intermediate and second dispatch disappear. A focused GPU parity
test also matches the CPU reference at 1×1, 1×3, 4×3, 5×3, and 33×35,
including replicated edges, packed tails, and odd row widths; each case reports
one GPU dispatch and no fallback.

Two release benchmark runs passed the exact Pillow parity gate and completed
100/100 samples on each requested CPU, SIMD, and GPU backend with no fallback.
Observed steps included filter application and result materialization. Run one
medians in milliseconds were Pillow 1.7467, CPU 0.8818, SIMD 0.6636, and GPU
0.5308; run two measured 1.7445, 0.8805, 0.6726, and 0.4600. The GPU used one
dispatch, uploaded and read back 786,432 bytes, and reported one full-frame
copy. Reported throughput was 1,884 ops/s GPU versus 1,507 SIMD in run one and
2,175 versus 1,487 in run two. Both SIMD medians exceeded 2× Pillow, and both
GPU medians beat SIMD. These are local macOS arm64 results at workload
concurrency 1; they do not prove throughput under concurrent saturation or
cover BoxBlur's other modes and radii. Keep the BoxBlur Kata item open and do
not count the operation toward the 33-item push batch yet. The c25 hosted
matrix remains the complete ranking snapshot. No coverage was run.

### Local Parallel CPU L/RGB BoxBlur(1) vertical candidate

For large Parallel CPU frames with integer vertical radius one, the candidate
processes replicated-edge rows directly in parallel and skips the transpose,
blur, and transpose-back sequence. All other radius paths retain their existing
dispatch. A focused test matched the existing fixed-point column kernel across
1×1, 1×7, 2×3, 17×9, and 65×33 inputs with 1–4 channels.

Two correctness-gated release runs for
`pipeline-op.boxblur.material-l-noise-1024x768-radius-1` and three for
`pipeline-op.boxblur.material-rgb-noise-1024x768-radius-1` timed `apply-filter`
and `observe-filter-result`, including output materialization. All five runs
reported exact Pillow parity, 100/100 executions under the
`python-parallel-cpu` profile with the opt-in `parallel` Cargo feature, actual
CPU backend receipts for every sample, complete terminal receipts, and no
fallback. L medians were 0.346396 and 0.350167 ms (p95 0.408000 and 0.434208
ms); RGB medians were 0.720855, 0.717396, and 0.727896 ms (p95 0.828125,
0.820125, and 0.896417 ms). Reported single-concurrency throughput was 2,887
and 2,856 ops/s for L, and 1,387, 1,394, and 1,374 ops/s for RGB. Two RGB and
both L benchmark/parity JSON pairs are saved under `build/migration-parity/`
with `boxblur-*-parallel-direct-vertical` filenames.

The c25 same-host snapshot recorded Pillow/Parallel CPU medians of
2.342104/3.044042 ms for L and 3.235688/4.654979 ms for RGB. Dividing its
Pillow medians by the local candidate medians implies 6.69–6.76× for L and
4.45–4.51× for RGB. These are separate runs, so retain those factors as
prioritization signals rather than refreshed comparisons. The full c25 matrix
remains canonical until a matched multi-profile snapshot is produced. This
candidate covers two modes, one size, and radius one; keep BoxBlur open and do
not count it toward the 33-item push batch. No coverage was run.

### Local standard BoxBlur L/RGB cross-backend repeats

Two release runs of the standard CPU/SIMD/GPU profiles measured both
1024×768 L and RGB BoxBlur(1) workloads in each run. The comparison timed
`apply-filter` and `observe-filter-result`, including allocation and
materialization. Every workload passed exact Pillow parity; each target profile
reported 100/100 actual executions and no fallback. The separately compiled
Parallel CPU profile was not included in these runs.

Per-run medians in milliseconds (Pillow / CPU / SIMD / GPU) were L
1.732292/0.906021/0.673666/0.676937 and
1.748854/0.877292/0.651687/0.602521; RGB
2.659542/2.235854/1.943395/2.373500 and
2.635479/2.245375/1.942209/2.331980. CPU stayed faster than Pillow but below
2× for both modes. SIMD passed 2× for L (2.57× and 2.68×) but not RGB (1.37×
and 1.36×). GPU L was slightly slower than SIMD in one run and faster in the
other; GPU RGB was 20–22% slower than SIMD in both. GPU dispatch receipts were
one for L and two for RGB; upload and readback were 786,432 bytes for L and
2,359,296 bytes for RGB, with one recorded full-frame copy and zero mode
conversions. These concurrency-1 runs do not prove sustained GPU throughput.
Keep the canonical c25 matrix and the BoxBlur Kata item open. Local result and
parity JSON pairs are `boxblur-standard-l-rgb-repeat1` and
`boxblur-standard-l-rgb-repeat2` under `build/migration-parity/`. No coverage
was run.

### Local SIMD RGB BoxBlur(1) streaming rows

The RGB SIMD BoxBlur(1) path now keeps three horizontally blurred rows in a
small ring and emits each vertically rounded output row directly. It removes
the full-frame horizontal intermediate while preserving both Pillow rounding
stages. The focused `rgb_box_blur_radius_one_direct_passes_match_cpu_at_edges_and_tails`
test passed over widths 1–1024 and heights 1–53, including odd vector tails.

Two correctness-gated 1024×768 RGB release runs passed exact Pillow parity and
reported 100/100 actual CPU, SIMD, and GPU executions with no fallback. Both
timed `apply-filter` and `observe-filter-result`, including output
materialization. Run one medians in milliseconds (Pillow / CPU / SIMD / GPU)
were 2.568938 / 2.180000 / 1.738500 / 1.517625; run two were
2.612604 / 2.176916 / 1.746542 / 1.610270. SIMD measured 1.48× and 1.50×
Pillow, up from 1.36× in the preceding local run, but remains below 2×. GPU
used one actual dispatch and was faster than SIMD in both runs; measured
concurrency-1 throughput was 659 vs 575 ops/s, then 621 vs 573. Upload and
readback remained 2,359,296 bytes with one full-frame copy and no mode
conversion. This does not prove sustained GPU throughput. The c25 matrix remains
canonical, BoxBlur stays open, and this candidate does not count toward the
33-item push batch. Result/parity JSON pairs are `boxblur-rgb-simd-ring` and
`boxblur-rgb-simd-ring-repeat` under `build/migration-parity/`. No coverage was
run.

### Local serial CPU RGB BoxBlur(1) streaming rows

The serial CPU BoxBlur(1) path now writes final rows from a checked three-row
horizontal ring. Its exact fixed-point test matches the two generic passes
across 1–4 channels, 1×1 through 65×33 dimensions, narrow borders, and row
tails. The first ring draft passed parity but recomputed each middle horizontal
row when it became the center row; that run measured 3.037 ms CPU versus
2.755 ms Pillow. Producing each horizontal row once corrected the work and cut
latency, demonstrating that a bounded ring must track which rows are already
live rather than simply recomputing the center.

Two correctness-gated 1024×768 RGB release repeats passed exact Pillow parity.
Both timed `apply-filter` and `observe-filter-result`, including allocation and
materialization, and reported 100/100 actual CPU, SIMD, and GPU executions with
no fallback. Run medians in milliseconds (Pillow / CPU / SIMD / GPU) were
2.582167 / 1.663313 / 1.708396 / 1.511313 and 2.492063 / 1.666041 / 1.708937 /
1.513917. CPU reached 1.55× and 1.50× Pillow, still below the 2× sequencing
target; SIMD reached 1.51× and 1.46×. The CPU receipt reports two checked
allocations totaling 2,368,512 bytes (full RGB output plus a three-row ring).
GPU used one dispatch, uploaded and read back 2,359,296 bytes, and reported one
full-frame copy. Its concurrency-1 latency beat SIMD, but sustained GPU
throughput remains unmeasured. Checked-allocation receipts do not count every
allocation in Python, Rust dependencies, or unchecked existing buffer clones.
The SIMD RGB row ring now also uses `CheckedDims`; a final-tree confirmation
measured Pillow / CPU / SIMD / GPU at 2.570250 / 1.671709 / 1.713250 /
1.510021 ms. All target profiles again reported 100/100 requested-backend
executions with exact parity and no fallback. The SIMD checked-allocation
receipt now also reports two buffers totaling 2,368,512 bytes.
After reverting the no-gain SIMD arithmetic experiment and rebuilding from the
current source, another exact-parity run measured 2.691855 / 1.690063 /
1.773896 / 1.582084 ms. CPU and SIMD were 1.59× and 1.52× Pillow; all three
target lanes again executed 100/100 times on the requested backend with no
fallback. The GPU was faster than SIMD at concurrency 1, but sustained
throughput remains unmeasured.
Keep the canonical c25 matrix and BoxBlur Kata item open; this is one mode,
size, and radius and does not count toward the 33-item push batch. Result and
parity pairs are `boxblur-rgb-cpu-row-ring-instrumented` and
`boxblur-rgb-cpu-row-ring-instrumented-repeat` under
`build/migration-parity/`. No coverage was run.

The harness previously had a measurement gap: Rust execution receipts carried
checked host allocation count/bytes, but the PyO3 serializer and JSON
aggregator omitted them. New receipts now preserve those fields, and the
validator still accepts the older resource shape. This is checked-buffer
accounting, not a process allocator trace. The focused tests cover reporting
nonzero counters and accepting pre-existing receipts.

### Rejected SIMD RGB BoxBlur(1) 16-bit pre-sum

A sampled profile placed much of the SIMD kernel time in horizontal RGB vector
loads and widening. A candidate summed source bytes in 16-bit lanes, then
widened once before the fixed-point multiply; the largest three-byte sum is
765, so this preserves the arithmetic range. Focused RGB and L edge/tail tests
passed, as did both exact end-to-end benchmark gates. The candidate runs
measured Pillow / CPU / SIMD / GPU at 2.940 / 1.798 / 1.963 / 1.641 ms and
2.687 / 1.724 / 1.910 / 1.582 ms. Every lane shifted slower relative to the
final baseline run, and SIMD/Pillow fell from about 1.50× to 1.41–1.50×. No
repeatable full-call gain was shown, so the widening change was reverted. No
test defect was found; no coverage was run.

### Local RGB BoxBlur SIMD constant hoist and fused GPU reuse

For the 1024×768 RGB BoxBlur(1) row-ring kernel, hoisting the fixed-point SIMD
weight vectors out of the per-block horizontal loop and per-row vertical loop
reduced the local SIMD median from 1.736 ms to 1.413–1.421 ms across two
correctness-gated runs. Exact Pillow parity passed and 100/100 samples reported
the actual SIMD backend with no fallback. The repeats measured Pillow / CPU /
SIMD / GPU at 2.523 / 1.667 / 1.466 / 1.355 ms and 2.555 / 1.671 / 1.460 /
1.356 ms. This is 1.72–1.75× Pillow for SIMD, still short of 2×; CPU remains
faster than Pillow on this workload.

The fused RGB GPU shader now reuses the six horizontal source samples needed
for each group of four adjacent output pixels. The focused GPU test
`gpu_rgb_box_blur_radius_one_matches_cpu_at_edges_and_fuses_axes` passed, and
both public-workflow parity gates passed. GPU latency was 1.355–1.356 ms versus
1.460–1.466 ms SIMD; its reported concurrency-one repeated-workflow throughput
was 738 ops/s versus 682–685 ops/s SIMD. Each target executed 100/100 times on
its requested backend with no fallback. GPU used one dispatch and one checked
host output allocation of 2,359,296 bytes; it uploaded and read back 2,359,296
bytes each way. CPU and SIMD each recorded two checked allocations totaling
2,368,512 bytes. The GPU's single-concurrency receipt does not establish
sustained throughput, so keep that category open.

The mathematically exact reciprocal rewrite `(sum + 1) * 21846 >> 16` for
three-byte sums preserved parity but regressed SIMD from 1.736 ms to 2.248–
2.255 ms and was reverted. A fixed-coefficient variant measured 1.746 ms and
was also reverted. An AArch64 intrinsic draft was rejected by the repository's
`-D unsafe-code` lint and removed without weakening it. No existing test defect
was found. Keep the canonical c25 matrix unchanged; this local one-mode,
one-size, one-radius result does not complete BoxBlur or count toward the
33-item push batch. Results and parity receipts use the
`boxblur-rgb-hoisted-vector-constants` and `boxblur-rgb-gpu-reuse-candidate`
names under `build/migration-parity/`. No coverage was run.

### Existing GPU telemetry test isolation defect

The default-parallel filtered run `cargo test --package pillow-rs luma_`
reproduced an unrelated failure twice in
`gpu_native_luma_affine_nearest_transform_reads_and_writes_packed_bytes`:
the test expected `receipt.6 == Some(1)` but observed `Some(10)`. Running that
test alone passed, and `cargo test --package pillow-rs luma_ --
--test-threads=1` passed all 42 selected tests. This confirms a test isolation or
shared-state interference problem while leaving its root mechanism
unconfirmed; it does not implicate BoxBlur or establish a product bug. Keep the
assertion intact and run the affected group serially until the test isolation
issue is repaired. No coverage was run.

The c12 RGB grayscale candidate in revision `e9a8a7cf2` selected the signed
two-multiply delta formula on x86 without AVX-512 and retained contribution
tables on AVX-512 x86. It passed exact parity with 100/100 actual CPU samples,
a complete terminal receipt, and no fallback, but regressed on the AMD EPYC
7763 runner: 1,173.324 µs CPU versus 526.164 µs Pillow (0.448×). The clean c11
same-image baseline with the table path measured 815.868 versus 518.728 µs
(0.636×), so the candidate increased the CPU median by 43.8% while Pillow's
median shifted by 1.4%. Reject the delta path on this x86 cohort; the RGB
grayscale CPU gap remains the first optimization target. The published matrix
preserves this exact-parity regression as an observed, verified comparison.

The c13 direct-multiply RGB grayscale candidate in revision `a8bcf03af` kept
the lookup path on AVX-512 x86 and selected the standard three-multiply formula
on non-AVX-512 x86. It also passed exact parity with 100/100 actual CPU samples, a
complete terminal receipt, and no fallback, but further regressed on the same
AMD EPYC 7763 image: 1,444.471 µs CPU versus 551.185 µs Pillow (0.382×). The
c11 table-path baseline measured 815.868 versus 518.728 µs (0.636×), so direct
multiplication increased the CPU median 77.0% while Pillow shifted 6.3%. Reject
this candidate and return to the contribution tables while testing a different
loop strategy. The full c13 matrix preserves the exact-parity regression.

The c14 paired-contribution RGB grayscale candidate in revision `df5231519`
kept the three 256-entry tables on AVX-512 x86 and used a 65,536-entry paired
red/green table plus the blue table on other x86. It passed exact parity with
100/100 actual CPU samples, a complete terminal receipt, and no fallback, but
measured 885.304 µs versus Pillow's 528.082 µs (0.597×) on the AMD EPYC 7763
Ubuntu x86 runner. The clean c11 three-table baseline measured 815.868 versus
518.728 µs (0.636×); c14 increased the target median by 8.5% while Pillow
shifted 1.8%. Reject the larger paired table and restore the three-table path.
The c14 matrix preserves the exact-parity regression as measured evidence.

The c16 AVX2 RGB grayscale candidate in revision `bd54c3abe` deinterleaves
sixteen RGB samples and evaluates Pillow's exact rounded BT.601 fixed-point
formula. The x86 exhaustive check covered all 2^24 RGB colors and vector tails;
it passed in [Benchmark workflow 37870849801](https://github.com/appunni-m/pillow-rs/actions/runs/37870849801).
On that run's Ubuntu x86_64 `pipeline-op.grayscale.material-rgb-noise-1024x768`
workload, exact-parity CPU measured 242.6945 µs versus Pillow's 529.435 µs
(2.181×); SIMD measured 245.424 µs (2.157×). Both rows recorded 100 samples,
the requested backend, complete terminal receipts, and no fallback. These
ratios meet the CPU and 2× SIMD thresholds for this RGB workload on this runner.
They do not meet the grayscale operation's all-mode and all-size targets: its
worst verified CPU row is the YCbCr ARM result below, and its worst verified
SIMD row is RGB on Ubuntu ARM at 1.341× Pillow. The x86 machine model also
changed between c15 (EPYC 9V45) and c16 (EPYC 7763), so the within-run Pillow
ratios are useful while raw cross-run timing differences are not a paired
speedup claim.

The c16 YCbCr CPU row needed a repeat before attributing its timing to source
behavior. The exact workload on the same Neoverse-N2 runner measured 421.179 µs
versus Pillow's 208.410 µs (0.495×) in c16; c15 measured 113.1725 versus
208.1385 µs (1.839×). The YCbCr CPU implementation was unchanged between those
revisions, and the c16 SIMD row remained 112.408 µs. The c17 repeat measured
112.921 µs versus Pillow's 214.8815 µs (1.903×), so c16 remains visible as an
isolated timing outlier rather than a current source regression. An exploratory
local ARM microbenchmark rejected applying the existing packed-word extraction
helper on ARM: its median was 192.42 µs versus 50.08 µs for the current scalar
gather. That helper change was reverted before commit.

The c17 LA GaussianBlur row on macOS ARM measured 10,986.104 µs CPU versus
9,797.625 µs Pillow (0.892×), improving on c16's 11,718.2085 versus 9,794.5
µs (0.836×) with 100 samples in both runs. Exact parity, the requested CPU
backend, complete terminal receipts, and no fallback were recorded, but the
serial CPU miss remains. The c17 SIMD row measured 6,648.9375 µs (1.474×
Pillow), still below the 2× SIMD priority threshold. Separately, c17's composed
`getdata` band-read CPU row on macOS ARM measured 954.1455 µs versus Pillow's
678.3545 µs (0.711×), while c16 measured 328.896 versus 554.646 µs (1.686×).
That source path was unchanged, and this workload has only six timed executions;
keep the c17 exact-parity miss in the matrix and repeat it before selecting an
optimization. These timing outliers do not establish a test defect. No genuine
test defect has been confirmed, and no coverage was run.

The x86 YCbCr luma-packing change in revision `ca2e8ae4f` first cleared its CPU
miss in c10. For `pipeline-op.grayscale.material-ycbcr-noise-1024x768`, c9
measured 254.584 µs CPU versus 157.376 µs Pillow (0.618×), and c10 measured
134.672 versus 144.881 µs (1.076×). The c10 row passed exact parity, recorded
100/100 actual CPU samples, a complete terminal receipt, and no fallback.
Later clean c11 and c12 rows remained just above Pillow at 1.061× and 1.042×;
c13 measured 147.263 versus 139.972 µs (0.950×), putting it back among the
current CPU misses. The YCbCr luma path was unchanged by the RGB grayscale
experiments; keep this near-threshold result visible rather than attributing it
to them.

The full-box native-I thumbnail change clears the x86 CPU miss from the prior
snapshot. For `pipeline-op.thumbnail.native-i32-1024x768`, the published CPU
medians versus Pillow are 741.3125 versus 1190.9375 µs on macOS arm64 (1.607×),
999.656 versus 3281.506 µs on Ubuntu arm64 (3.283×), and 2418.7545 versus
5651.975 µs on Ubuntu x86_64 (2.337×). Each cohort recorded exact parity,
100/100 actual CPU samples, a complete terminal receipt, and no fallback. The
hosted result clears 2× on Ubuntu ARM and x86, but not macOS ARM. The x86 SIMD
row remains 0.741× Pillow, so the scalar fix does not close that separate SIMD
gap. These hosted results use the runner identities recorded per row; the
ephemeral x86 host model differs from the earlier c7 cohort.

The separate Pillow-SIMD asset remains unusable for verified ratios: revision
`101fdb8cc2c9da7ea98da8602ac4e7879ab23c9d` is marked dirty and covers 9 of 34
declared cases, so the matrix classifies its 18 reference rows and 18 target
rows as dirty evidence. The c8 version-matched job for revision
`469f8faa3` ([workflow 37849632545](https://github.com/appunni-m/pillow-rs/actions/runs/37849632545))
and c10 job for revision `ca2e8ae4f`
([workflow 37854781213](https://github.com/appunni-m/pillow-rs/actions/runs/37854781213))
failed at the parity-gated benchmark step. The c12 job for revision
`e9a8a7cf2`
([workflow 37859792281](https://github.com/appunni-m/pillow-rs/actions/runs/37859792281))
and c13 job for revision `a8bcf03af`
([workflow 37864202531](https://github.com/appunni-m/pillow-rs/actions/runs/37864202531))
also failed. Public job summaries do not identify a workload, and the raw logs
require GitHub authentication. No specific failing case or test defect is
confirmed.
Until a clean, complete matched cohort is published, the matrix does not treat
Pillow-SIMD ratios as verified evidence.

### c18 result, c19 repeat, and c20 AVX2 result

The fdb run includes the two-channel `blur_line_step` specialization for LA
GaussianBlur. Its focused direct-window reference test covers widths 1, 2, 5,
and 17 and radii 0, 0.25, 1, 1.375, 4.75, and 30; the existing fused/full-frame
test also passes. On a separate local macOS ARM host, the exact-parity
1024×768 LA benchmark measured 5.150375 ms CPU versus 5.663083 ms Pillow,
about 3.1% faster than the same host's pre-change CPU median. The hosted fdb
run measured CPU speedups of 1.371× on macOS ARM, 1.845× on Ubuntu ARM, and
2.873× on Ubuntu x86. SIMD measured 1.328×, 2.072×, and 4.455× respectively.
All rows had exact parity, the requested backend, complete terminal receipts,
and no fallback. macOS GPU measured 5.250 ms versus SIMD at 8.294 ms (1.580×
lower latency); Linux has no GPU runner, and sustained GPU throughput is not
measured. The c19 unchanged-source repeat measured LA CPU at 1.095× on macOS
ARM, 1.844× on Ubuntu ARM, and 2.881× on x86. These are above Pillow on all
three hosts, but hosted Mac movement across c17, aaf, fdb, and c19 is
non-monotonic; the 3.1% local CPU gain does not establish the cause of the
hosted ratio changes.

RGB material grayscale on Ubuntu x86_64 was the first stable CPU target. Its
c19 result was 637.799 µs CPU versus 427.073 µs Pillow (0.670×), with exact
parity and five measured samples. The fdb result was 635.962 versus 426.457 µs
(0.671×), and aaf was 640.579 versus 426.302 µs (0.665×) on the same EPYC 9V74
runner model. c17 measured 242.737 versus 528.870 µs (2.179×) on an EPYC 7763.
Grayscale source code did not change between c17 and c19, so those
cross-runner ratios do not prove a regression; the aaf/fdb/c19 repeats confirm
that the 9V74 CPU miss was stable before the dispatch change.

Revision `1ba7fd7` enables the existing exact AVX2 kernel in the CPU path even
when AVX-512 is present. The clean c20 snapshot confirms the x86 RGB result at
245.548 µs CPU versus 525.931 µs Pillow (2.142×), compared with c19's 637.799
µs (0.670×). The c20 CPU result has five timed sample batches (100 timed
workflow iterations); it passed exact parity, recorded the requested CPU
backend and a complete terminal receipt, and reported no fallback. This closes
the RGB grayscale CPU miss on that x86 cohort and clears 2× there. The remaining
c20 serial CPU misses are RGB thumbnail on macOS ARM (0.961×), LA GaussianBlur
on macOS ARM (0.975×), and RGB thumbnail on Ubuntu ARM (0.987×). No genuine
test defect has been identified.

### c21 local candidate: sparse RGB thumbnail reduction

The next CPU miss is RGB material thumbnail on macOS ARM. Its CPU profile
showed the RGB 2×2 reducing-gap scan and horizontal resample as the largest
sampled native stacks. A local candidate changes only the large sparse RGB
reducer: it skips eight-pixel blocks whose three 64-bit words are all zero,
while keeping the existing density cutoff and dense reducer. On this Mac, a
5,000-repeat target-only profile moved from a 1.153 ms median to 0.792 ms
(1.456×); this profile is diagnostic, not the published acceptance result.

The local correctness-gated whole-workflow benchmark includes image setup,
the pixel write, thumbnail, allocation, and materialization. Pillow measured
1.078 ms and CPU 0.791 ms (1.363×). CPU, SIMD, and GPU parity preflights all
passed, each requested backend produced 100 timed samples with a complete
terminal receipt and no fallback, and the reported actual backends matched.
SIMD measured 2.323 ms and GPU 2.321 ms on this host. The clean c21 full
snapshot at revision `94b816f` confirms exact-parity CPU results for the
1024×768 RGB thumbnail on macOS ARM (1.501× Pillow), Ubuntu ARM (1.322×), and
Ubuntu x86_64 (1.513×), each with 100 samples, a complete requested-CPU
receipt, and no fallback. The two earlier ARM CPU misses are now above Pillow,
but all three hosted rows remain below 2×. The sparse test also checks the
scan tail on an even-sized input whose pixel count is not divisible by eight.

### c22 local candidate: fractional radius-one LA GaussianBlur

The c20 serial CPU matrix ranked native LA GaussianBlur at 0.975× Pillow on
macOS ARM. A local profile found the fused three-pass horizontal LA routine
among the main native stacks. The candidate specializes fractional radius-one
rows, keeps Pillow's per-pass byte rounding and fixed-point weights, and
separates replicated edge pixels from the interior loop. Its exact line
comparison covers widths 5, 6, 7, 17, 65, and 1024; existing fused-versus-
full-frame checks cover tiny and ordinary LA images.

On the same local Mac and source revision, the generic CPU median was 5.110 ms
versus Pillow at 5.430 ms; the candidate median was 3.682 ms versus Pillow at
5.480 ms (1.488×). This is a 1.388× local improvement over the generic CPU
path. Candidate profiling sampled the new LA radius-one row and line functions
inside the materialized CPU operation. The correctness-gated whole-workflow
benchmark passed all three Pillow parity comparisons (CPU, SIMD, GPU); each
target had 100 timed samples, the requested backend actually ran, a complete
terminal receipt, and no fallback. SIMD measured 4.149 ms and GPU 2.078 ms in
this local run; the GPU row recorded 1,572,864 upload and readback bytes.
The clean c22 hosted snapshot at revision `b490bdf` confirms exact parity and
actual backend receipts for all 100 samples. CPU measured 6.619 ms on macOS
ARM (1.560× Pillow), 6.794 ms on Ubuntu ARM (2.735×), and 9.775 ms on Ubuntu
x86_64 (3.271×). CPU now beats Pillow on all three hosts, but remains below 2×
on macOS ARM. SIMD was 1.096× Pillow on macOS ARM, 2.078× on Ubuntu ARM, and
4.180× on x86_64; the macOS SIMD row remains below the 2× sequencing target.
The macOS GPU row executed on the requested GPU backend at 5.493 ms versus
9.425 ms SIMD, with exact parity and a complete receipt. The hosted one-request
comparison does not establish sustained GPU throughput. No genuine test defect
was found; no coverage was run.

### Rejected c23 local candidate: native-F thumbnail 12-tap unrolling

The c20 native-F thumbnail was near Pillow on x86 (1.025× CPU) and Ubuntu ARM
(1.053×), though the local Mac row already measured 2.254×. A local profile
identified boxed F Lanczos horizontal accumulation, so two scalar candidates
were tried: interleave four independent 12-tap outputs, then specialize one
12-tap output. The whole-workflow benchmark retained the public thumbnail call
and receiver observation, ran 100 samples, and passed exact Pillow parity for
each candidate. Against the local generic CPU median of 0.485396 ms, four-output
interleaving measured 0.572354 ms (17.9% slower) and per-output unrolling
measured 0.505750 ms (4.2% slower). Both experiments were reverted.

The local GPU request fell back to CPU in all 100 samples with the reason
`Thumbnail reducing-gap or typed arithmetic is not proven`; those rows are not
GPU measurements. The focused bitwise test initially assumed six interior
Lanczos taps for 2× reduction. The coefficient builder actually produces 12
interior taps, with tapered edge rows; the test assumption was corrected and
the scalar-order check passed. This was an invalid new test assumption, not a
product or existing-test defect. No coverage was run.

The clean c22 host matrix for the unchanged F path measured CPU at 1.869×
Pillow on macOS ARM, 1.187× on Ubuntu ARM, and 1.006× on Ubuntu x86_64, each
with exact parity and 100 completed actual-CPU samples. The c21 x86 result was
0.990×, so the one-percent movement straddles parity noise; no F candidate was
retained. SIMD was 1.338× Pillow on macOS ARM and slower than Pillow on both
Ubuntu runners. The macOS GPU request again fell back to CPU for unsupported
typed-F reducing-gap semantics.

### Local candidate: pre-sized F thumbnail reducer output

After the rejected resampler unrolling experiments, the 2×2 typed-F reducer
was profiled separately. The candidate pre-sizes its final byte buffer and
writes each four-byte sample into its row instead of extending the vector for
every output pixel. It preserves the scalar sample order and F32 rounding, and
a focused bitwise test covers finite values, negative zero, a payload NaN, and
both infinities.

The correctness-gated 1024×768 `thumbnail` workflow measured 100 samples per
subject and included the call and receiver observation. Against the same local
Mac baseline median of 0.485396 ms, the candidate measured 0.367792 ms and
0.376625 ms in two runs (1.32× and 1.29× faster). Pillow measured 1.063604 ms
and 1.045896 ms. Both runs passed exact parity; all 100 CPU samples executed
on CPU with complete terminal receipts and no fallback. SIMD remained at
0.808 ms. GPU requests fell back to CPU in all samples because typed-F
reducing-gap semantics are not proven, so they provide no GPU evidence.

The clean c24 hosted snapshot confirms the CPU improvement across all three
runner cohorts with exact parity and 100/100 actual-CPU samples: 558.209 µs on
macOS ARM (2.254× Pillow), 1,030.596 µs on Ubuntu ARM (1.474×), and 1,777.696
µs on Ubuntu x86_64 (1.113×). The candidate clears Pillow on each host but is
still below the 2× sequencing target on Ubuntu. Its SIMD rows are 1.035× Pillow
on macOS ARM, 0.867× on Ubuntu ARM, and 0.839× on x86_64. The macOS GPU request
fell back to CPU in all 100 samples because typed-F reducing-gap semantics are
not proven; it is not GPU evidence. No genuine product or test defect was
found, and no coverage was run.

The c19 grayscale rows keep mode-specific limits visible: YCbCr CPU is 2.214×
on macOS ARM, 1.887× on Ubuntu ARM, and 1.036× on Ubuntu x86. In c20, these
ratios are 2.222×, 1.779×, and 1.084× respectively; the x86 YCbCr path is now
the weakest CPU grayscale row. RGB grayscale SIMD is 2.089× on macOS ARM,
1.316× on Ubuntu ARM, and 2.158× on Ubuntu x86 in c20. The composed RGB
`getdata` band-read row on macOS ARM was 2.393× in c19, following 1.592× in
fdb and 1.834× in aaf; c17's 0.711× result is not repeatable evidence of a
stable miss. Preserve every cohort in the matrix.

The published Pillow-SIMD JSON remains marked dirty at revision
`101fdb8cc2c9da7ea98da8602ac4e7879ab23c9d` and contains only 9 of the 34
declared x86 cases (36 rows across 9 workloads). The current asset was measured
at `2026-10-07T13:19:09.608307Z`, has SHA-256
`8963ea465b99a6b2a2cca91ab2aa07ee321fb3a450edfd10a102f4681b3247e4`, and has
no grayscale workload. The full-benchmark matrix therefore retains those rows
as dirty evidence instead of treating them as verified Pillow-SIMD
comparisons. Use the published JSON as the source for rankings, but do not
begin the Pillow-SIMD-only phase until the serial CPU priority cohort has
cleared its 2× target and a clean, complete matched Pillow-SIMD cohort is
available.

The Pillow results page has two searchable and sortable tables.
**Individual operations** lists the declared single-operation workloads, with
operation classes such as point, multi-image, geometry, neighborhood, and draw.
Each row retains its image mode and size. **Composed pipelines** holds pipeline
chains, matrices, lifecycle, and quick workflows. Search matches operation or
pipeline names, workload IDs, modes, and sizes; type filters narrow each table,
and timing headers sort numerically. Parallel CPU is its own column, built with
the opt-in `parallel` Cargo feature and compared directly with the ordinary
Pillow oracle. It is not relabeled as SIMD or merged into serial CPU.

The public Benchmark workflow uses the full `standard` fixture selection, then
runs the same selected workloads with the separately compiled Parallel CPU
profile. The exporter combines only matching revisions, manifests, input
documents, workload sets, modes, and repeat policies; it rejects a mismatched
pair instead of producing misleading side-by-side values. Reproduce both runs
and export the combined view with:

```sh
MIGRATION_BENCHMARK_PROFILE=standard make migration-parity-benchmark
MIGRATION_BENCHMARK_PROFILE=standard make migration-parity-benchmark-parallel-cpu
DOCS_BENCHMARK_PARALLEL_SOURCE=build/migration-parity/benchmark-result-parallel-cpu.json \
  make docs-benchmark DOCS_BENCHMARK_OUTPUT=build/migration-parity/public-benchmark.json
```

The Parallel CPU run includes the ordinary Pillow oracle; the public view uses
the ordinary Pillow subject as its single baseline and adds only the
`python-parallel-cpu` measurements from that second run. The original source
receipts remain separate artifacts so each build's feature identity and parity
results can be inspected.

The committed snapshot is historical: source
`3a3ae28b20b0d86eebc1dddb45234f2da65af6b7`, measured 2026-09-12. It contains
11 workloads and 44 subject rows. Its original result SHA-256 is
`50431a65122edb1341a5ac2253a515b4992a0dfd95dd57dc7778fe2d06eebba7`.
The associated unchanged five-percent comparison reported three timing-only
violations: SIMD draw-batch RGB shapes, CPU SIMD-constant 1024x768, and the Pillow
terminal-read CMYK workflow. Its budget SHA-256 is
`3e85f936dfe56a06b9ea3f6128d09dfdd2400dadddc2e6de634cc83327ff4b97`.
The required consecutive zero-violation comparisons remain open. This snapshot
is not a timing measurement of release 0.1.3.

After running the maintained benchmark, export its public view:

```sh
make docs-benchmark
make docs-build
```

The exporter retains the original report hash, revision, environment, policy,
and all subject outcomes, while omitting local paths and hostnames. Full result
and parity receipts remain CI artifacts. The Benchmark workflow publishes a
public data artifact; the Documentation workflow validates and renders that data
using site code from main. Site publication does not change benchmark budgets.

## Reading the comparison table

The public page places one workload on each row and implementations in columns.
Column headings sort the underlying time values, independent of the displayed
ns/µs/ms units. Workload and implementation filters retain the baseline and update
the visible comparison counts. Details expose sample counts, percentiles and
recorded correctness; the full measurements and JSON remain downloadable.

A speed factor is baseline median divided by project median. For example,
20 µs versus 10 µs is **2× faster**; 10 µs versus 20 µs is **2× slower**.
Each row's bars share a linear scale; different workloads do not share a scale.
Counts and the lowest-median highlight describe observations, not a statistical
significance test or an overall project score. Small differences may be noise.
Timing-only rows remain labeled; failed execution, differing output, missing
baseline, incompatible measurement policies and unconfirmed GPU completion do
not receive a comparative speed label. A fallback names the backend actually used.
The UI never changes samples, acceptance thresholds or correctness results.

The presentation takes cues from [Artificial Analysis](https://artificialanalysis.ai/methodology)
and [MLPerf Endpoints](https://mlcommons.org/benchmarks/endpoints/): make the
comparison clear while keeping task, environment and quality context visible.
These projects are references for presentation, not validators of these results.

### Rejected local RGB UnsharpMask vertical specialization

The c22 verified CPU matrix ranks material RGB `UnsharpMask(radius=2,
percent=150, threshold=3)` at 1.051× Pillow on macOS ARM, 1.938× on Ubuntu
ARM, and 2.500× on Ubuntu x86_64. A fresh local profile confirmed the
native RGB Gaussian blur dominates the CPU stack; one trial specialized its
radius-one fractional vertical pass. The row walk reused the leaving sample
for the left fractional tap and matched the generic fixed-point pass byte for
byte on tiny and ordinary shapes.

The same 100-sample correctness-gated workflow passed exact parity in CPU,
SIMD, and GPU lanes. Local baseline CPU/Pillow medians were 9.022/9.754 ms
(1.081×); the two candidate pairs were 8.706/9.508 ms (1.092×) and
8.733/9.476 ms (1.085×). All 100 target samples used their requested backend
with complete terminal receipts and no fallback. The raw CPU latency decrease
tracked Pillow's run-to-run shift, so the Pillow-relative result did not
improve and the candidate was reverted. GPU single-request latency remained
below SIMD, but sustained throughput is still unmeasured. No genuine test
defect was found; no coverage was run.

### Local RGB UnsharpMask horizontal-row candidate

A second CPU trial specializes the three horizontal RGB Gaussian passes when
their integer radius is one and the fractional weight is nonzero. It keeps the
three channel accumulators explicit and reuses the leaving sample as the left
fractional tap. The focused scalar comparison covers widths 5, 6, 7, 17, 65,
and 1024; the existing fused-versus-full-frame test covers RGB multi-pass
rounding and small rows.

The same 100-sample public workflow passed exact Pillow parity in all three
backend lanes. The clean local CPU/Pillow median pair was 9.022/9.754 ms
(1.081×); two candidate pairs were 7.237/8.487 ms (1.173×) and 7.168/8.512
ms (1.188×). Each target lane ran 100/100 times on its requested CPU, SIMD, or
GPU backend with complete terminal receipts and no fallback. The candidate
improves the local CPU/Pillow ratio by 8.5–9.8%, but still misses the 2×
sequencing target. GitHub benchmark run [#124](https://github.com/appunni-m/pillow-rs/actions/runs/37894520226)
completed on candidate revision `3e559e4` and confirmed exact parity with 100
actual-backend samples across hosts. CPU/Pillow speedups were 1.330× on macOS
ARM, 2.018× on Ubuntu ARM, and 2.561× on Ubuntu x86_64. SIMD/Pillow measured
1.266×, 1.719×, and 3.693× respectively; GPU/SIMD latency measured 1.430× on
macOS ARM. GPU sustained throughput remains unmeasured. No genuine test defect
was found; no coverage was run.

### Local BoxBlur(1) mapped RGB input and queued GPU throughput

On the local macOS arm64 Metal device, native RGB BoxBlur(1) already had a
compact transfer and fused one-dispatch shader, but its eager route still
uploaded through `write_buffer_with`. Using the existing shared-storage mapped
input path reduced the exact 1024×768 RGB workload's GPU median from the prior
1.388 ms sample to 1.004 ms while preserving byte parity. The final rebuilt
run measured Pillow / CPU / SIMD / GPU at 2.748 / 1.736 / 1.187 / 1.021 ms.
Each target reported 100/100 executions on its requested backend without
fallback; GPU used one dispatch, uploaded and read back 2,359,296 bytes each,
and materialized one checked 2,359,296-byte output allocation. CPU and SIMD
each reported two checked buffers totaling 2,368,512 bytes. SIMD was 2.31×
Pillow, and GPU latency was 0.86× SIMD.

The explicit `GpuBatchExecutor` has a separate upload path, so changing the
eager route alone did not improve its RGB throughput. For singleton radius-one
L/RGB BoxBlur graphs on devices that support shared primary-buffer mapping, its
existing input slot now uses `MAP_WRITE | STORAGE`; all other graphs and
unsupported devices retain `write_buffer_with`. This avoids an additional
source buffer. Two 1024×768 grouped runs passed exact output-digest comparison
against Pillow, CPU, SIMD, and eager GPU, with eight jobs per submission and
one actual shader dispatch per job. Queued GPU/SIMD images-per-second ratios
were 1.64× and 1.29× for L, and 1.42× and 1.13× for RGB. The 7×5 L/RGB tail
run also matched exactly and reported one dispatch per job; its throughput is
not meaningful at that size.

These local results establish a GPU latency and grouped-throughput gain for
BoxBlur(1) at the measured modes and sizes, not completion of BoxBlur. The
serial CPU RGB result is 1.58× Pillow and still misses the 2× sequencing goal;
other radii, modes, sizes, composed pipelines, and a clean complete matched
Pillow-SIMD JSON cohort remain outstanding. Keep the canonical c25 operation
matrix unchanged and the Kata item open. This work is not counted in the
33-operation push batch. No genuine product or test defect was found; no
coverage was run.

### Rejected static SIMD receipt counter for CMYK getprojection

A candidate removed the per-block SIMD/scalar receipt-counter updates and
derived those counts from row width and image height. The focused six-test
CMYK module passed, and three whole-workflow sparse-CMYK runs each passed all
three CPU/SIMD/GPU parity comparisons with requested backends and no fallback.
Candidate Pillow/CPU/SIMD/GPU medians in microseconds were
430.167/189.083/208.667/432.376, 407.000/187.729/209.000/431.167, and
446.416/188.416/209.771/430.438. After restoring the per-64-byte-block
counter batching, three control runs measured 406.146/208.188/166.021/454.604,
440.250/188.500/180.875/430.520, and 436.688/212.896/174.875/432.895. These
are separate cohorts, so the timing delta is directional, but every static
counter run was slower on SIMD than every restored control. The candidate was
reverted; no cause for the regression was established. Keep the exact backend
receipt and reject this change based on whole-workflow latency, not parity
alone. The candidate and controls are in the local ledger and ignored
`build/performance-optimization/` artifacts. No coverage was run.

### Local CMYK getprojection terminal fusion

For the 1024×768 sparse-CMYK pipeline, three queued `PutPixel` operations feed
`getprojection`. The terminal CPU and SIMD paths apply those writes to the two
projection axes directly, preserving last-write-wins behavior and avoiding a
materialized edited frame. The SIMD scanner keeps the native four-byte CMYK
carrier and screens 64-byte blocks with `wide::u8x16`; tails and nonzero blocks
are handled without converting the image mode.

On little-endian Metal devices with shared primary-buffer mapping, the GPU
route writes the source into a reusable `MAP_WRITE | STORAGE` buffer. Other
devices keep the queue upload. The retained GPU kernel scans one 16×16 pixel
tile per workgroup and atomically marks both axes after clearing the compact
output range. Two local macOS 15.7.7 arm64 release runs passed exact Pillow
parity. Pillow / CPU / single-thread SIMD / GPU medians were 0.408416 / 0.232792
/ 0.185542 / 0.455354 ms, then 0.383229 / 0.193063 / 0.185041 / 0.456542 ms.
CPU stayed faster than Pillow but did not reach 2× in either whole-workflow
sample; SIMD exceeded 2× Pillow in both. GPU was about 2.45× slower than SIMD.
Each GPU run executed 100/100 times on GPU with no fallback and one dispatch.
Upload was 3,145,728 bytes, readback was 7,168 bytes, and two checked host
output buffers totaled 7,168 bytes; the reusable device working set retained
9,443,600 bytes. Per-call concurrency-one throughput is not sustained GPU
throughput evidence.

The atomic scan improved on two parity-gated runs of the mapped cooperative
row-reduction kernel, whose GPU medians were 0.616959 and 0.610479 ms. The
cooperative kernel emitted both axis vectors directly. A compact atomic-bitset
candidate reduced readback to 224 bytes but did not improve end-to-end latency;
fewer output bytes alone are not evidence of a faster call. A sequential
per-axis scan was slower and was discarded. A host all-zero check removed the
3,145,728-byte upload, but the parity-gated GPU median was 1.354 ms versus
1.045 ms in a separate earlier run, so that fast path was reverted. These
separate runs are directional only; keep candidates gated on same-run Pillow,
CPU, SIMD, and GPU samples. Focused GPU regressions cover repeated writes,
zero overwrites on zero and nonzero sources, and wide-row boundaries; other
modes, sizes, and shapes remain outstanding.

### Direct RGB getprojection routing

Direct `Image.getprojection` on a materialized image previously bypassed
pipeline backend routing. It now selects the active eager backend without
allocating the sorted backend list, records CPU/SIMD/GPU receipts, and uses a
native RGB SIMD scanner or a shared packed RGB/RGBA GPU projection shader when
supported. Unsupported modes and source sizes still report CPU fallback; they
do not count as accelerated execution.

The 16×16 black RGB standard workload passed exact Pillow parity. The corrected
whole-workflow run `migration-benchmark-3616c55b14ca478c988186ff28fe78eb`
(parity evidence `migration-parity-benchmark-gate-8ef362a03c074e3da7faaafd6f77fd72`)
measured Pillow / CPU / single-thread SIMD / GPU medians of 0.005041 / 0.004750
/ 0.004541 / 0.265625 ms. CPU and SIMD were 1.061× and 1.110× Pillow, so they
meet the Pillow baseline but remain below the 2× sequencing goal and SIMD's 5×
goal. GPU speedup versus SIMD was 0.0171× (GPU latency was 58.5× SIMD). There
were 100 timed latency samples per subject; each target also had a separate
terminal-complete route proof with one receipt, the requested backend matching
the actual backend, and no fallback.
GPU used one dispatch, uploaded 768 bytes, read back 128 bytes, and transferred
32 parameter bytes. Its concurrency-one rate is not sustained GPU throughput
evidence.

Two benchmark-harness defects inflated target-only timing: the initial run
enabled pipeline telemetry during timing, and a later run still called
`lock_active_backend()` inside every timed target workflow. That lock was a
no-op for this eager materialized image; an isolated CPython 3.12 loop measured
the lock helper at about 276 ns per call. The benchmark adapter now measures
the unforced route under one active backend and proves it separately with a
matching no-fallback receipt. Separate-run timing variation prevents assigning
the entire previous gap to the lock. Earlier local latencies (0.005021 /
0.005709 / 0.005459 / 0.274709 ms and 0.004625 / 0.005458 / 0.005250 /
0.274771 ms) are historical and must not be used for backend comparisons.

A separate 24×4 nonzero RGB diagnostic returned the expected projections on
all three backends. Its SIMD receipt recorded 10 vector blocks and 32 scalar
tail pixels. The GPU receipt named `getprojection_atomic.wgsl`, reported one
dispatch across two workgroups, uploaded 288 bytes, and read back 112 bytes.
This diagnostic has no paired Pillow gate, so only the black standard workload
is claimed as exact Pillow parity. A bytewise all-zero early scan was tested
and reverted after it did not reduce the whole-call CPU or SIMD medians.

The full published matrix now uses the clean ff0009 snapshot; its separate
Pillow-SIMD JSON remains dirty/partial and has no getprojection comparison.
The public standard workload is still missing from that published snapshot,
so retain the new local RGB result as separate local evidence and keep the Kata
item open. The first attempted benchmark selected the `pipeline` profile,
which correctly rejected this standalone workload; the follow-on Make recipe
also tried to validate the absent output and emitted a secondary
`FileNotFoundError`. This was a workload-selection error plus a benchmark
recipe diagnostic defect, not a parity failure. No coverage was run.

### Sparse CMYK getprojection pipelines

The terminal CMYK path now recognizes a sole active CPU, SIMD, or GPU backend
on an unlocked pipeline. This preserves the no-lock benchmark profile while
admitting the same exact three-`PutPixel` fusion as an explicitly locked
pipeline. Two 1024×768 macOS arm64 whole-workflow repeats passed exact Pillow
parity. Pillow / CPU / single-thread SIMD / GPU medians were 537.376 / 213.875
/ 205.146 / 477.042 µs and 413.042 / 223.104 / 205.958 / 444.980 µs. Both
target runs completed with the requested backend, terminal receipts, three
fused operations, and no fallback. CPU and SIMD had no full-frame copy or
transfer. GPU used one dispatch, uploaded 3,145,728 bytes, read back 7,168
bytes, and copied one full frame; its latency remained above SIMD. Sustained
GPU throughput is unmeasured, so the operation remains open.

Two zero-input pre-scans were rejected using the same end-to-end workload. A
scalar byte scan raised CPU latency from 188.354 to 899.417 µs while adjacent
Pillow medians stayed near 0.41 ms. A SIMD byte-stream scan raised SIMD
latency from 205.958 to 262.438 µs with adjacent Pillow medians of 406.584 and
413.042 µs. Keep the ordinary projection scans. The per-run results, including
the route-missing intermediate diagnostic, are preserved in
[`performance-optimization-local-runs.csv`](evidence/performance-optimization-local-runs.csv)
and the ignored `build/performance-optimization/getprojection-*.json`
artifacts. The local arm64 runs do not validate the published x86 SIMD gap or
the separate Pillow-SIMD comparison.

### Profile-complete parity and sparse-write repair

The first profile-complete benchmark run found two separate defects. The
benchmark subjects included CPU, SIMD, and GPU, but their parity input selected
only CPU; the SIMD and GPU timings therefore lacked a correctness gate. The
benchmark now points to dedicated parity cases that execute all three target
profiles. The repaired three-workload run passed all 9 selected comparisons,
and a sparse-workload repeat passed all 3 profile comparisons.
The GPU edge tests for wide-row zero overwrites and a nonzero source passed
(2/2), and after restoring the 64-byte SIMD grouping all six tests in
`getprojection_cmyk_tests` passed. `make fmt clippy` and `make docs-lint`
completed; Clippy reported warnings but exited successfully. No coverage was
run.

That expanded gate caught a GPU correctness bug: when every queued `PutPixel`
write was nonzero, the shader scanned the source image but never added the
positive writes to either projection. The GPU returned all-zero projections
for the sparse CMYK case. The shader now processes the bounded positive-write
list in an extra workgroup row within the same dispatch. The same shader path
still applies the final value for duplicate writes when zero overrides are
present. The original failed parity artifact is retained under
`build/performance-optimization/getprojection-all-profile-parity-current-parity.json`;
the repaired artifacts are `getprojection-all-profile-parity-fixed*.json` and
`getprojection-sparse-postfix-repeat*.json` in that build directory.

In the first repaired run, Pillow / CPU / SIMD / GPU medians were 5.209 / 4.917
/ 4.667 / 233.500 µs for the 16×16 RGB workload, 372.688 / 9.792 / 8.459 /
8.375 µs for dense 1024×768 CMYK, and 462.729 / 219.604 / 216.813 / 511.458 µs
for sparse 1024×768 CMYK. In the repeat sparse run they were 409.688 / 222.334
/ 240.271 / 454.688 µs. Every target output matched Pillow in the selected
parity comparisons. The dense-CMYK GPU request fell back to CPU with the
reported reason `GPU getprojection is unsupported for mode CMYK`; keep it out
of GPU performance ratios. The RGB GPU path executed on GPU but was about 50×
slower than SIMD. The sparse GPU path executed on GPU with one dispatch, a
3,145,728-byte upload, 7,168-byte readback, and one full-frame copy; it remained
1.89–2.36× slower than SIMD. Concurrent sustained GPU throughput is unmeasured.
The completed-work batch harness currently covers median, BoxBlur, and
grayscale, not getprojection. The current getprojection timing is a sequential
concurrency-one series, so its reciprocal-latency rate is not a sustained
throughput result. Do not use unrelated batch-operation rates as a proxy.

These two local sparse runs vary enough that they do not establish a stable 2×
CPU or SIMD result. Keep the operation open, retain both runs, and continue
prioritizing its GPU and weakest CPU/SIMD rows. The local results do not satisfy
cross-platform, all-mode, or Pillow-SIMD obligations. No coverage was run.

Two SIMD block-size candidates were rejected on the same sparse workload. A
256-byte scan block passed 3/3 parity comparisons and measured Pillow / CPU /
SIMD / GPU at 482.875 / 219.376 / 256.604 / 442.792 µs. A 128-byte block also
passed 3/3 and measured 437.083 / 201.458 / 253.021 / 431.271 µs. The retained
64-byte grouping measured SIMD at 216.813 and 240.271 µs in the preceding
parity-gated runs. These separate runs have control variation and prove no
gain from the candidates; restore the 64-byte grouping. Keep both rejected
results in the local ledger and do not claim a SIMD improvement.

### Combine sparse CMYK SIMD block checks

A second candidate kept 64-byte groups but ORed their four 16-byte SIMD
vectors before one zero comparison. It passed the same sparse whole-workflow
parity gate in three runs (3/3, 3/3, and 6/6 comparisons when the dense CMYK
control was included). Sparse SIMD medians were 195.563, 192.501, and 192.896
µs, versus 216.813 and 240.271 µs in the preceding retained 64-byte runs.
Same-run Pillow controls put these three SIMD results at 2.28–2.32× Pillow.
The 1024×768 dense-CMYK SIMD control measured 8.583 µs, close to the preceding
8.459 µs result; its GPU request fell back to CPU and remains excluded.

Keep the combine-before-reduce candidate: its three sparse results are
tightly grouped and beat both earlier 64-byte controls, while the dense control
showed no material shift. GPU still ran one dispatch, uploaded 3,145,728 bytes,
read back 7,168 bytes, and remained 2.48–2.56× slower than SIMD. No sustained
throughput result exists. The local SIMD speedup clears 2× for this sparse ARM
case but does not meet the stricter 5× target; the published x86 SIMD gap,
small RGB case, other modes and sizes, and the GPU target remain open. Retain
the operation as an incomplete item and keep these measurements separate from
the canonical published matrix.

### Batch CMYK SIMD projection receipt counter updates

The 64-byte scan callback counted its four 16-byte vectors with a saturating
counter update inside the inner loop. Move that update to the scan-block level
and add the number of vectors in that block once; the SIMD work and reported
vector count stay the same. Three 1024×768 sparse-CMYK whole-workflow runs
passed exact CPU/SIMD/GPU parity (3/3 each, 100 timed samples per subject).
SIMD medians were 163.916, 164.937, and 165.605 µs; paired Pillow medians were
416.480, 436.271, and 408.520 µs, giving 2.467–2.645× speedups. The preceding
combine-before-reduce runs measured SIMD at 192.501–195.563 µs and 2.277–2.321×
Pillow. The new SIMD results are tightly grouped and their paired ratios are
higher, though these were separate runs rather than an interleaved A/B test.

In the same new runs, CPU measured 188.167–197.333 µs (2.110–2.314× Pillow).
GPU executed without fallback at 450.396–466.646 µs, 2.73–2.82× slower than
SIMD, with one dispatch, 3,145,728 uploaded bytes, and 7,168 readback bytes;
sustained throughput remains unmeasured. The six focused CMYK projection
tests passed, including the SIMD execution receipt test. Keep this as a local
ARM candidate only: the canonical x86_64 sparse-CMYK SIMD row remains 0.258810×
Pillow and has not been rerun with this change. The operation remains open, and
these local results do not establish its all-mode, cross-platform, 5× SIMD, or
GPU targets.

A follow-up grouped the same 64-byte scan block into two `u8x32` values. Its
exact parity gate passed 3/3, but the single 100-sample run measured SIMD at
165.209 µs, inside the counter-batched candidate's 163.916–165.605 µs range.
It did not establish a gain, so restore four `u8x16` values and retain the
counter batching only. The rejected run is kept in the local ledger.

### Sparse CMYK GPU projection from a zero source

The queued sparse-CMYK case starts from a zero-filled frame and then applies
three pixel writes. A GPU path can verify that source with the 16-byte SIMD
preflight and let the existing single dispatch process only deduplicated
nonzero writes; it need not upload or scan the complete frame on the GPU. The
nonzero-source path remains unchanged. Three exact-parity whole-workflow runs
passed 3/3 comparisons each with actual GPU execution and no fallback. Pillow /
CPU / SIMD / GPU medians (µs) were 418.521 / 188.771 / 164.542 / 443.062,
418.270 / 188.792 / 171.708 / 456.479, and 415.084 / 198.896 / 171.772 /
449.209. GPU used one dispatch, uploaded zero source bytes, read back 7,168
bytes, and recorded no full-frame copy. These separate runs show only a small
local latency change from the preceding GPU controls; they are not an
interleaved A/B comparison. GPU remained 2.62–2.69× slower than SIMD, and no
sustained-throughput measurement exists. Keep this only as a local transfer
and work reduction; it does not meet the GPU target or complete the operation.

A scalar bytewise zero-source check passed parity but raised whole-workflow GPU
latency to 1,249.438 µs, so it was replaced with the SIMD preflight. The focused
CMYK projection module passed all six tests, including zero- and nonzero-source
GPU overwrite cases. Results are in the local run ledger; the published matrix
is unchanged.

### Local grayscale CMYK CPU follow-up

The full published matrix still ranks 1024×768 YCbCr grayscale on Ubuntu
x86_64 as the weakest verified serial CPU row at 1.000848× Pillow. A separate
macOS 15 arm64 diagnostic measured `pipeline-chain.grayscale.material-cmyk-1024x768`
macOS 15 arm64 diagnostic before and after CMYK-to-L optimization. The retained
CPU path removes redundant clamps, writes through the output iterator, and
fuses CMYK channel conversion with Pillow luma using weights whose sum is
65,536. The fused expression passed exact parity in two repeated full-call
plus materialization runs, each with 100 timed samples and a separate proof of
actual CPU, SIMD, and GPU execution, complete terminal output, and no fallback.
CPU medians were 306.854 and 304.250 µs versus Pillow at 654.855 and 615.250 µs
(2.134× and 2.022×). This is 14.1% below the repeated iterator-only candidate
and 18.9% below the single pre-change CPU run at 376.708 µs. SIMD stayed near
Pillow at 645.291 and 616.500 µs, while GPU was 10.6–16.8% slower than SIMD;
sustained GPU throughput is unmeasured. The per-run receipts and resource
counts are preserved separately in
[`performance-optimization-local-runs.csv`](evidence/performance-optimization-local-runs.csv);
the hosted ranking matrix is unchanged. Keep grayscale open: this one CMYK
workload does not clear the hosted x86 YCbCr gap or the SIMD and GPU targets.
A candidate replacing `saturating_sub` for byte K with ordinary subtraction
passed parity, but its paired CPU/Pillow ratios varied from 1.959× to 1.663×
under broad host timing variation; it was reverted and logged as inconclusive.
No coverage was run.

### Local CMYK grayscale output-collection follow-up

The validated CMYK grayscale source has exactly four bytes per output pixel.
Collecting the final luma iterator avoids zero-initializing an output buffer
that is immediately overwritten. Four release repeats of the 1024×768
`pipeline-chain.grayscale.material-cmyk-1024x768` workload timed the public call
and result materialization with 100 samples per profile per run. All passed
exact Pillow parity; CPU, SIMD, and GPU requested and actual backends matched,
each completed its output, and none reported fallback. Per-run medians in
microseconds (Pillow / CPU / SIMD / GPU)
were 634.500 / 302.562 / 236.667 / 719.542, 594.313 / 296.396 / 217.500 /
631.584, 598.896 / 296.230 / 232.875 / 716.729, and 614.125 / 317.792 /
233.000 / 716.708. The paired CPU/Pillow speedups were 2.097×, 2.005×,
2.022×, and 1.932× (median 2.013×); only three of four runs cleared 2×, so
the local result does not establish a stable 2× margin. SIMD/Pillow ranged
from 2.572× to 2.732×, below the repository's 5× target. GPU latency was
2.90×–3.08× SIMD latency; each GPU call dispatched once, uploaded 3,145,728
bytes, and read back 786,432 bytes. The queued-window evidence also remains
below SIMD throughput. Keep the small allocation change, but leave grayscale
open and retain every run in the local ledger. These local measurements do not
replace the hosted matrix or prove other modes, sizes, or architectures. No
coverage was run.

### Rejected CPU RGB delta-luma formulation

The RGB delta-luma helper passed the exhaustive test over all 16,777,216 RGB
triples for both supported rounding policies. Enabling it for the portable
1024×768 RGB grayscale CPU path also passed exact Pillow parity, but its
end-to-end CPU medians were 158.167 and 161.730 µs around a matched direct
weighted-sum baseline at 101.083 µs. Paired Pillow medians stayed at 189.104,
193.083, and 194.104 µs; SIMD stayed near 93–95 µs and GPU near 586–591 µs.
The CPU candidate was 56–60% slower than the direct formula on both A/B/A
candidate runs. It was reverted; the test-only helper remains available for
exhaustive checks. The three parity gates, route proofs, and resource receipts
are in
[performance-optimization-local-runs.csv](evidence/performance-optimization-local-runs.csv).
This Mac ARM RGB result does not replace the hosted x86 YCbCr ranking row or
complete grayscale across modes and backends. No coverage was run.

### Pending x86 YCbCr CPU gather candidate

The current published full benchmark JSON still has the x86
1024×768 YCbCr grayscale CPU row at 140.8945 µs versus Pillow at 141.014 µs
(1.000848×); its single-thread SIMD row is 81.1305 µs. The local CPU candidate
now uses the same sixteen-pixel SSSE3 Y-channel gather as the SIMD adapter on
x86 hosts that support SSSE3, while retaining the packed-word fallback. The
SIMD adapter now shares that helper. Native ARM YCbCr grayscale tests passed,
and the x86 target passed test type-checking and release-library compilation.
The macOS host cannot execute x86, and its Linux test binary could not link
with the host linker. This is compile evidence only: there is no new x86 parity,
backend receipt, or latency result, so the published matrix remains unchanged.
Keep the candidate pending until a compatible native x86 run proves parity,
actual CPU and SIMD execution, and end-to-end timings. No coverage was run.

### Grayscale GPU batching throughput

A local Metal diagnostic compared completed 16-image 1024×768 L/RGB grayscale
windows across Pillow, CPU, single-thread SIMD, eager GPU, and queued GPU. Exact
output bytes matched. Queued GPU reached 1,484 images/s for L and 1,169 for RGB,
versus SIMD at 17,999 and 7,254 images/s. Queueing improved over eager GPU by
2.7–3.0×, but neither queued throughput nor existing per-call GPU latency meets
the SIMD target. The single-kernel graphs transferred the source and materialized
the full grayscale output for every image. At 64 images/window, queued GPU
reached 1,775 images/s for L and 1,057 for RGB, versus SIMD at 16,632 and 7,305;
`max_in_flight=1` did not improve those rates. These are local
sustained-throughput measurements, not latency estimates or canonical matrix
rows. The smaller 256×256 workloads and transfer counters are in the [GPU batch validation
record](GPU_BATCH_VALIDATION.md#grayscale-completed-work-throughput-diagnostic-2026-10-09)
and the local evidence ledger. Grayscale GPU remains open. No coverage was run.

### Grayscale GPU 256-thread workgroup trial

The grayscale shader and its checked dispatch planner were changed together
from 64 to 256 invocations per workgroup. Exact output bytes passed for L and
RGB at 1024×768, odd 257×129, and 1×1. The queued GPU profile reported actual
GPU execution, all submitted jobs completed, and no fallback. Across eight
64-image measurement reports, queued GPU median throughput was 2,007 images/s
for L and 1,255 for RGB; the contemporaneous SIMD medians were 16,856 and
6,972 images/s. The separate four-report 64-thread cohort measured 1,759 and
1,153 queued GPU images/s, but its L SIMD control was 8,937 images/s versus
16,856 in the 256-thread cohort. That host-rate shift prevents treating the
raw GPU difference as a controlled causal gain. Keep the 256-thread code as
an exploratory candidate only; it has not demonstrated the sustained GPU
target.

A separate exact-parity single-call RGB run, including result materialization,
measured Pillow at 179.937 µs, CPU at 110.813 µs, SIMD at 104.938 µs, and GPU
at 771.041 µs. The GPU receipt showed requested/actual GPU, one dispatch,
completed terminal materialization, and no fallback. SIMD was 1.715× Pillow,
below the requested 2× target; GPU was 0.136× SIMD speed. The GPU path uploaded
2,359,296 bytes and read back 786,432 bytes for the operation. Its median
shader pipeline phase was 4.209 µs while its terminal phase was 766.188 µs,
indicating that synchronization and output materialization dominate this
end-to-end measurement. The workgroup trial does not close the GPU latency or
throughput gap, and grayscale remains open. Reports and per-profile details
are recorded in the local evidence ledger and ignored
`build/performance-optimization/` artifacts. No coverage was run.

### YCbCr grayscale 128-thread workgroup trial — rejected — 2026-10-10

For `pipeline-op.grayscale.material-ycbcr-noise-1024x768`, the 128-thread
candidate changed the WGSL workgroup size and checked host dispatch planner
together. The first parity attempt exposed a stale `gid.y` stride still
multiplied by 256, causing GPU output words to be skipped. This was a candidate
implementation bug, not a test defect. After the shader stride was corrected,
the candidate passed all three CPU/SIMD/GPU parity comparisons with actual
requested backends and no fallback.

The 256-thread control measured Pillow/CPU/SIMD/GPU at
108.875/44.458/44.375/386.500 µs. The corrected 128-thread candidate measured
116.979/43.437/45.250/384.021 µs. Pillow and SIMD shifted upward; GPU fell by
0.6%, and GPU/SIMD worsened from 0.1148× to 0.1178×. There is no normalized
target gain, so restore the 256-thread setting. Both GPU calls used one
dispatch, 2,359,296 uploaded bytes, and 786,432 readback bytes. This trial did
not measure sustained throughput; the separate grayscale queue diagnostic
remains below SIMD. Full results are in the local evidence ledger and reports
`grayscale-ycbcr-wg256-control-20261010.json` and
`grayscale-ycbcr-wg128-corrected-20261010.json` under
`build/migration-parity/`. The failed-stride parity receipt is retained as
`grayscale-ycbcr-wg128-stride256-failed-parity-20261010.json`. No coverage was
run.

### Rejected AArch64 scalar contribution-table grayscale

An AArch64 CPU trial enabled the existing three 256-entry weighted-contribution
tables for RGB grayscale. Both exact-parity 1024×768 runs routed to actual CPU,
completed materialization, and reported no fallback. CPU medians were 451.750
and 451.250 µs against paired Pillow at 215.021 and 189.145 µs (0.475× and
0.419×), versus 110.813 µs for the retained direct-formula CPU path in the
preceding matched workload. SIMD and GPU controls did not show a similar
regression. Revert the table lookup on AArch64 and keep the scalar weighted
formula; no cause beyond the measured slowdown is established. The post-revert
exact-parity confirmation measured Pillow at 193.312 µs, CPU at 108.000 µs,
SIMD at 86.062 µs, and GPU at 587.271 µs, with requested and actual backends
matching and no fallbacks. CPU remains below 2× Pillow in this run; grayscale
stays open. Per-profile rows and reports are in the local evidence ledger and
ignored benchmark artifacts. No coverage was run.

### Rejected AArch64 scalar RGB grayscale unroll

A four-pixel unroll of the serial RGB grayscale loop also preserved exact
Pillow output and ran on actual CPU with complete materialization and no
fallback. Two 1024×768 runs measured CPU at 377.188 and 417.125 µs against
paired Pillow at 196.188 and 226.292 µs (0.520× and 0.543×), far slower than the
retained direct-formula medians of 108.000 and 124.125 µs from the adjacent
reverted runs. Revert the unroll; do not infer a benefit from fewer loop
iterations. The final post-revert run again passed exact parity and measured
Pillow at 230.792 µs, CPU at 124.125 µs, SIMD at 104.417 µs, and GPU at
664.208 µs. CPU remains faster than Pillow but below 2×; SIMD clears 2× in this
repeat, while GPU latency remains over six times SIMD. Grayscale stays open.
Rows and reports are in the local evidence ledger and ignored benchmark
artifacts. No coverage was run.

### NEON SIMD CMYK grayscale

The CMYK SIMD kernel now uses an AArch64 NEON four-channel structure load to
deinterleave each sixteen-pixel block before the exact CMYK conversion and
Pillow luma calculation. Its intermediate bounds fit in unsigned 16-bit lanes;
the incomplete final block remains zero-padded and writes only active pixels.
The existing vector-tail regression passed. Three live-oracle benchmark runs
used the complete grayscale call-plus-materialization boundary and exact Pillow
parity. The pre-change SIMD median was 576.291 µs versus Pillow at 613.333 µs
(1.064×). Candidate runs measured SIMD at 236.875 and 237.145 µs, versus paired
Pillow at 739.500 and 663.479 µs (3.122× and 2.798×). Requested and actual
backends matched with no fallbacks and complete outputs. A separate telemetry
proof reported the vector path with 49,152 blocks and zero scalar tails. This
clears the 2× SIMD target for the 1024×768 CMYK workload on this ARM host; it
does not clear grayscale's other modes, CPU x86 ranking row, or GPU latency and
throughput targets. Keep the full operation open and the canonical hosted
matrix unchanged. Per-run results are in the local evidence ledger. No coverage
was run.

### NEON direct-store RGB grayscale

For 1024×768 RGB noise on macOS ARM, the fresh pre-change SIMD median was
111.229 µs versus Pillow at 236.292 µs (2.124×). The AArch64 path now stores
complete `vld3q_u8` grayscale vectors directly into the final output `Vec`,
avoiding the sixteen-byte temporary and copy per block. It sets the vector
prefix length only after all full blocks are initialized and pads the final
partial block before appending only active pixels.

Six correctness-gated call-plus-result-materialization repeats passed exact
Pillow parity and reported requested/actual SIMD with no fallback. SIMD medians
were 94.083, 99.938, 91.834, 115.979, 92.042, and 99.271 µs; paired Pillow
medians were 247.750, 178.646, 193.979, 205.958, 191.708, and 253.166 µs. The
2× comparison passed in four repeats and missed in two (1.788× and 1.776× on the
misses), so the candidate remains a partial local gain, not a demonstrated
target pass. Its six-run median was 96.677 µs, about 13.1% below the fresh
baseline. The focused
vector-tail regression passed on this AArch64+NEON host and checked exact
results plus vector-block counts for full blocks and tails. These local results
do not change the hosted matrix or close grayscale across its other modes,
sizes, CPU cohorts, and GPU goals. Full reports and per-profile receipts are in
the local evidence ledger and ignored `build/performance-optimization/`
artifacts. No genuine test defect was found and no coverage was run.

### Rejected NEON 64-pixel RGB loop unroll

Two correctness-gated repeats unrolled four sixteen-pixel blocks in the
NEON direct-store loop. Exact Pillow parity passed, requested/actual SIMD
matched, fallbacks were zero, and materialization completed. SIMD measured
106.917 and 100.625 µs versus paired Pillow at 260.958 and 216.292 µs (2.441×
and 2.149×), but both SIMD times were slower than the retained direct-store
five-run median of 94.083 µs. Revert the loop unroll; passing the 2× ratio in
two runs does not make it a latency improvement over the retained kernel. The
post-revert direct-store confirmation measured 99.271 µs with exact parity.
This candidate remains separate from the direct-store cohort. No genuine test
defect was found and no coverage was run.

### Rejected widened-multiply RGB SIMD candidate

A second RGB trial kept the direct NEON stores but replaced the exact `u16`
base/residual arithmetic with widened `u32` multiply-accumulate and a rounded
shift. Its focused vector-tail regression and live-Pillow parity gate passed,
and the receipt showed actual SIMD, no fallback, and completed materialization.
On the same 1024×768 workload, the candidate SIMD median was 139.229 µs versus
Pillow at 216.333 µs (1.554×), 43.5% slower than the retained direct-store
kernel's four-run median of 97.011 µs. The widened arithmetic was rejected and
the exact `u16` decomposition restored; a follow-up parity-gated run measured
the retained SIMD path at 92.042 µs. No cause for the widened path regression
was established, so do not generalize beyond this measured cohort. Full report
and receipts are in the local evidence ledger and ignored benchmark artifacts.
No genuine test defect was found and no coverage was run.

### AArch64 scalar RGB grayscale coefficient decomposition

The rounded AArch64 CPU path now splits Pillow's exact coefficients as
`19595=77*256-117`, `38470=150*256+70`, and `7471=29*256+47`. Adding the
rounding bias before subtracting the red residual keeps every `u16`
intermediate within 2,933..62,603; the base is at most 65,280 and the final
base-plus-carry at most 65,524. This preserves the full integer formula without
unsigned underflow.

The existing exhaustive RGB arithmetic test checked alternate delta and table
helpers but did not call the production `grayscale_rgb_pixel` function. It now
checks that function too. The focused Rust test passed all 16,777,216 RGB
triples for both rounding modes on macOS arm64 in 0.06 seconds. This was a
genuine parity-test coverage gap; no coverage instrumentation was run.

Four release benchmark repeats used the full
`pipeline-op.grayscale.material-rgb-noise-1024x768` call-plus-result-observation
boundary, 100 measured calls per profile, single-thread execution, and live
Pillow parity. Every target receipt completed on its requested backend with no
fallback. Per-run Pillow/CPU/SIMD/GPU medians in microseconds were
196.416/91.750/92.125/595.771,
198.666/91.750/92.083/595.021,
197.145/91.625/92.166/623.917, and
251.834/116.250/117.000/677.855. Paired CPU/Pillow speedups were 2.141× to
2.166× (median 2.158×); SIMD/Pillow was 2.132× to 2.158× (median 2.146×).
The CPU candidate therefore clears the user's 2× checkpoint for this local
RGB workload. SIMD remains below the repository's stricter 5× target.

GPU latency remained 0.148× to 0.173× of SIMD latency. Each GPU call used one
actual GPU dispatch, uploaded 2,359,296 bytes, and read back 786,432 bytes; the
separate queued-window diagnostic also remains below SIMD sustained throughput.
Keep this AArch64 CPU candidate, but leave grayscale open across remaining
modes, sizes, x86 CPU/Pillow-SIMD comparison, the 5× SIMD target, and GPU
latency/throughput. Full reports and per-profile receipts are recorded in
[`performance-optimization-local-runs.csv`](evidence/performance-optimization-local-runs.csv)
and ignored `build/performance-optimization/` artifacts. No commit, push, or
coverage run was made.

### Shared-Metal mapped input for grayscale GPU

On adapters that already support mapped primary Metal storage, the singleton
RGB/YCbCr grayscale path now writes its checked, four-byte-padded input into a
mapped storage buffer and keeps that buffer alive through dispatch. Other
adapters keep the existing queue upload. The strict GPU parity case for a
3×1 RGB image (an odd nine-byte input) passed, including the padded tail.

Four live-Pillow 1024×768 RGB repeats passed exact parity with actual GPU
execution, one dispatch, complete output materialization, and no fallback.
GPU medians were 353.021, 344.521, 339.688, and 339.229 µs; paired SIMD
medians were 99.521, 92.042, 92.083, and 91.958 µs. Compared with the previous
four-run upload cohort's median GPU latency of 609.844 µs, the mapped-input
cohort median was 342.104 µs (43.9% lower). This is a local cross-cohort result:
the first run's CPU/SIMD/Pillow controls shifted together, so the paired ratios
and exact backend receipts remain the evidence for this host. Upload and
readback volumes stayed at 2,359,296 and 786,432 bytes, with one reported
full-frame copy. GPU latency remains only 0.267×–0.282× of SIMD latency.

The completed-window follow-up used 64 images per window, two warmup windows,
three measured windows, and exact byte comparisons. RGB queued GPU throughput
was 1,203.9 images/s versus SIMD at 6,436.3; L queued GPU was 1,976.2 versus
SIMD at 15,904.4. GPU receipts reported 320 actual dispatches, 40 queued
submissions, largest submission eight, and zero live jobs after export. The
mapped input path improves the measured single-call GPU latency but does not
meet the sustained-throughput target. CPU and SIMD controls in this cohort
cleared the 2× user checkpoint in three of four repeats and missed it in the
first shifted-control run; this does not establish a consistent 2× result
across cohorts. Keep the candidate local to supported Metal grayscale and
leave the operation open. Per-profile rows are in the local ledger; the batch
report is in ignored `build/performance-optimization/`. No coverage was run.

### YCbCr and CMYK grayscale controls

The mapped RGB uploader also applies to native YCbCr triples. A separate
1024×768 parity-gated run measured Pillow/CPU/SIMD/GPU at
133.229/50.188/52.896/384.625 µs. CPU and SIMD exceeded 2× Pillow, but SIMD
remains below 5×; GPU latency was about 7.27× SIMD.

The CMYK path does not use the mapped RGB upload or the AArch64 RGB scalar
formula. Its 1024×768 parity-gated run measured Pillow/CPU/SIMD/GPU at
652.146/377.959/270.917/752.250 µs. Serial CPU remains below the user's 2×
checkpoint at 1.725×; SIMD is 2.407× Pillow and below 5×; GPU is slower than
Pillow and about 2.78× slower than SIMD, with one actual dispatch and no fallback. Keep
this mode's results separate from RGB/YCbCr and leave grayscale open. All
profiles completed materialization with exact Pillow parity. Per-run rows are
in the local ledger and the raw report is ignored under
`build/performance-optimization/`. No coverage was run.

### Packed CMYK grayscale GPU kernel

For complete four-pixel CMYK groups, the grayscale shader now loads four
packed pixels and writes the four luma bytes as one output word. Keep the
256-thread workgroup and the checked tail path for incomplete groups. Four
1024×768 call-plus-materialization runs passed exact live-Pillow parity on all
three profiles, with actual CPU/SIMD/GPU execution and no fallback. Their
median-of-run-medians were Pillow/CPU/SIMD/GPU 600.16/306.98/217.21/338.85 µs.
The GPU receipt reported one dispatch, 3,145,728 uploaded bytes, 786,432 bytes
read back, and one full-frame copy. Packing improved GPU latency about 19% over
the four-run mapped-input cohort, but GPU remained 1.56× slower than SIMD;
SIMD was 2.76× Pillow and CPU was 1.96× Pillow by the median ratio. Keep the
kernel as partial progress and leave grayscale open.

The strict GPU parity cases for varied CMYK at 17×13 (including a partial
four-pixel group) and 4096×4096 both passed. A 512-thread workgroup trial was
rejected by GPU pipeline validation, so both shader and dispatch planner remain
at 256; the current adapter rejected the 512-thread pipeline during validation,
not during a Pillow parity comparison. This was not a test defect. A delta-luma
formula rewrite passed parity but measured 419.96–430.17 µs across three runs,
showing no gain over the mapped-input shader, and was reverted.

Mapping CMYK input in the explicit GPU batch stream did not improve its
completed-window result. In 64-image 1024×768 windows, the mapped-stream run
measured 730.0 queued GPU images/s against 3,193.2 SIMD; the unmapped control
measured 991.9 against 2,587.6. Both reports passed 64 exact output-byte
comparisons and recorded 320 actual GPU jobs/dispatches, no fallback,
1,006,632,960 upload bytes, and 251,658,240 readback bytes. CPU and SIMD
controls shifted substantially between these cohorts, so they do not support
a causal speedup claim. Keep the eager mapped-input path separate and retain
the queue upload for explicit batching. Both runs included graph construction,
submission, execution, materialization, and byte export. The 13 single-call
runs add 52 rows to the local ledger (1,096 data rows); raw reports and batch
receipts are under ignored `build/performance-optimization/`. No new test
defect was found and no coverage was run.

### Rejected SIMD CMYK output-capacity path

A SIMD-only candidate changed the CMYK grayscale destination from a zeroed
vector to reserved capacity, then appended each completed vector block and the
checked tail. This removed the initial zero fill but used `extend_from_slice`
for the final stores. Three release runs passed exact live-Pillow parity with
actual CPU/SIMD/GPU execution and no fallback. Candidate medians in
Pillow/CPU/SIMD/GPU microseconds were 677.625/331.479/229.459/351.666,
635.437/317.875/224.666/341.500, and 642.980/317.750/224.917/345.854. SIMD
was 2.83×–2.95× Pillow, below the 2.92× fresh control ratio and no better
than the prior packed-kernel cohort after controls shifted. Revert the
allocation rewrite; reduced zero-fill work did not improve the measured
Pillow-relative SIMD result. The fresh control and three candidate reports
remain under ignored `build/performance-optimization/`; they add 16 profile
rows to the local ledger (1,112 data rows). No new test defect was found and
no coverage was run.

### CMYK grayscale CPU byte-width MULDIV255

In `cmyk_to_grayscale`, retain the direct output collection and narrow each
byte-by-byte Pillow `MULDIV255` calculation to `u16`. For byte operands,
`t = a*b + 128` is at most 65,153 and `t + (t >> 8)` is at most 65,407, so
the original rounded identity is exact without `u32` arithmetic. The
grayscale coefficient sum is 65,536, which allows the reconstructed CMYK
channel luma to be expressed as one rounded weighted ink subtraction. An
exhaustive 65,536-pair helper test matched the shared `u32` formula.

Two 1024×768 release control runs followed by two candidate runs measured the
complete grayscale call and materialization, with 100 latency samples per
subject. Pillow/CPU medians were 673.250/299.250 and 723.854/336.229 µs in the
controls (CPU 2.250× and 2.153× Pillow), then 725.312/201.875 and
778.666/213.917 µs with the candidate (3.593× and 3.640×). Exact parity passed
3/3 selected comparisons in all four runs; receipts showed actual CPU, SIMD,
and GPU execution with no fallback. The candidate CPU medians were 32.5% and
36.4% below their corresponding controls.

The independent SIMD results were 3.106× and 3.246× Pillow. GPU latency was
365.084 and 390.625 µs, only 1.987× and 1.993× Pillow and slower than SIMD
(GPU/SIMD latency ratios 0.640 and 0.614). Each GPU call recorded one dispatch,
3,145,728 uploaded bytes, and 786,432 readback bytes. Sustained GPU throughput
was not measured. Keep the serial CPU candidate; leave the grayscale API
issue open because SIMD, GPU-vs-SIMD, other modes/sizes, the hosted x86 CPU
row, Parallel CPU, and throughput targets remain. This local dirty-worktree
result does not alter the canonical full-runner matrix. The four reports and
parity sidecars are under ignored `build/migration-parity/`; their 16 profile
rows bring `docs/evidence/performance-optimization-local-runs.csv` to 1,738
data rows. No coverage was run.

### Grayscale completed-work queue throughput

An initial tuning batch was accidentally run with system Python 3.9.6 and
Pillow 11.3.0, while this project pins the comparison distribution to Pillow
12.2.0. Its output bytes and backend receipts are useful diagnostics, but its
timing rows are `inconclusive_diagnostic` and excluded from comparisons and
optimization ranking. The initial benchmark preflight also treated the
replacement's `PIL.__version__` (the pillow-rs package version) as the oracle
version. That was the wrong identity to compare. The runner now records both
the installed Pillow distribution and the imported `PIL` facade; it requires
the Pillow distribution to match and checks that the oracle module reports
that distribution's version.

The matched 2026-10-10 run used CPython 3.12.13 with Pillow distribution
12.2.0. The replacement facade reports pillow-rs 12.2.1, as expected. It
processed 64 fresh grayscale graphs per window, with two warmups and five
measured windows. Timing includes graph construction, submission, execution,
materialization, and byte export. All 64 outputs matched byte-for-byte across
Pillow, serial CPU, SIMD, eager GPU, and queued GPU for each mode. Backend
receipts confirmed the requested CPU/SIMD/GPU path, zero fallback, terminal
completion, and 448 completed operations per workload.

| Mode, 1024×1024 | Pillow images/s | CPU images/s | SIMD images/s | Queued GPU images/s | GPU/SIMD throughput |
| --- | ---: | ---: | ---: | ---: | ---: |
| L | 4,997.9 | 6,621.6 | 7,338.4 | 1,329.3 | 0.181× |
| RGB | 1,273.2 | 3,409.7 | 3,701.0 | 1,028.1 | 0.278× |
| CMYK | 886.2 | 2,342.7 | 2,226.8 | 786.7 | 0.353× |

For the explicit `max_in_flight` A/B at `max_jobs=64`, two four-flight
candidate runs averaged 1,233.0/971.1/726.0 queued GPU images/s for L/RGB/CMYK,
versus 1,150.1/811.0/706.6 with two flights (+7.2%/+19.7%/+2.7%). The
rebuilt-default run reached 1,329.3/1,028.1/786.7 images/s. Retain four as the
default: the matched repeats improved each mode. This does not prove GPU
kernel overlap or long-duration sustained throughput. Queued GPU remained
below SIMD for all three modes, so the latency/throughput target is still
unmet. The full reports and all per-subject rows are in ignored
`build/migration-parity/`; the local ledger separates ineligible Python 3.9
diagnostics from matched runs. No coverage was run.

A matched `max_in_flight=8` diagnostic at the same 64-job capacity measured
1,310.9/1,055.7/824.9 queued images/s for L/RGB/CMYK versus 1,329.3/1,028.1/
786.7 with four flights. The eight-flight queue made 67/69/70 submissions
instead of 35/49/49, with largest submissions of 8 instead of 16. Since it
regressed L and its RGB/CMYK gains were small in this single cohort, retain
four flights. Exact bytes still passed and GPU remained below SIMD at
0.179×/0.260×/0.391× throughput. The report is
`build/migration-parity/grayscale-throughput-py312-max64-flight8-20261010.json`.

A direct mapped-input upload trial reused the eager GPU grayscale upload path
for one-stage RGB/YCbCr/CMYK graphs. It preserved exact bytes, requested GPU
execution, and transfer volumes, but the queued RGB rate fell from 1,028.1 to
466.6 images/s. After reverting to `queue.write_buffer`, a matched control
returned to 1,016.4 images/s; L and CMYK also stayed near their prior rates.
The CPU-mapped buffer wait is a poor fit for this queued path, so keep the
existing upload implementation. See the candidate and post-revert reports in
`build/migration-parity/`; no candidate timing is counted as a retained gain.

### AArch64 RGB getprojection SIMD block screening

For each 48-byte RGB scan block, check the first `u8x16` vector and exit when
it already contains a nonzero byte. When that vector is zero, OR the other two
vectors and compare once. This keeps the dense-block early exit while
coalescing checks for the all-zero RGB case. Three 16×16 black-RGB
whole-workflow runs passed all three CPU/SIMD/GPU parity comparisons; each
receipt reported the requested actual backend and no fallback. Pillow/CPU/SIMD/
GPU medians in microseconds were 4.7295/4.500/4.292/273.792,
5.000/4.750/4.3125/274.667, and 5.000/4.583/4.625/276.208. The SIMD medians
were below the earlier single-run 4.667 µs control, but these are separate
run groups, so they do not establish an interleaved A/B gain. The local SIMD
speedup was only about 1.08–1.16× Pillow and GPU latency was about 60–64× the
SIMD latency. Keep this as an incremental local candidate, not a target met;
larger RGB inputs, remaining modes, the x86 sparse-CMYK SIMD regression, and
GPU sustained throughput remain unresolved. Full runs and receipts are in the
local ledger and ignored `build/performance-optimization/` artifacts. No
coverage was run.

### AArch64 dense RGB getprojection column coverage

For direct RGB projection, keep a count of horizontal columns already found
nonzero. While some columns remain uncovered, inspect each 48-byte RGB block
and update the projection exactly as before. Once all columns are covered, the
horizontal result is complete; for each remaining row, only find one nonzero
block or scalar-tail pixel to establish its vertical bit. Preserve the scalar
tail and report only vectors actually scanned. The large solid-RGB workload
and dense/sparse tail workloads all passed their live-Pillow gates (three
selected, executed, and passed comparisons each).

Three whole-workflow 1024×768 solid-RGB baseline runs measured Pillow/CPU/SIMD/
GPU medians of 680.646/526.270/317.542/637.604 µs,
696.667/561.562/333.479/478.084 µs, and
647.688/507.916/324.563/669.791 µs. Three candidate runs measured
642.166/506.895/94.125/563.479 µs,
644.917/507.105/94.416/456.167 µs, and
653.333/507.667/94.250/479.334 µs. SIMD was 6.82–6.93× faster than Pillow in
the candidate runs, and the median of the three per-run SIMD medians improved
about 3.44× against the separate baseline cohort. Serial CPU was 1.27× Pillow,
so it meets the CPU ≤ Pillow requirement here but remains below the 2×
checkpoint. GPU executed with actual GPU receipts and no fallback, but its
latency was 4.83–5.99× slower than SIMD; sustained GPU throughput remains
unmeasured.

The 17×4 dense-tail SIMD medians were 5.500 µs for both candidate and control;
the sparse-tail medians were 12.792 µs candidate and 13.084 µs control. For the
15×3 dense scalar-tail case they were 5.000 and 5.375 µs. These are small,
separate run groups and do not establish an interleaved speedup; they show no
material tail regression. GPU requests on 15×3 RGB fell back to CPU because
that image is unsupported for GPU getprojection, so exclude those rows from
GPU performance comparisons. Keep the candidate local and the operation open:
published x86_64 sparse-CMYK SIMD remains 0.258810× Pillow, other modes and
sizes remain unresolved, and GPU has not met its latency or throughput target.
All twelve benchmark reports and their parity receipts are in the local
ledger and ignored `build/performance-optimization/` artifacts. No genuine
test defect was found; no coverage was run.

### AArch64 serial CPU RGB getprojection row scan

The generic materialized-image scanner marks a flat pixel index by dividing
and taking its remainder by the image width. For RGB/HSV/YCbCr rows at least
64 pixels wide, scan packed rows instead. While horizontal columns remain
uncovered, mark nonzero pixels by their row-local column; after all columns are
covered, short-circuit each later row with a bytewise nonzero check. Keep the
existing pixel traversal for narrower rows: two alternate narrow paths passed
parity but did not establish a repeatable CPU win.

Three release whole-workflow runs of the final wide-row candidate measured
Pillow/CPU/SIMD/GPU medians (µs) of 646.375/94.208/118.229/456.479,
646.980/94.375/94.938/642.729, and
657.105/94.333/97.729/556.375 on the 1024×768 solid-RGB workload. Each run
passed all twelve selected profile comparisons across the solid and three
RGB-tail workloads. CPU was 6.85–6.97× faster than Pillow and about 5.38×
faster by the median-of-run-medians than the preceding 507.105 µs control.
SIMD was 5.47–6.81× Pillow in these runs. Requested and actual CPU, SIMD, and
GPU backends matched on the large image, with no fallback. GPU latency was
3.86–6.77× slower than SIMD; sustained throughput remains unmeasured.

The retained narrow path still trails Pillow on these very small RGB cases:
CPU medians were 5.417–5.500 µs versus Pillow at 5.166–5.167 µs for dense
17×4; 10.750–10.833 versus 10.083–10.166 µs for sparse 17×4; and
5.375–5.416 versus 5.125 µs for dense 15×3. An all-columns state machine on
these widths raised CPU medians to 5.46–5.71, 10.83–11.17, and 5.46–5.67 µs,
so it was disabled below 64 pixels. A simpler narrow row loop produced
5.04–5.46, 10.71–10.83, and 5.38–5.50 µs; the sparse and scalar-tail cases
remained slower than Pillow and the dense result was inconsistent, so that
variant was also reverted. Keep both rejected cohorts in the local ledger.
GPU requests on 15×3 fell back to CPU and are excluded; GPU did execute on
17×4, but its latency is far above SIMD. The published x86_64 sparse-CMYK SIMD
regression remains unresolved, as do other modes and sizes. Keep getprojection
open. No genuine test defect was found and no coverage was run.

### Rejected sparse-CMYK fixed-width SIMD loads

An explicit four-slice load and OR chain for each 64-byte CMYK SIMD block did
not improve the retained chunked vector loop. Three release whole-workflow
runs, each with 100 samples after five warmups and a 3/3 exact-parity gate,
measured candidate Pillow/SIMD medians (µs) of 484.646/196.854,
549.730/241.646, and 528.521/228.834. A fresh three-run restored-control
cohort after reverting the experiment measured Pillow/SIMD at 477.708/212.604,
930.792/204.958, and 431.666/176.125 µs. The candidate and control ranges
overlap, and separate cohorts do not establish a paired causal delta. The
candidate did not demonstrate a repeatable gain, so revert the explicit loads
and retain the chunked vector loop and receipt counting. Receipts showed actual
SIMD, three fused operations, terminal completion, and no fallback. GPU also
ran without fallback but remained slower than SIMD, and sustained throughput
was not measured. Keep getprojection open, especially for the published
x86_64 sparse-CMYK SIMD gap. No genuine test defect was found and no coverage
was run. The 24 profile rows and parity evidence for candidate and control
cohorts are recorded in
`docs/evidence/performance-optimization-local-runs.csv`.

### Rejected tiny-RGB getprojection axis masks

A bounded `u64` horizontal/vertical bit-mask candidate for RGB-family images
with width below 64 and height at most 64 passed three exact-parity gates
(9/9 profile comparisons each). Across three release whole-workflow runs,
CPU/Pillow medians (µs) were 5.542/5.333, 11.042/10.334, and 5.542/5.250;
5.708/5.375, 11.416/10.417, and 5.750/5.292; then 5.875/5.666,
11.666/11.208, and 5.708/5.292 for dense 17×4, sparse 17×4, and dense
15×3. CPU remained slower than Pillow in every sample group and the candidate
did not beat the retained narrow path in median-of-run-medians. Revert the
mask specialization. The 15×3 GPU request actually ran CPU and is excluded;
the 17×4 GPU runs were real but much slower than SIMD. Keep getprojection
open. No genuine test defect was found and no coverage was run. All 36
candidate profile rows are retained in
`docs/evidence/performance-optimization-local-runs.csv`.

### Retained exact-tuple RGB Image.new conversion

The tiny RGB whole-workflow misses were concentrated in setup: the RGB
projection call phase for dense tuple-color inputs was already slightly faster
than Pillow. The PyO3 `Image.new` binding tried seven independent color
conversions for an ordinary three-item tuple. Add a narrow fast path for an
exact built-in three-byte tuple in RGB mode and keep the general conversion
path for every other type, mode, and tuple shape. Four direct `Image.new`
parity cases passed, and each of three tail-workload runs passed all 9/9
selected profile comparisons. On AArch64, CPU/Pillow medians (µs) for dense
17×4 were 4.875/5.667, 6.083/6.708, and 4.958/6.084; dense 15×3 measured
4.834/6.229, 5.438/5.458, and 5.000/5.666. The setup-phase reduction made the
complete CPU workflows no slower than Pillow in all six tuple-color runs.
SIMD/GPU goals also remain open: GPU ran on 17×4 without fallback but was far
slower than SIMD, while the 15×3 GPU request fell back to CPU and is excluded.
Sustained GPU throughput is unmeasured. Keep this input conversion fast path,
but do not count the getprojection operation complete. No genuine test defect
was found and no coverage was run. The 36 profile rows are in
`docs/evidence/performance-optimization-local-runs.csv`; the direct
`Image.new` parity gate is `migration-parity-08df3ceebc4c43cd986aef2330753f92`.

### Rejected exact-integer RGB Image.new conversion

An extension for exact built-in Python integers was tested for RGB `Image.new`
to reduce setup in the sparse 17×4 workflow, whose initial fill color is
integer zero. It passed four direct `Image.new` parity cases
(`migration-parity-d93dd861552e44a9a7b8d459224fa515`) and all three
whole-workflow gates passed 9/9 selected comparisons. The AArch64 sparse
17×4 CPU/Pillow medians (µs) were 13.000/13.125, 11.292/10.667, and
10.750/10.770: two runs were no slower, but one was about 5.9% slower, so the
candidate did not show a repeatable end-to-end gain. Revert the integer branch
and retain only the exact-tuple fast path. The dense tuple workloads did not
exercise the integer branch. The three 36-row run groups, including
actual-backend and fallback receipts, are in
`docs/evidence/performance-optimization-local-runs.csv`. No genuine test defect
was found and no coverage was run.

### Rejected narrow RGB row-local CPU scan

A CPU candidate replaced the narrow RGB pixel iterator and per-pixel axis
division with a packed-row scan. Three fresh control runs and three candidate
runs each passed all 9/9 selected parity comparisons for dense 17×4, sparse
17×4, and dense scalar-tail 15×3 RGB workflows. The median-of-run-medians for
CPU latency (µs), control versus candidate, were 4.833/5.042, 11.292/11.375,
and 4.917/5.000. The candidate did not improve any of the three workloads, so
the row scan was reverted. These tiny end-to-end workflows remain noisy and
include setup and result materialization. GPU executed on 17×4; the 15×3 GPU
request fell back to CPU and is excluded from GPU comparisons. The six run
groups (72 profile rows) are in
`docs/evidence/performance-optimization-local-runs.csv`. No genuine test defect
was found and no coverage was run.

### RGB getprojection terminal backend-attribution repair

The sparse RGB pipeline exposed a benchmark attribution defect. Ordinary RGB
images use native `Rgb8` storage without an explicit mode tag, while terminal
fusion admitted only pipelines tagged explicitly as `RGB` or `CMYK`. A queued
GPU or SIMD `PutPixel` prefix therefore ran on its requested backend, but the
final `getprojection` scan ran on CPU. The prefix receipt remained in
thread-local telemetry and made the whole call look accelerated. Exact output
parity did not catch this backend-path error.

Infer ordinary RGB from the loaded source mode and admit the native RGB
terminal path. The GPU terminal applies RGB overrides to the three stored
channels (ignoring the normalized alpha byte), preserves ordered last-write
semantics, and handles a zero-filled source without uploading the frame. The
GPU shader receipt now reports one terminal dispatch for the sparse 17×4 RGB
case instead of the earlier two prefix dispatches. Historical SIMD/GPU rows
for sparse 17×4 in the local ledger are reclassified as mixed CPU-terminal /
accelerated-prefix execution, and their accelerated speedup fields are blank.
Their recorded prefix transfers remain in the ledger as raw whole-workflow
resources, with notes that they do not prove terminal execution.

`cargo test getprojection_rgb_pipeline_tests --lib` passed 3/3 RGB CPU, SIMD,
and GPU terminal tests, including alpha-ignored writes; the six existing CMYK
terminal tests also passed. `make build-parity` succeeded. A release
whole-workflow benchmark then passed 9/9 live-Pillow comparisons across dense
17×4 RGB, sparse 17×4 RGB, and dense 15×3 RGB scalar-tail workloads
(`migration-parity-benchmark-gate-adc26353e0f04bb1b7685e0b01176fb7`). The
sparse result reported requested/actual GPU, no fallback, one dispatch, zero
source-upload bytes, 84 readback bytes, 16 override bytes, and no full-frame
copy. Its one-run medians were Pillow/CPU/SIMD/GPU 10.250/11.250/11.375/321.292
µs: CPU still trailed Pillow, SIMD did not beat Pillow, and GPU latency was
about 28× the SIMD latency. GPU sustained throughput remains unmeasured. The
15×3 GPU request actually used CPU and is excluded from GPU comparisons. These
measurements verify terminal attribution; they do not accept an optimization
candidate or close getprojection. The benchmark run is
`migration-benchmark-fa618e6c146f4107aef18803ed00cdcc`; all 12 subject rows are
in the local ledger and the JSON reports are under ignored
`build/performance-optimization/`. The local ledger now has 894 data rows. No
coverage was run.

### Rejected tiny sparse RGB zero-source pre-scan

After fixing terminal routing, a CPU-only candidate tried a bounded all-zero
pre-scan for native RGB sources up to 4 KiB. For a zero source with sparse
`PutPixel` writes, the final projection could be formed directly from the
last-write-per-coordinate map. Three fresh control and three candidate
whole-workflow runs each passed their CPU live-Pillow parity check (1/1 per
run). CPU medians in microseconds were 10.750, 10.834, and 10.854 for control
versus 11.042, 11.584, and 10.980 for candidate. The median-of-run-medians
rose from 10.834 to 11.042 µs, about 1.9% slower, and the candidate was slower
in all three CPU runs. Pillow timing varied between cohorts and does not
support a paired causal estimate. Revert the pre-scan; keep the zero-source
overwrite case as a parity regression test. The sparse composed CPU workload
remains slower than Pillow in the control median and the operation remains
open. The 12 profile rows are in the local ledger, now 906 data rows; reports
are under ignored `build/performance-optimization/`. No coverage was run.

### Retained exact RGB `putpixel` tuple conversion

The sparse RGB fixture constructs each `putpixel` color from an ordinary
three-item tuple. In `putpixel_mode`, after the existing mode, coordinate, and
bounds checks, an exact built-in tuple of three exact built-in integers in
`0..=255` can now call the typed RGB core method directly, skipping the
temporary component `Vec`. Lists, tuple subclasses, integer subclasses and
`bool`, wrong tuple lengths, and out-of-range values retain the generic
conversion path so its validation and error behavior remain authoritative.

Three AArch64 CPU-only whole-workflow runs passed exact live-Pillow parity.
Their CPU medians were 10.583, 10.292, and 10.584 µs, against fresh control
medians of 10.750, 10.834, and 10.854 µs. The median-of-run-medians fell about
2.3%, and each candidate CPU median was below every control CPU median;
however, CPU remained slower than Pillow in all three candidate runs, with
Pillow/CPU ratios of 0.957, 0.984, and 0.984. Keep the narrowly gated binding
fast path, but do not count the getprojection item as complete.

A subsequent full-profile cohort passed 9/9 parity comparisons across dense
17×4, sparse 17×4, and dense scalar-tail 15×3 RGB. For sparse 17×4, its single
cohort medians were Pillow/CPU/SIMD/GPU 13.125/11.042/10.917/199.333 µs. The
GPU receipt confirmed requested and actual GPU, no fallback, one terminal
dispatch, zero source-frame upload, 84 readback bytes, a 16-byte override
payload, 32 parameter bytes, and no full-frame copy. GPU latency was about
18.3× SIMD latency. This single full-profile cohort is corroborating evidence
only; it does not override the three CPU-only comparisons or establish the
latency target. On 15×3 the GPU request ran on CPU and is excluded from GPU
comparisons. Candidate and full-profile reports are under ignored
`build/performance-optimization/`; all 18 profile rows are in the local
ledger, now 924 data rows. No genuine test defect was found and no coverage
was run.

### Constructor-proven zero source and packed CMYK GPU projection

For the 1024×768 CMYK pipeline (`Image.new` with zero fill, three queued
`PutPixel` calls, then `getprojection`), track the zero-fill invariant only on
constructor-created RGB/CMYK storage. Any eager in-place mutation clears it;
queued writes remain deferred on the immutable source. This lets serial CPU
projection derive the result from the sparse writes without rescanning the
frame. SIMD continues to scan with its actual vector implementation so its
backend receipt still proves vector work.

Three matched whole-workflow control runs and three candidate runs each passed
all 3 live-Pillow parity cases, with CPU, SIMD, and GPU requested/actual backend
receipts and no fallback. Control median-of-run-medians were Pillow/CPU/SIMD/GPU
489.104/256.749/215.104/438.396 µs. The constructor-proven candidate measured
478.375/130.437/164.604/424.542 µs; paired Pillow speedups were 3.268× CPU and
2.590× SIMD. GPU remained 2.460× slower than SIMD in paired latency and had
0.406× the repeated-call SIMD throughput. It uploaded zero source bytes, read
back 7,168 bytes, used one actual GPU dispatch, and recorded no full-frame copy.

A zero-source-only GPU candidate packs the horizontal and vertical results into
bitsets before readback and allocates its GPU working buffers to that compact
result. It preserves the public `Vec<u32>` output by expanding both axes after
readback. The output transfer fell from 7,168 to 224 bytes and retained GPU
cache from 6,297,872 to 6,864 bytes; upload remained zero and the full-frame
copy count remained zero. The three packed candidate runs passed 3/3 parity
cases each and reported actual GPU with no fallback. Paired CPU and SIMD
speedups over Pillow remained above 2× in this workload, but packed GPU
latency was still about 1.97× SIMD latency and repeated-call GPU throughput
about 0.51× SIMD. The 32× readback reduction did not produce a reliable
end-to-end latency gain. A separate ablation removing the pre-map health poll
did not establish a repeatable gain, so restore the existing poll.

`cargo test getprojection_cmyk_tests --lib` passed 8/8, including eager
mutation invalidation, sparse-write parity, and the 72-byte packed GPU
readback assertion. `make build-parity PYTHON=.venv/bin/python` succeeded.
The six Python candidate reports and their parity receipts are under ignored
`build/performance-optimization/`; their 48 profile rows bring the local run
ledger to 972 rows. No new test defect was found, sustained GPU throughput was
not measured, and no coverage was run. Keep getprojection open: this is one
CMYK size and pipeline shape, and GPU latency/throughput targets remain unmet.

An additional 4096×4096 size-scaled probe reused the same sparse CMYK pipeline
and the same 5 warmups, 20 iterations, 5 samples, concurrency-1 policy. Its
temporary fixture used three writes at the origin, an interior point, and the
last pixel; the tracked 1024×768 input files were restored after the probe.
Each of three whole-workflow runs passed all three Pillow parity comparisons
with actual CPU, SIMD, and GPU execution and no fallback. Median-of-run-medians
were Pillow/CPU/SIMD/GPU 9,273.604/3,081.208/4,111.875/3,416.708 µs. Paired
Pillow speedups were 3.320× CPU and 2.458× SIMD. GPU latency was below SIMD in
all three paired runs (GPU/SIMD ratios 0.931×, 0.780×, 0.833×), and measured
repeat throughput was higher in all three (ratios 1.074×, 1.282×, 1.200×;
median 1.200×). GPU used one dispatch, uploaded 0 bytes, read back 1,024 bytes,
and recorded no full-frame copy. The 4K result shows that the compact sparse
GPU path can beat SIMD once the scan is large enough, but it does not erase the
1024×768 GPU regression or prove sustained GPU throughput. Keep the operation
open for other modes, sizes, dense inputs, and sustained-throughput evidence.
The three reports and parity receipts are under ignored
`build/performance-optimization/`; their 12 profile rows bring the local
ledger to 984 data rows. No coverage was run.

The same size-scaled sparse workflow was probed in RGB. Three whole-workflow
runs each passed all three Pillow parity comparisons with actual CPU, SIMD,
and GPU execution and no fallback. Median-of-run-medians were Pillow/CPU/SIMD/
GPU 15,139.063/2,804.459/3,650.458/3,176.812 µs; paired Pillow speedups were
5.548× CPU and 4.271× SIMD. GPU latency beat SIMD in all three runs, with a
median GPU/SIMD ratio of 0.835×, and measured repeated throughput was 1.198×
SIMD. Its receipt showed one dispatch, zero upload, 1,024 bytes read back, no
full-frame copy, and 8,464 bytes of retained GPU cache. As in CMYK, this is
100 warm concurrency-one calls rather than long-duration sustained-throughput
evidence. The temporary RGB fixture was restored after measurement; its
reports and parity receipts remain under ignored
`build/performance-optimization/`. The 12 profile rows bring the local ledger
to 996 data rows. No coverage was run.

### Sparse 1024×768 GPU projection binding reuse

A zero-source GPU branch reuses its projection bind group with the pooled
working set. The cached group binds the compact output buffer and the dummy
source word, and is rebuilt if the parameter arena grows. A fresh packed-path
control cohort passed three live-Pillow cases per run and measured median
Pillow/CPU/SIMD/GPU latencies of 488.063/132.854/164.416/407.917 µs. Two
bind-group-cache cohorts also passed all parity comparisons with actual GPU
execution and no fallback. Their combined six-run median-of-run-medians were
485.010/166.375/205.313/374.875 µs; the first cohort's GPU median was
364.792 µs, while the second was 442.959 µs. This is partial evidence for a
lower GPU median across the pooled runs, not a repeatable win in each cohort.
GPU/SIMD latency ratios across the six candidate runs ranged from 1.766× to
2.146×, and repeated-call throughput ranged from 0.466× to 0.566× SIMD.
Sustained GPU throughput is unmeasured, so the latency and throughput targets
remain unmet.

A separate zero-clear experiment replaced the packed-result `clear_buffer`
command with shader stores and a workgroup barrier. It measured 408.229 µs GPU
latency versus 407.917 µs in the fresh packed control, so it showed no gain and
was rejected; the buffer clear remains. `cargo test getprojection_cmyk_tests --lib`
passed 8/8. `make build-parity PYTHON=.venv/bin/python` succeeded.
The twelve fresh-control, twelve zero-clear-ablation, and twenty-four cache
candidate rows bring the local ledger to 1,044 data rows. No new test defect
was found and no coverage was run. Keep getprojection open.

### RGB material thumbnail: SIMD eligibility and remaining GPU gap

The hosted matrix still ranks RGB material thumbnail as a high-return SIMD gap: 8/9 published
single-thread SIMD workloads are slower than Pillow, while no published serial-CPU workload is
slower. This local experiment is supplemental evidence on Apple Silicon; it does not replace the
hosted rows.

The measured public workflow constructs a black 1024×768 RGB image, writes `(2, 3) = (180, 120,
60)`, calls `thumbnail((256, 256), resample=2)`, and materializes `tobytes()` inside the
`whole_workflow` boundary. Pillow resample code `2` is BILINEAR. The image uses native
`ImageRgb8` storage while its internal logical mode tag is `None`. The former shortcut required
an explicit `Some("RGB")` tag and the Bicubic filter, so it did not cover this public route. Two
early candidates changed the sparse reducer but recorded no end-to-end gain because the route
was still ineligible. The execution profile exposed the missed branch; eligibility was then tied
to native RGB storage and `None | Some("RGB")`, and extended to the measured non-nearest filter.

The retained candidate uses an exact sparse 2×2 RGB reducer when the large even image has at
most 1/1024 nonzero pixels, then skips source rows proven all-zero in the horizontal and
vertical resampling passes. Dense images, partial reductions, other layouts, and unproved cases
retain the existing generic paths. Three exact-parity 100-sample repeats measured
median-of-run-medians of Pillow / serial CPU / single-thread SIMD / GPU at **1,049.333 / 779.563
/ 440.792 / 2,227.625 µs**. On this one workload CPU is 1.346× Pillow and SIMD is 2.381× Pillow;
the user-requested 2× SIMD tier is met locally, but the repository’s 5× SIMD bar is not. The
hosted operation remains open because this does not cover its other modes, sizes, runners, and
workloads.

The GPU ran for real with no fallback, but took 5.054× the SIMD latency. Each call used three
dispatches, uploaded 3,145,728 bytes for the 2,359,296-byte RGB source, read back 196,608 bytes,
and recorded one mode conversion and one full-frame copy. The report’s 100 sequential-call
throughput is not sustained-throughput evidence; the GPU target remains unmet.

A separate GPU admission ablation used compact RGB input for the aligned `Reduce(2×2) + Resize`
thumbnail prefix. Three live-Pillow parity-gated runs recorded actual GPU execution and no
fallback. It reduced upload from 3,145,728 to 2,359,296 bytes and removed the source mode
conversion, while keeping three dispatches. Median-of-run-medians GPU latency was 2,241.708 µs
versus 2,227.625 µs before the change; repeated-call throughput was 446 versus 449 operations/s.
The GPU pipeline phase increased by 96 µs even as the terminal phase fell. The byte reduction
did not improve end-to-end latency or throughput, so the admission change was reverted. A future
GPU candidate must shorten the dispatch or synchronization critical path without changing the
observable byte-rounding boundaries.

The live parity sweep passed 177/177 thumbnail cases on CPU and 5/5 cases each in the fixture’s
strict SIMD and GPU sets. Three retained SIMD candidate repeats passed the live-Pillow parity
gate with requested and actual CPU, SIMD, and GPU execution and no fallbacks. The three GPU
input-ablation repeats also passed parity with actual GPU execution and no fallback. The
Pillow-SIMD GitHub JSON snapshot remains dirty and partial (9/34 workloads) and contains no
thumbnail workload, so no Pillow-SIMD ratio is valid here. The local evidence ledger now has
1,160 rows, including three controls, six SIMD candidates, and three rejected GPU input trials.
No genuine test defect was found, and no coverage was run.

### Sparse CMYK getprojection SIMD from constructor-proven zero input

The 1024×768 sparse-CMYK pipeline constructs a zero-filled image, queues three
positive `PutPixel` operations, then calls `getprojection`. The SIMD terminal
path now uses that immutable zero-fill provenance when there are at most eight
non-palette positive writes. It builds each four-element projection-axis block
with `u32x4` comparisons, so the actual SIMD work produces the returned axes
without scanning the 3 MiB source. Zero overwrites, palette-index writes, and
larger write batches keep the existing overwrite-aware SIMD scan.

Four live-Pillow whole-workflow repeats passed all three selected CPU/SIMD/GPU
parity comparisons per run. Median-of-run-medians were Pillow / CPU / SIMD / GPU
**407.031 / 130.177 / 130.470 / 414.084 µs**. The prior four-run sparse SIMD
source-scan cohort was 409.011 / 129.886 / 164.271 / 407.907 µs, respectively. SIMD
latency fell 20.6% while CPU stayed about the same; this local run is 3.12× Pillow
for SIMD and 3.13× for CPU. It is local macOS arm64 evidence, not a refresh of
the published Ubuntu x86 SIMD ratio (0.258810× Pillow) or Ubuntu arm64 ratio
(0.263122×).

The terminal receipts reported requested and actual SIMD, three fused writes,
complete output, and no fallback. The SIMD zero-source route made no full-frame
copy or device transfer; it allocated two output arrays totaling 7,168 bytes.
The GPU also ran without fallback, in one dispatch with zero upload, 224 bytes
read back, and no full-frame copy, but its latency was 3.17× SIMD. Four
concurrency-one run groups of repeated calls do not establish sustained GPU
throughput. The GPU target remains open.

A fixed-width unrolled four-vector scan candidate was also parity-gated in three
runs and then rejected: its median SIMD latency was 166.750 µs, versus 164.271
µs in the prior scan controls, with one candidate run at 201.084 µs. Directly
spelling out the loads did not improve the end-to-end workload, so the generic
64-byte block scan remains for cases that do not qualify for the zero-source
route.

`cargo test -p pillow-rs getprojection_cmyk_tests --lib` passed 10/10, including
the positive-write SIMD route, axis tails, and the over-limit scan fallback.
The four full workflow reports and parity receipts for the retained candidate,
plus the three rejected scan reports, are under ignored
`build/performance-optimization/`; they add 28 rows to the local ledger, now
1,188 data rows. The published matrix is unchanged. No test defect was found
and no coverage was run. Keep getprojection open: x86 and other runner results,
other modes and sizes, Pillow-SIMD comparisons, and GPU latency and sustained
throughput remain unresolved.

The live GitHub Pages full benchmark asset was re-fetched for this checkpoint;
it remains clean at revision `ff0009b71755d6f1cebae63c96b96d39aa183797`, measured
2026-10-09, with 8,619 rows across 663 workloads. The separately served
Pillow-SIMD asset remains dirty at revision `101fdb8cc2c9da7ea98da8602ac4e7879ab23c9d`,
with 36 rows for 9/34 workloads and no getprojection row, so it cannot provide
a valid Pillow-SIMD comparison for this operation.

### Rejected packed wordwise GPU projection output

For the 1024×768 sparse-CMYK pipeline, a bounded shader candidate assigned one
lane to each of the 56 packed output words, ORed the sparse writes into a local
word, and stored that word once. It skipped the output clear and avoided GPU
atomics when the packed result contained at most 256 words. Three whole-workflow
runs passed all three live-Pillow comparisons per run with actual CPU, SIMD,
and GPU execution and no fallback. Median-of-run-medians were Pillow / CPU /
SIMD / GPU **529.937 / 180.438 / 177.250 / 379.917 µs**. GPU latency was
2.09–2.19× SIMD latency, and the repeated-call GPU throughput was 0.456–0.478×
SIMD. Each GPU receipt showed one dispatch, zero upload, 224-byte readback,
zero full-frame copies, 24 bytes of overrides, and two host result allocations
totaling 7,168 bytes.

Three matched control runs restored the existing clear-plus-atomic shader path
and passed all three live-Pillow GPU comparisons per run. Their GPU medians were
512.958, 387.979, and 397.813 µs. The two lower controls had a median of
392.896 µs, only 3.3% above the candidate; the first control reached 512.958 µs.
This variation does not support a reliable end-to-end gain, so the wordwise
shader change was reverted. The remaining small-workload GPU gap is consistent
with submission, mapping, and materialization cost dominating the tiny sparse
kernel; that is an inference from these end-to-end timings, not a separately
measured phase attribution. The repeated-call throughput samples are not
long-duration sustained-throughput evidence.

`cargo test -p pillow-rs getprojection_cmyk_tests --lib` passed 10/10 after the
revert. Candidate and control reports plus parity receipts are under ignored
`build/performance-optimization/`; 18 rows bring the local evidence ledger to
1,206 data rows. The published matrix is unchanged. No test defect was found
and no coverage was run. Keep getprojection open: GPU latency and throughput,
published x86 SIMD, other modes and sizes, and Pillow-SIMD comparisons remain
unresolved.

### Rejected sparse-axis SIMD zero-fill and mark

The sparse-CMYK SIMD terminal builds each four-element axis vector by comparing
its lanes with the deferred positive-write coordinates. A candidate instead
filled output vectors with SIMD zero lanes, then set the few marked coordinates
directly. Three whole-workflow runs passed all three live-Pillow comparisons per
run with actual CPU, SIMD, and GPU execution and no fallback. Median-of-run-
medians were Pillow / CPU / SIMD / GPU **481.063 / 161.438 / 157.396 / 379.042
µs**; candidate SIMD medians were 174.417, 157.396, and 154.708 µs.

The preceding full-profile cohort measured Pillow / CPU / SIMD at 529.937 /
180.438 / 177.250 µs. Pillow and serial CPU also moved down with the candidate,
while SIMD speedup over Pillow changed only from 2.99× to 3.06×. Candidate p95
was 201.458 µs versus 190.334 µs in the earlier cohort. These samples do not
isolate a repeatable SIMD gain from run-to-run variation, so the candidate was
reverted. It did not reach the repository's 5× SIMD bar. The GPU remained
slower than SIMD; those candidate runs used the restored clear-plus-atomic GPU
path and are separate profile evidence.

`cargo test -p pillow-rs getprojection_cmyk_tests --lib` passed 10/10 with the
restored implementation. Three candidate reports and parity receipts are under
ignored `build/performance-optimization/`; 12 rows bring the local evidence
ledger to 1,218 data rows. The canonical published matrix is unchanged. No
genuine test defect was found and no coverage was run. Keep getprojection open
for hosted x86 SIMD evidence, other modes and sizes, Pillow-SIMD comparisons,
and the GPU latency and throughput targets.

### Rejected spin-first GPU projection readback

The projection path maps only a 224-byte result after its single GPU dispatch.
A bounded candidate busy-polled the device up to 16 times before using the
existing 50 µs retry backoff. Three full-workflow runs passed all three
live-Pillow comparisons per run with actual CPU, SIMD, and GPU execution and no
fallback. GPU medians were 447.000, 368.104, and 481.604 µs, versus 385.792,
379.042, and 370.563 µs in the preceding normal-poll cohort. Median-of-run-
medians rose from 379.042 to 447.000 µs. Candidate GPU/SIMD latency ratios
were 2.30–2.65×; repeated-call throughput was 0.378–0.435× SIMD, with the
GPU's median repeated-call rate falling from 2,638 to 2,237 operations/s.

The device executed one dispatch with zero upload, 224 bytes of readback, no
full-frame copy, and no fallback in every candidate run. The additional polling
did not lower end-to-end latency or improve throughput, so it was reverted; the
existing bounded poll/backoff remains. These short sequential samples do not
measure long-duration sustained GPU throughput.

`cargo test -p pillow-rs getprojection_cmyk_tests --lib` passed 10/10 after the
revert. Three candidate reports and parity receipts are under ignored
`build/performance-optimization/`; 12 rows bring the local evidence ledger to
1,230 data rows. The published matrix is unchanged. No genuine test defect was
found and no coverage was run. Keep getprojection open for GPU latency and
throughput, hosted x86 SIMD, other modes and sizes, and Pillow-SIMD comparisons.

### Rejected F-mode SIMD sample-view candidate

The 1024×768 native F bicubic resize workload decodes the little-endian input
into a temporary `Vec<f32>` in the SIMD adapter even though the shared checked
sample-view helper can borrow aligned native storage. A bounded candidate used
that helper and retained its exact decoder for unaligned storage. Each local
run used the full public resize call and output materialization, five warmups,
20 iterations across five samples, and concurrency one. All three runs passed
the 3/3 selected live-Pillow comparisons. Terminal-complete proof receipts
matched the requested CPU, SIMD, and GPU backends with no fallback.

| Run | Pillow ms | CPU ms | SIMD ms | GPU ms | Pillow/SIMD |
| --- | ---: | ---: | ---: | ---: | ---: |
| Same-worktree baseline | 3.398 | 1.648 | 6.794 | 63.583 | 0.500× |
| Borrowed-view candidate | 2.767 | 1.340 | 5.585 | 53.417 | 0.495× |
| Candidate repeat | 2.751 | 1.326 | 5.464 | 52.352 | 0.503× |

Pillow and all target profiles shifted together; the paired SIMD/Pillow ratio
did not improve. The benchmark's checked pixel-buffer counters also stayed at
3,932,160 bytes for CPU and SIMD. Those counters do not capture every Rust
temporary allocation, so they do not measure whether the sample `Vec` was
removed. The candidate was reverted because it provided no end-to-end SIMD
gain. GPU executed two dispatches with 3,145,728 upload bytes, 786,432 readback
bytes, and one full-frame copy; its latency remained about 9.4–9.6× SIMD. The
concurrency-one repeated-call rate is not sustained-throughput evidence. These
local arm64 rows do not update the published cross-runner matrix. No genuine
test defect was found and no coverage was run.

### F-mode SIMD lane-plan resize trial

The SIMD `PIL.Image.Image.resize` path now builds reusable coefficient/source
lane plans for native F-mode Bicubic reductions at exactly 2:1 scale. The
horizontal plan preserves Pillow's ordered f64 FMA and f32 intermediate; the
vertical plan reuses checked source-row offsets and f64 weight vectors across
each output row's width blocks. Other scales and filters keep the general
kernel. The change is local to the SIMD implementation and does not enable
Rayon.

An early same-worktree cohort shows the lane-plan progression against the
borrowed-view baseline. Each benchmark passed its selected exact Pillow gate;
all four backend receipts were terminal-complete and matched the requested
backend with no fallback.

| Run | Pillow ms | CPU ms | SIMD ms | GPU ms | Pillow/SIMD speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| Borrowed-view baseline | 3.398 | 1.648 | 6.794 | 63.583 | 0.500× |
| Horizontal lane plan | 3.752 | 1.629 | 1.947 | 59.965 | 1.927× |
| Horizontal lane plan repeat | 2.909 | 1.333 | 1.960 | 53.550 | 1.484× |
| Horizontal + vertical plan | 2.776 | 1.324 | 1.902 | 53.324 | 1.459× |
| Horizontal + vertical plan repeat | 2.669 | 1.325 | 1.874 | 52.681 | 1.424× |

The final two local runs used the same 1024×768 F-mode Bicubic workload, the
public resize call plus output materialization, five warmups, 20 iterations in
each of five samples, warm cache, and concurrency one. Both selected benchmark
parity gates passed. A separate SIMD run over the F-mode corpus passed all
138/138 selected cases, with zero failures, infrastructure errors, or cases
not run.

| Run | Pillow ms | CPU ms | SIMD ms | GPU ms | Pillow/CPU speedup | Pillow/SIMD speedup | SIMD/GPU latency |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Lane plan, final run 1 | 3.525 | 1.697 | 2.373 | 63.978 | 2.077× | 1.485× | 0.037× |
| Lane plan, final run 2 | 3.420 | 1.644 | 2.546 | 63.799 | 2.081× | 1.343× | 0.040× |

Terminal-complete receipts identified actual CPU, SIMD, and GPU execution with
no fallback. GPU used two dispatches, uploaded 3,145,728 bytes, read back
786,432 bytes, and made one full-frame copy. This concurrency-one result does
not measure sustained GPU throughput. The local CPU row cleared 2× Pillow in
both runs, but SIMD remained below 2× Pillow and GPU latency remained about
25–27× slower than SIMD. The published cross-runner matrix is unchanged; for
`Image.resize`, its weakest verified CPU ratio is 1.314680× and the weakest
verified SIMD ratio is 0.337965× Pillow.

The direct shared-weight horizontal splat variant passed its selected parity
gate but regressed: two runs measured Pillow/SIMD speedup at 1.206× and 1.194×,
versus 1.485× and 1.343× for the restored lane plan in the later cohort. It was
reverted. A safe byte-slice copy also passed selected parity but showed no
repeatable whole-call gain and was reverted. An earlier attempt to transfer a
`Vec<f32>` allocation directly into `Vec<u8>` stopped at the parity preflight
with bytemuck's `AlignmentMismatch`; this was an invalid candidate conversion,
not a test defect. Keep the owned byte-carrier allocation unless the image
representation itself changes safely.

The current [Pillow-SIMD benchmark JSON](https://appunni-m.github.io/pillow-rs/assets/pillow-simd-benchmark.json)
is dirty (revision `101fdb8cc2c9da7ea98da8602ac4e7879ab23c9d`) and its 36 rows
cover nine workloads for BoxBlur, GaussianBlur, and getchannel. It contains no
`Image.resize` workload, so it provides no valid Pillow-SIMD resize comparison.
Keep resize in the Pillow-priority phase until its published applicable rows
meet the 2× CPU target; then add or use a clean, matched Pillow-SIMD resize row.
The [full benchmark JSON](https://appunni-m.github.io/pillow-rs/assets/benchmark.json)
remains clean at revision `ff0009b71755d6f1cebae63c96b96d39aa183797`.

At the lane-plan checkpoint, the local results ledger had 1,278 data rows and
recorded each measured backend row for that candidate and its rejected
alternatives. No genuine test defect was found and no coverage was run. Keep
`PIL.Image.Image.resize` open at priority 1.

### Native F serial CPU accumulation trial

The serial CPU F-mode vertical pass now visits each source tap across the
output row, using one reusable f64 accumulator per output column. Each column
still receives Pillow's ordered FMA sequence and is rounded to f32 at the same
pass boundary, including signed-zero results. The change does not alter SIMD,
Parallel CPU, or GPU execution.

The combined CPU parity lane passed all 177 selected F/I cases (138 F cases and
39 applicable I cases). Three final all-backend runs used the full public
1024×768 F-mode Bicubic resize plus materialization, five warmups, 20 timed
iterations in five samples, and concurrency one. Every run passed the benchmark
Pillow gate and produced terminal-complete CPU, SIMD, and GPU receipts with no
fallback.

| Run | Pillow ms | CPU ms | CPU speedup vs Pillow | SIMD ms | SIMD speedup vs Pillow | GPU ms | SIMD/GPU latency ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Retained all-backend 1 | 2.752 | 1.018 | 2.703× | 1.904 | 1.445× | 52.509 | 0.036× |
| Retained all-backend 2 | 2.751 | 1.023 | 2.690× | 1.899 | 1.449× | 52.166 | 0.036× |
| Retained all-backend 3 | 2.946 | 1.022 | 2.883× | 1.909 | 1.543× | 52.029 | 0.037× |

The GPU used two dispatches, uploaded 3,145,728 bytes, read back 786,432
bytes, and made one full-frame copy. This concurrency-one benchmark does not
measure sustained GPU throughput. A same-worktree CPU-only control measured
Pillow/CPU at 2.863/1.343 ms (2.132×); the final local CPU speedups versus
Pillow were 2.690–2.883×. These are local macOS arm64 results and do not refresh
the published cross-runner matrix, whose weakest verified CPU row remains 1.314680×.

Native I remains below the 2× CPU target. Three original-loop controls measured
CPU/Pillow at 1.608–1.627×. A vertical tap-major I candidate passed parity but
did not improve normalized speedup (1.574–1.596×), so it was reverted. A
four-output interleaved vertical candidate also passed its 39-case I parity
lane and regressed to 1.252–1.314×. A horizontal tap-major candidate passed
the same parity lane but regressed to 0.878–0.894×. Keep the established I
loops until an exact, whole-workflow candidate improves against its matched
Pillow timing.

A bounded macOS Time Profiler run of 3,500 I resize calls collected 2,330
main-thread samples, including 697 in `resize_i`; two `Vec::from_elem`
zero-initialization stacks contributed 27 and 22 samples. The profile also
included adapter and module-loading work, so those counts are diagnostic and do
not attribute a precise share of resize latency. They do not justify an unsafe
uninitialized-buffer change. No genuine test defect was found. One parity
launch used the system Python with Pillow 11.3.0 instead of the required 12.2.0;
running the same corpus through `.venv/bin/python` passed 177/177 and confirms
this was an environment mismatch, not a corpus defect. No coverage was run.

Keep `PIL.Image.Image.resize` open at priority 1: native I is below the 2× CPU
gate, SIMD is 1.445–1.543× Pillow in these runs, GPU latency is 27× slower than
SIMD, and sustained GPU throughput is unmeasured. The dirty partial
Pillow-SIMD snapshot still contains no matched resize workload.

The local results ledger has 1,422 data rows, including these final all-backend
F cohorts, I-mode controls, rejected I candidates, and the matched register-block
trial below.

### Rejected I-mode four-column vertical register block

The serial I-mode vertical pass was changed temporarily to keep four adjacent
output accumulators live and visit all eight shared source-row weights for each
column in the original FMA order. The selected 1024×768 Bicubic resize plus
materialization workload passed exact Pillow parity for Pillow, CPU, SIMD, and
GPU. Receipts recorded each requested backend as the actual backend with no
fallback.

The matched original-loop control measured 1.229 ms CPU against 1.974 ms Pillow
(1.607×). The four-column candidate measured 1.451 ms CPU against 1.979 ms
Pillow (1.364×), an 18.1% CPU latency regression. Both cohorts had five
warmups, 20 iterations across five samples, warm cache, and concurrency one.
CPU/SIMD observed two host buffers totaling 3,932,160 bytes. GPU observed two
dispatches, 3,145,728 bytes uploaded, and 786,432 bytes read back; concurrency
one does not establish sustained GPU throughput. The candidate was reverted.
No genuine test defect was found and no coverage was run.

### Rejected I-mode explicit FMA register block

A follow-up explicitly unrolled the eight FMA steps across four adjacent
vertical output columns, keeping the four independent accumulators in registers.
The 1024×768 Bicubic resize plus materialization passed exact Pillow parity in
the CPU, SIMD, and GPU profiles; each executed the requested backend without
fallback. Its Pillow/CPU/SIMD/GPU medians were 1.993/1.518/4.514/36.795 ms.
Against the matched original-loop control at 1.974/1.229/4.511/37.069 ms, CPU
latency regressed 23.6% and speedup fell from 1.607× to 1.313×. Revert the
explicit unroll and choose a different mechanism for the next I-mode trial.
GPU again used two dispatches, 3,145,728 bytes uploaded, and 786,432 bytes read
back at concurrency one, so sustained throughput remains unmeasured. No
genuine test defect was found and no coverage was run.

### Rejected I-mode horizontal four-column FMA block

The next serial CPU trial explicitly interleaved the eight FMA chains for four
adjacent horizontal outputs while retaining each output's coefficient set and
rounding point. The same full public resize and materialization benchmark
passed exact parity in the CPU, SIMD, and GPU profiles with no fallback. Its
Pillow/CPU/SIMD/GPU medians were 1.968/1.564/4.537/36.559 ms. The matched
original-loop control measured 1.974/1.229/4.511/37.069 ms, so CPU latency
regressed 27.3% and speedup fell from 1.607× to 1.259×. Revert this traversal
and avoid more four-column register blocks. GPU used two dispatches, 3,145,728
bytes uploaded, and 786,432 bytes read back at concurrency one; sustained
throughput remains unmeasured. No genuine test defect was found and no coverage
was run.

### Retained I-mode vertical streaming ring

The serial CPU I-mode resizer now computes each horizontal INT32 row as the
vertical pass needs it and retains recent rows in a power-of-two ring. Each
horizontal row keeps Pillow's ordered f64 FMA accumulation and INT32 rounding;
each vertical result uses the same ordered coefficients and rounding. The
Parallel CPU implementation remains on its separate two-pass path. For the
1024×768 to 512×384 Bicubic workload, the ring holds 16×512 i32 values (32,768
bytes), replacing the 1,572,864-byte full horizontal intermediate.

Two full public resize-plus-materialization cohorts passed exact selected
Pillow parity with 100/100 terminal-complete actual CPU, SIMD, and GPU samples
and no fallback. Each used five warmups, 20 iterations across five samples,
warm cache, and concurrency one:

| Run | Pillow ms | CPU ms | CPU speedup vs Pillow | SIMD ms | SIMD speedup vs Pillow | GPU ms | SIMD/GPU latency ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Streaming ring 1 | 2.107 | 1.197 | 1.760× | 4.666 | 0.452× | 36.640 | 0.127× |
| Streaming ring 2 | 1.980 | 1.166 | 1.698× | 4.529 | 0.437× | 35.986 | 0.126× |

Against the matched original-loop control at 1.229 ms CPU and 1.974 ms Pillow,
the candidate CPU medians were 2.6% and 5.1% lower. The local CPU result remains
below the requested 2× Pillow sequencing gate, and the SIMD rows still miss
the half-Pillow goal. GPU latency remains about eight times SIMD; each request
uses two dispatches, 3,145,728 uploaded bytes, and 786,432 readback bytes.
Concurrency one does not establish sustained GPU throughput.

A strict CPU parity run selected 8,884 `PIL.Image.Image` cases and passed all
8,884 with zero failures, infrastructure errors, or skipped cases. That result
contains 847 `PIL.Image.Image.resize` cases; all 847 passed across their modes,
sizes, and edge inputs. The benchmark host-buffer telemetry remains two buffers
totalling 3,932,160 bytes, but it does not count every core-owned temporary such
as the ring or the previous full intermediate. Allocation and materialization
time remain inside the measured public boundary; do not treat the incomplete
buffer counter as a total-allocation measurement. The published cross-runner
matrix is unchanged; retain this as local macOS arm64 evidence and keep resize
open. No genuine test defect was found and no coverage was run.

### I-mode source-span, SIMD coefficient-plan, and GPU proof updates

For serial I-mode horizontal filtering, the retained path obtains the dense
eight-tap source row as one checked contiguous span. On the 1024×768 Bicubic
public resize-plus-materialization workload, this source measured 1.040 ms CPU
against 1.969 ms Pillow (1.893×); its same-source recontrol measured 1.048 ms
CPU against 2.011 ms Pillow (1.919×). The streaming ring remains 32,768 bytes
instead of a 1,572,864-byte full horizontal intermediate. A strict CPU surface
run passed 8,884/8,884 cases, including all 847 resize cases. These are local
macOS arm64 rows and remain below the 2× CPU sequencing target.

The retained single-thread SIMD path builds checked horizontal source indexes
and `f64x8` weights once per resize, then reuses that plan for each source row.
It keeps Pillow's ordered vector FMA sequence and the horizontal INT32 pass
boundary. Selected I-mode SIMD parity passed 5/5 supported resize cases; a
strict full-surface sweep's 741 failures were unsupported-capability outcomes,
not byte mismatches, so that sweep is not a full-surface parity pass. The
1024×768 SIMD medians were 2.057/2.204 ms (1.072× Pillow) in the candidate and
1.919/1.970 ms (1.026×) in the plan-only recontrol; a later same-source row was
1.946/1.988 ms (1.022×). SIMD remains far short of the half-Pillow target.

For GPU marker 11, the exact host admission proof previously scanned the same
tap row twice: once to find a common coefficient exponent, then again to build
the exact sum. It now aligns the accumulated integer sum as a lower exponent
appears and checks each tap against Pillow's ordered f64 FMA result in one pass.
The source carrier is decoded to typed I32 words once, and the proof passes the
horizontal word vector directly to its vertical pass instead of materializing
and rereading an intermediate byte vector. The exact proof still visits every
sample and rejects unproved rows. In this proof-only trial, the shader remained
two-pass; the later shader reduction is recorded separately below.

| Full-call run | Pillow ms | CPU ms | SIMD ms | GPU ms | GPU/SIMD latency ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Original-proof control | 1.970 | 1.045 | 1.919 | 36.554 | 0.052× |
| One-pass proof candidate | 1.988 | 1.057 | 1.946 | 31.392 | 0.062× |
| Same-source candidate recontrol | 1.968 | 1.044 | 1.927 | 30.825 | 0.063× |

The first paired cohort reduced GPU latency 14.1% while Pillow, CPU, and SIMD
medians moved by at most 1.4%; a final same-source parity run measured GPU at
30.784 ms. The selected parity gate passed 3/3 target comparisons, and GPU
telemetry confirmed the actual GPU backend, two dispatches, 3,145,728 uploaded
bytes, 786,432 readback bytes, and no fallback. A three-second host sample put
1,410 of 2,318 main-thread samples in the two exact proof passes; this is a
profiling diagnostic, not GPU throughput evidence. GPU remains about 16× slower
than SIMD, and sustained GPU throughput is unmeasured. Host-allocation
telemetry reports zero for this route but does not count every core-owned
`Vec`; those allocations remain inside the measured boundary.

A further predecoded coefficient-parts plan passed parity but did not improve
the normalized GPU/Pillow latency ratio: candidate and repeat stayed at about
15.7×, matching the controls. That plan was reverted. The strict SIMD
capability outcomes were not a test defect, and no genuine test defect was
found. No coverage was run. The published cross-runner matrix and its dirty,
partial Pillow-SIMD snapshot remain unchanged; resize stays open for other
modes, sizes, hosts, and the GPU latency and sustained-throughput targets.

### I-mode GPU marker-11 one-pass convolution

Both marker-11 convolution shaders now discover the common coefficient
exponent and accumulate the exact signed integer sum in one tap pass. When a
lower exponent arrives, the kernel rescales the accumulated magnitude before
adding that tap. The host admission proof checks the corresponding bounded
integer arithmetic and Pillow-order f64 FMA result before this marker is used.
The shader change removes the duplicate per-tap source and coefficient reads;
it does not change coefficient tables, transfers, dispatch count, or routing.

On the full 1024×768 I-mode Bicubic resize plus materialization workload, the
matched two-pass control measured 30.692 ms GPU latency. Two one-pass runs
measured 30.359 and 30.435 ms (1.1% and 0.8% lower medians). The other subject
medians were 1.965/1.045/1.923 ms Pillow/CPU/SIMD in the control,
1.983/1.064/1.911 ms in the first candidate, and 1.971/1.044/1.939 ms in the
second. Treat this as a small GPU improvement: the selected full-call gates
passed 3/3, and five strict GPU I-mode resize cases passed 5/5, including
zero-width geometry and fractional Box gap cases. The benchmark receipts
confirmed two actual GPU dispatches, no fallback, 3,145,728 uploaded bytes,
and 786,432 readback bytes in each run. GPU latency remains about 16× SIMD
latency; concurrency-one measurements do not demonstrate sustained
throughput. I-mode CPU remains below 2× Pillow and SIMD remains below 2× Pillow.

The local results ledger has 1,462 data rows, including the paired kernel
control and two one-pass measurements. These local macOS arm64 observations do
not change the published matrix. No genuine test defect was found and no
coverage was run.

### Rejected I-mode vertical ring row-base caching

The serial CPU trial precomputed the eight masked ring row bases once per
vertical output row, aiming to remove repeated mask and row-stride arithmetic
from every output pixel. Its selected full-call parity gate passed 3/3 with the
requested backends. The candidate Pillow/CPU/SIMD/GPU medians were
1.966/1.053/1.930/30.651 ms; the post-revert matched control measured
1.972/1.047/1.918/30.679 ms. Candidate CPU latency was 0.6% worse, moving from
1.884× to 1.868× Pillow, with no repeatable whole-call gain. Revert the row
base specialization and retain the original ordered vertical accumulator.
The GPU receipt showed actual GPU execution, two dispatches, no fallback,
3,145,728 uploaded bytes, and 786,432 readback bytes; sustained throughput is
unmeasured. Resize remains open. No genuine test defect was found and no
coverage was run. The local results ledger has 1,462 rows; published results
remain unchanged.

### Retained SIMD I-mode block-slice stores

The SIMD I-mode horizontal intermediate and final output stores now resolve a
single checked slice for each eight-pixel block, then write rounded samples
through that slice. This removes repeated per-lane output offset and bounds
checks without changing coefficient order, ordered f64 FMA, INT32 intermediate
rounding, or little-endian serialization.

The parity-gated 1024×768-to-512×384 Bicubic call-plus-materialization
benchmark passed 3/3 CPU/SIMD/GPU comparisons on every run. Five strict SIMD
I-mode resize cases also passed, including zero-width input geometry and
fractional Box gap cases. Three block-store runs recorded Pillow-normalized
SIMD ratios of 1.064×, 1.073×, and 1.063×; the fresh original-store control was
1.033×. That is a repeatable but modest 2.9–4.0% SIMD gain over Pillow after
normalizing cohort shifts. The local CPU ratio remained about 1.90× Pillow,
below the 2× sequencing gate. GPU latency was 30.692 ms against SIMD at
1.858 ms on the final candidate run, and the GPU receipt confirmed two real
dispatches, 3,145,728 uploaded bytes, 786,432 readback bytes, and no fallback.
Concurrency-one throughput does not demonstrate sustained GPU throughput.

Retain the store change as local progress, but leave resize open: SIMD remains
far short of the user’s 2× Pillow target and the repository’s 5× target; CPU is
below 2× on this host; GPU latency is about 16.5× slower than SIMD; hosted
cross-runner coverage and sustained GPU throughput remain unproven. The host
allocation counters do not account for every core-owned `Vec`, although its
allocations and materialization remain inside the measured boundary. The local
ledger has 1,462 rows and contains the three candidate receipts, the fresh
recontrol, and parity sidecars. The benchmark target’s 13 workload-selection
tests passed. No genuine test defect was found and no coverage was run.

### Rejected SIMD I-mode horizontal chain interleaving

The SIMD experiment interleaved two independent eight-output horizontal FMA
chains at each tap index while preserving each output's ordered coefficient
sequence. The selected strict SIMD I-mode cohort passed 5/5, and both complete
call-plus-materialization benchmark runs passed exact CPU/SIMD/GPU parity 3/3
with the requested backend actually executing and no fallback.

The first candidate cohort shifted across all subjects and measured
Pillow/CPU/SIMD/GPU at 2.265/1.130/2.037/34.297 ms. Its 1.112× Pillow-to-SIMD
ratio was not a reliable improvement because the Pillow timing itself was
about 15% above the retained control. The repeat aligned with the original
store control: candidate medians were 1.974/1.046/2.078/32.908 ms, while the
post-revert single-chain control measured 1.965/1.043/1.848/30.512 ms. At this
matched Pillow level, SIMD regressed from 1.063× Pillow to 0.950× and became
slower than Pillow. The paired chain experiment was reverted; the independent
block-slice store change remains.

The GPU subject used two dispatches, 3,145,728 uploaded bytes, and 786,432
readback bytes, with no fallback. Its call latency remained far above SIMD and
the concurrency-one workload did not prove sustained throughput. The 13
benchmark workload-selection checks passed; no genuine test defect was found
and no coverage was run. The local ledger now has 1,462 rows. These local
macOS arm64 findings do not update the published matrix, and I-mode resize
remains open against CPU, SIMD, and GPU targets.

### Rejected CPU I-mode typed output stores

The serial CPU trial used `bytemuck::try_cast_slice_mut` to write rounded I-mode
samples through an aligned `i32` view of the checked output allocation, keeping
the original byte writer as the alignment fallback. It did not change sample
rounding or FMA order. Five strict selected CPU I-mode resize cases passed, and
the two full call-plus-materialization benchmark runs passed CPU/SIMD/GPU
parity 3/3 with actual requested backends and no fallback.

Candidate Pillow/CPU/SIMD/GPU medians were 1.964/1.037/1.826/30.090 ms and
1.963/1.035/1.849/30.323 ms; the post-revert control measured
1.974/1.040/1.840/30.460 ms. The candidate's absolute CPU latency was lower,
but its Pillow-normalized CPU ratios (1.893× and 1.896×) did not exceed the
reverted control's 1.898×. Revert the typed writer because it showed no
repeatable normalized gain. CPU remains below the 2× sequencing goal; GPU
latency and sustained throughput goals remain unmet. The local ledger has 1,462
rows. No genuine test defect was found and no coverage was run.

### Rejected I-mode repeated-coefficient sharing

For the 1024×768-to-512×384 Bicubic I-mode call plus materialization, the
horizontal interior has bit-identical eight-tap coefficient rows. A guarded
candidate reused one row for those outputs while keeping the clipped edges,
sample order, f64 FMA sequence, INT32 pass boundary, and final rounding intact.
The coefficient proof test passed, and each full benchmark run passed exact
CPU/SIMD/GPU parity 3/3 with the requested backend executing and no fallback.
All cohorts used five warmups, 20 iterations across five samples, 100 timed
samples per subject, and concurrency one.

| Cohort | Pillow ms | CPU ms | Pillow/CPU speedup | SIMD ms | GPU ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Before candidate | 2.262 | 1.185 | 1.909× | 2.127 | 35.327 |
| Candidate 1 | 2.326 | 1.117 | 2.083× | 2.195 | 32.240 |
| Candidate 2 | 2.153 | 1.038 | 2.073× | 2.049 | 34.850 |
| Post-revert control | 2.337 | 1.172 | 1.994× | 2.121 | 34.965 |
| Narrow-gated candidate recheck | 2.029 | 1.022 | 1.985× | 1.892 | 32.244 |

The first two candidate ratios suggested a roughly 4% normalized gain, but the
narrowly gated repeat did not separate from the post-revert control. Absolute
Pillow, CPU, SIMD, and GPU times moved between cohorts, so the candidate was
reverted rather than treated as a speedup. GPU calls used two dispatches, a
3,145,728-byte upload, and a 786,432-byte readback; these concurrency-one
latencies do not establish sustained throughput. The local ledger now has
1,482 rows. The published matrix is unchanged, resize remains open, no genuine
test defect was found, and no coverage was run.

### Rejected SIMD I-mode row-local float conversion

A bounded live profile of the 1024×768 I-mode Bicubic SIMD call sampled
`simd_resize_i32` in 687 of 1,579 main-thread samples. That identified the
resize kernel as hot, but did not isolate sample conversion. A candidate
converted each source row from i32 to a reusable f64 scratch once, then kept
the existing ordered f64x8 FMA and rounded INT32 intermediate. All selected
call-plus-materialization benchmarks passed exact parity 3/3 with actual
CPU/SIMD/GPU execution and no fallback.

| Cohort | Pillow ms | SIMD ms | Pillow/SIMD speedup | GPU ms |
| --- | ---: | ---: | ---: | ---: |
| Initial SIMD control | 2.045 | 1.923 | 1.064× | 32.656 |
| Candidate 1 | 2.025 | 1.889 | 1.072× | 35.207 |
| Candidate 2 | 2.292 | 1.954 | 1.173× | 34.773 |
| Rebuilt post-revert control | 2.496 | 2.336 | 1.068× | 34.868 |

Candidate 1's normalized result was effectively unchanged from control. In
candidate 2 the Pillow median rose 13% while SIMD latency also rose, so its
larger ratio did not establish a kernel gain; the rebuilt post-revert control
returned to the initial ratio. Revert the extra row buffer and conversion pass.
The first control's CPU measurement came from a binding that still contained a
previous reverted CPU-only candidate, so exclude that CPU cell from CPU
comparisons; its SIMD path was unchanged and the requested SIMD backend
executed. GPU used two dispatches, 3,145,728 upload bytes, and 786,432 readback
bytes. Concurrency-one throughput does not prove sustained GPU throughput. The
local ledger now has 1,498 rows. The hosted matrix is unchanged, resize remains
open, no test defect was found, and no coverage was run.

### Rejected F-mode SIMD zero-initialization append trial

The F-mode SIMD resize fully overwrites its output and intermediate vectors
for non-empty convolution inputs. A candidate replaced zero-filled vectors
with exact-capacity allocations and row-major `extend_from_slice` writes to
avoid initializing those buffers. Each cohort used the 1024×768 Bicubic
full-call-plus-materialization boundary, five warmups, 20 iterations across
five samples (100 timed samples per subject), warm cache, concurrency one, and
the live Pillow parity gate. All four cohorts passed 3/3 selected comparisons;
CPU, SIMD, and GPU receipts were terminal-complete, matched their requested
backends, and had no fallback.

| Cohort | Pillow ms | CPU ms | SIMD ms | GPU ms | Pillow/SIMD speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| Pre-candidate control | 3.473 | 1.337 | 2.423 | 63.688 | 1.433× |
| Candidate 1 | 2.812 | 1.036 | 2.017 | 53.250 | 1.394× |
| Candidate 2 | 2.881 | 1.068 | 2.229 | 53.954 | 1.292× |
| Rebuilt post-revert control | 3.021 | 1.097 | 2.086 | 53.710 | 1.449× |

The candidates did not improve the normalized SIMD ratio: both were below the
pre-candidate and post-revert controls. Revert the append path. The retained
SIMD lane plan remains about 1.29–1.45× Pillow on this local F-mode workload,
below the 2× sequencing goal. GPU latency is about 24–26× SIMD; the GPU used
two dispatches, 3,145,728 uploaded bytes, 786,432 readback bytes, and one
full-frame copy. These concurrency-one repeated-call rates do not establish
sustained throughput. Host-allocation counters do not expose the internal Rust
`Vec` initialization costs that motivated the candidate, so full-call latency
is the evidence for rejecting it. The dirty, partial Pillow-SIMD JSON still
has no matched resize workload. The local ledger now has 1,530 rows; the
published matrix is unchanged. Resize remains open. No genuine test defect was
found and no coverage was run.

### Retained F-mode thumbnail full-box SIMD route

For native F-mode thumbnails, Pillow first reduces the 1024×768 source by 2
and then resizes 512×384 to 256×192 with Bicubic. The scaled reducing-gap box
ends exactly at the reduced image bounds for this divisible geometry. The SIMD
adapter now uses the existing unboxed F resize kernel in that case, reusing its
2:1 coefficient lane plan. It keeps the boxed kernel when ceil-sized partial
edges leave the Pillow box short of the reduced image dimensions.

The benchmark measured the public mutation and `tobytes()` observation together
with five warmups, 20 iterations across five samples (100 timed samples per
subject), warm cache, and concurrency one. Each candidate full-call benchmark
passed its selected exact Pillow gate 3/3; the third candidate also passed
strict SIMD parity for the large F case, the specialized F path, and the odd
17×13 reducing-gap edge case (3/3).

| Cohort | Pillow ms | CPU ms | SIMD ms | Pillow/SIMD speedup |
| --- | ---: | ---: | ---: | ---: |
| Pre-candidate control | 1.031 | 0.336 | 0.821 | 1.256× |
| Candidate 1 | 1.324 | 0.393 | 0.845 | 1.567× |
| Candidate 2 | 1.233 | 0.429 | 0.881 | 1.400× |
| Reverted control | 1.226 | 0.435 | 0.959 | 1.279× |
| Candidate 3 | 1.103 | 0.346 | 0.727 | 1.517× |

All candidate ratios exceeded both controls, although each cohort moved in
absolute latency. Candidate 2's Pillow and CPU medians were within 1% and 1.4%
of the reverted control while SIMD latency was 8% lower. Retain the exact
full-box route as a local SIMD improvement. Its measured SIMD speedups remain
1.400–1.567× Pillow, short of the 2× target. CPU was faster than Pillow in
these runs. The requested GPU profile fell back to CPU on every run because
GPU does not prove the thumbnail reducing-gap or typed arithmetic semantics;
exclude those rows from GPU performance claims. The local ledger now has 1,550
rows. The hosted matrix is unchanged; thumbnail remains open for the other
modes, shapes, runners, and GPU target. No genuine test defect was found and
no coverage was run.

### Native RGB `Image.merge` local controls

The clean hosted matrix's worst SIMD merge comparison is the 1024×768 RGB
three-L-band workload on x86_64 at 0.464× Pillow. Two local ARM64 controls of
that workload used the full public merge call and result observation, five
warmups, 20 iterations across five samples (100 observations per subject),
warm cache, concurrency one, and a passing selected parity gate (3/3). The
current CPU and SIMD routes were the actual requested backends in both runs.

| Cohort | Pillow ms | CPU ms | SIMD ms | CPU/Pillow | SIMD/Pillow | GPU ms | SIMD/GPU |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Control | 1.115 | 0.319 | 0.193 | 3.490× | 5.773× | 1.747 | 0.111× |
| Repeat | 1.152 | 0.329 | 0.354 | 3.503× | 3.250× | 1.411 | 0.251× |

The SIMD medians moved substantially between cohorts, so treat the local range
as ARM64 control evidence, not as a candidate gain or a resolution of the x86
regression. GPU executed one dispatch without fallback, but each call uploaded
2,359,296 bytes, read back 2,359,296 bytes, and materialized one 2,359,296-byte
host output. Its isolated-call latency remained 4.0–9.1× slower than SIMD, and
these single-operation concurrency-one runs did not measure sustained
throughput. Keep `PIL.Image.merge` open until the x86 SIMD gap and GPU latency
and throughput targets are addressed. Full receipts are in
`docs/evidence/performance-optimization-local-runs.csv`; the hosted matrix is
unchanged. No genuine test defect was found and no coverage was run.

### Native `L` GaussianBlur weighted SIMD row kernel — 2026-10-10

The published matrix ranks the material 1024×768 `L` GaussianBlur SIMD lane
below Pillow on macOS ARM, Ubuntu ARM, and Ubuntu x86. On the local macOS ARM
host, the call-plus-result-materialization control measured Pillow / serial
CPU / SIMD / GPU at 3.815 / 2.384 / 5.324 / 1.463 ms. A row-local generic
three-pass horizontal candidate preserved exact parity but left SIMD at
5.298 ms, so fusing rows alone was not useful.

The retained candidate computes each fractional radius-one horizontal pass
with 16 adjacent `L` pixels per vector. It keeps the far-edge fractional taps,
Pillow's fixed-point weights, and byte rounding at each of the three pass
boundaries, then processes one row at a time before the vertical passes. The
first draft reused the integer BoxBlur(1) average and omitted GaussianBlur's
fractional edge weights. The parity gate caught this SIMD-only candidate bug;
it returned no timing data. This was not a fixture or test defect.

| Cohort | Pillow ms | CPU ms | Pillow/CPU | SIMD ms | Pillow/SIMD | GPU ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Weighted-vector SIMD candidate | 3.834 | 2.339 | 1.639× | 2.342 | 1.636× | 0.948 |
| SIMD repeat | 3.837 | 2.390 | 1.605× | 2.349 | 1.634× | 0.951 |
| Restored-source confirmation | 3.938 | 2.435 | 1.617× | 2.356 | 1.672× | 0.890 |
| CPU direct-vertical candidate | 3.833 | 2.327 | 1.647× | 2.337 | 1.641× | 0.938 |
| CPU direct-vertical repeat | 3.808 | 2.292 | 1.661× | 2.354 | 1.618× | 0.954 |

Each candidate run passed all three exact CPU/SIMD/GPU parity comparisons,
completed 100 timed samples on the requested backend, and recorded no
fallback. The full public call and materialization were timed. SIMD's host
allocation receipt increased from one allocation of 786,432 bytes to four
allocations totaling 1,574,912 bytes; these counters are not a process-wide
allocation trace. GPU executed six dispatches and transferred 786,432 bytes in
each direction. GPU latency beat SIMD in these concurrency-one runs, but
sustained throughput was not measured. The candidate improves this local SIMD
workload by about 56% over its control, but reaches only 1.63–1.67× Pillow, so
the 2× target remains unmet.

A separate row-major direct vertical five-tap SIMD kernel also passed parity,
but its cohort shifted all subject medians substantially and SIMD was slower
than the preceding candidate. It was reverted and is excluded from gain
claims. The CPU lane has a distinct scalar direct vertical L pass, checked
against the existing fixed-point column implementation for radius 1 and 1.375
over 1×1, 1×7, 2×3, 17×9, and 65×47 inputs. The focused test and both full
candidate parity gates passed. CPU speedup moved from 1.617× in the immediate
control to 1.647× and 1.661× in two candidate cohorts; this is a modest local
improvement, not a confirmed 2× result. Serial CPU and SIMD both remain below
2× Pillow. The GaussianBlur Kata item remains open: this evidence covers one
mode, size, radius, and host only. The published matrix remains tied to its
existing clean snapshot. The local evidence ledger now has 1,654 rows. No
genuine test defect was found and no coverage was run.

Result/parity pairs use the `gaussian-luma-row-fusion-*` names under
`build/migration-parity/`.

### Rejected L GaussianBlur vertical row-ring candidate — 2026-10-10

A candidate streamed the three horizontal and three vertical L GaussianBlur
passes through five-row rings to avoid full-frame intermediate writes. The
focused unit test matched the existing separate-pass fixed-point reference for
integer and fractional weights on one-pixel, narrow, odd, and material-shaped
inputs. The public 1024×768 parity-gated run also passed all three CPU/SIMD/GPU
comparisons. The candidate nevertheless regressed: on local macOS ARM, its
observed-call medians were Pillow 3.887 ms, CPU 2.600 ms, SIMD 2.410 ms, and GPU
1.484 ms. CPU fell to 1.495× Pillow and was 13.4% slower than the preceding
2.292 ms CPU cohort, so the row-ring implementation was removed. No test defect
was found.

The recorded full-phase medians (setup + pipeline + terminal/materialization)
were 5.312 / 3.950 / 3.824 / 2.880 ms for Pillow / CPU / SIMD / GPU. The GPU
receipt confirmed actual GPU execution with six dispatches, 786,432 bytes
uploaded and read back, and no fallback. Its median was below SIMD, but its
p95 was 3.583 ms versus SIMD at 2.541 ms; sustained throughput was not
measured. The complete four-profile cohort and both raw reports are in the
local ledger and `build/migration-parity/gaussian-luma-row-rings-20261010.json`
plus its parity companion. The local evidence ledger now has 1,654 rows. No
coverage was run.

### Rejected L GaussianBlur rolling vertical recurrence — 2026-10-10

A scalar vertical kernel reused Pillow's entering and leaving rows with a
rolling three-sample accumulator. Its focused edge test matched the existing
fixed-point column implementation, and the public live-Pillow gate passed on
CPU, SIMD, and GPU (3/3). On local macOS ARM, CPU measured 2.427 ms against
Pillow at 3.866 ms (1.594×); the preceding direct-vertical CPU cohort was
2.292 ms, so the recurrence was 5.9% slower and was removed. No test defect was
found. Full phase medians including setup and materialization were
5.281/3.790/3.777/2.323 ms for Pillow/CPU/SIMD/GPU. GPU was the actual backend
with six dispatches and 786,432 bytes transferred in each direction; sustained
throughput was not measured.

### Superseded L GaussianBlur scalar horizontal recurrence — 2026-10-10

The retained CPU candidate specializes the radius-one fractional L horizontal
pass. Its scalar rolling sum reuses the sample leaving the three-pixel window
for the fractional far-left tap. Widths below five continue through the
generic fixed-point helper. The focused test matches that reference for widths
1, 2, 3, 4, 5, 6, 7, 17, 65, and 1024. Two full public cohorts passed all three
CPU/SIMD/GPU parity comparisons:

| Cohort | Pillow ms | CPU ms | CPU/Pillow | SIMD ms | GPU ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Candidate | 3.960 | 2.108 | 1.879× | 2.410 | 1.001 |
| Repeat | 3.945 | 2.080 | 1.896× | 2.419 | 0.986 |

CPU improved 8–9% relative to the recent 2.292 ms direct-vertical cohort, but
still misses 2× Pillow. The corresponding full-phase medians (setup + pipeline
+ terminal/materialization) were 5.360/3.480/3.789/2.377 ms and
5.381/3.464/3.831/2.328 ms for Pillow/CPU/SIMD/GPU. Receipts show the requested
backend executed without fallback in each lane. GPU used six dispatches and
transferred 786,432 bytes each way; its median beat SIMD, but concurrent
throughput was not measured and p95 latency moved substantially between
cohorts. This was the in-progress CPU candidate at that checkpoint; the direct
five-tap formula below supersedes it. Keep GaussianBlur open. The complete
four-profile rows and raw reports are in the local
ledger and `build/migration-parity/gaussian-luma-horizontal-specialized-*`.
The ledger now has 1,666 rows. No test defect was found and no coverage was run.

### In-progress L GaussianBlur bounded-slice vertical rows — 2026-10-10

The scalar CPU vertical pass now binds each clipped source row and destination
row once, then walks the contiguous slices together. This removes per-pixel
row-index calculation from the fractional radius-one inner loop while keeping
the same five taps, replicated edges, fixed-point weights, and byte rounding.
The full observed call still includes `apply-filter` and
`observe-filter-result`, and both local 1024×768 cohorts passed exact Pillow
parity for all three target profiles (3/3 each):

| Cohort | Pillow ms | CPU ms | Pillow/CPU | SIMD ms | GPU ms | Pillow/GPU | GPU/SIMD |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Bounded slices A | 3.900 | 2.028 | 1.923× | 2.408 | 2.621 | 1.488× | 0.919× |
| Bounded slices B | 3.893 | 2.053 | 1.896× | 2.441 | 1.476 | 2.638× | 1.654× |

The CPU candidate improves about 1–2% over the preceding 2.080 ms local
cohort, but it remains below the 2× checkpoint. GPU latency varied: its median
was slower than SIMD in cohort A and faster in cohort B, while GPU p95 exceeded
SIMD p95 in both. Each run proved six actual GPU dispatches, 786,432 uploaded
and read-back bytes, and no fallback; concurrency-one results do not establish
sustained throughput. Full-phase medians (setup + pipeline + terminal) were
5.328/3.440/3.804/5.166 ms and 5.312/3.452/3.858/2.868 ms for Pillow/CPU/SIMD/GPU.
The local ledger now has 1,674 rows. Keep the item open across unmeasured
modes, sizes, Parallel CPU, runners, and GPU throughput. The clean published
matrix remains unchanged. Raw report/parity pairs use
`gaussian-luma-row-sliced-*` under `build/migration-parity/`. No test defect
was found and no coverage was run.

### Rejected L GaussianBlur horizontal slice-only loop — 2026-10-10

Pre-slicing the horizontal recurrence's interior inputs and output removed
per-pixel index calculations but did not demonstrate a CPU gain. Its focused
fixed-point line test and both full public parity gates passed 3/3 for
CPU/SIMD/GPU. The first benchmark ran while a separate
`uvicorn-rs-httpbench` process consumed 826% CPU; the repeat also overlapped
that benchmark. All subject medians shifted, and the repeat CPU result of
2.045 ms was within the preceding 2.028–2.053 ms range. Revert the slice-only
change and exclude both cohorts from optimization ranking. The exact reports
and parity receipts remain in the local ledger as rejected rows; no fallback
was counted and no test assertion was changed.

### In-progress L GaussianBlur independent five-tap CPU formula — 2026-10-10

The scalar CPU horizontal pass now computes the three near samples and two
fractional edge samples independently for each interior output. Explicit edge
formulas retain Pillow's replicated taps; lines narrower than five still use
the generic fixed-point helper. The focused line comparison passed exactly
against that helper for widths 1, 2, 3, 4, 5, 6, 7, 17, 65, and 1024. Two
parity-gated public call-plus-materialization cohorts passed all three CPU,
SIMD, and GPU comparisons (3/3 each):

| Cohort | Pillow ms | CPU ms | Pillow/CPU | SIMD ms | GPU ms | GPU/SIMD |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Direct taps A | 4.333 | 0.847 | 5.12× | 2.716 | 1.528 | 1.78× |
| Direct taps B | 4.417 | 0.755 | 5.85× | 2.791 | 1.537 | 1.82× |

The CPU lane now clears the local 2× target for this L workload, reaching
5.12–5.85× Pillow on macOS ARM. Each profile used its requested backend with
no fallback. CPU reported two full-frame host buffers totaling 1,572,864
bytes. GPU executed six dispatches and transferred 786,432 bytes each way;
its median beat SIMD, but GPU p95 exceeded SIMD p95 in both cohorts and
sustained throughput remains unmeasured. Full-phase medians including setup,
pipeline, and terminal/materialization were 5.735/2.410/4.237/3.002 ms and
5.887/2.177/4.370/3.005 ms for Pillow/CPU/SIMD/GPU. Keep GaussianBlur open:
the SIMD target, other modes and sizes, Parallel CPU, additional runners, and
GPU sustained throughput remain unproven. The clean published matrix is
unchanged. The local ledger now has 1,690 rows; reports and parity sidecars
use `gaussian-luma-direct-taps-*` under `build/migration-parity/`. No test
defect was found and no coverage was run.

### In-progress LA GaussianBlur independent five-tap CPU formula — 2026-10-10

The LA radius-one fractional horizontal pass now uses the same independent
three-near/two-far sample formula for its interleaved luminance and alpha
bytes. Explicit first/last-pixel formulas preserve replicated edges; every
interior output still rounds to a byte before the next Gaussian pass. The
focused generic-reference test passed at widths 5, 6, 7, 17, 65, and 1024.
Two current-source LA controls and two candidate cohorts passed exact Pillow
parity for CPU/SIMD/GPU (3/3 each):

| Cohort | Pillow ms | CPU ms | Pillow/CPU | SIMD ms | GPU ms | GPU/SIMD |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Control A | 5.258 | 3.645 | 1.442× | 4.086 | 1.304 | 3.13× |
| Control B | 5.266 | 3.635 | 1.449× | 4.092 | 1.465 | 2.79× |
| Direct taps A | 5.639 | 1.736 | 3.247× | 4.213 | 1.602 | 2.63× |
| Direct taps B | 5.701 | 1.727 | 3.300× | 4.199 | 1.694 | 2.48× |

The CPU change improves the matched local LA call from about 1.44× to
3.25–3.30× Pillow. The call-plus-materialization boundary is unchanged, and
the actual requested backends ran without fallback. CPU reported two
full-frame host buffers totaling 3,145,728 bytes. GPU used six dispatches and
transferred 1,572,864 bytes each way. GPU median latency beat SIMD, but p95
exceeded SIMD p95 in both candidate runs; sustained throughput was not
measured. Candidate full-phase medians (setup + pipeline + terminal) were
8.972/4.452/7.001/4.405 ms and 8.956/4.443/6.936/4.413 ms for
Pillow/CPU/SIMD/GPU. Keep GaussianBlur open across the L runner gaps, the
missing standard RGB workload, additional modes/sizes, Parallel CPU, and GPU
throughput. The published matrices remain unchanged. The local ledger now has
1,706 rows; report/parity pairs use `gaussian-la-direct-taps-*` and
`gaussian-la-direct-l-control-*` under `build/migration-parity/`. No test
defect was found and no coverage was run.

### In-progress RGB GaussianBlur independent five-tap CPU formula — 2026-10-10

The 1024×768 RGB radius-one fractional horizontal pass now evaluates each
interior byte from its independent three-near/two-far samples rather than a
loop-carried accumulator. Explicit first, second, penultimate, and last-pixel
formulas retain Pillow's replicated-edge taps and fixed-point byte rounding;
rows narrower than five pixels still use the generic path. The focused
generic-reference test passed at widths 5, 6, 7, 17, 65, and 1024. Two current
source controls and two candidate cohorts passed exact Pillow parity for
CPU/SIMD/GPU (3/3 each), with the requested backend executing and no fallback:

| Cohort | Pillow ms | CPU ms | Pillow/CPU | SIMD ms | GPU ms | GPU/SIMD |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Control A | 5.934 | 6.102 | 0.972× | 5.512 | 2.765 | 1.993× |
| Control B | 5.914 | 6.154 | 0.961× | 5.517 | 4.315 | 1.279× |
| Direct taps A | 5.944 | 2.587 | 2.297× | 5.568 | 3.175 | 1.754× |
| Direct taps B | 6.197 | 2.667 | 2.324× | 5.543 | 3.610 | 1.535× |

The CPU candidate clears the local 2× Pillow priority threshold in both
cohorts. Full-phase medians, including setup, pipeline, and terminal
materialization, were 10.446/6.685/9.727/7.333 ms and
10.798/6.937/9.633/8.209 ms for Pillow/CPU/SIMD/GPU. The equivalent observed
call-plus-materialization boundary remains the table's primary latency.
CPU receipts report two full-frame host buffers totaling 4,718,592 bytes.
GPU used six dispatches, one RGB mode conversion, and transferred 3,145,728
bytes each way. Its median beat SIMD in both cohorts; p95 was lower than SIMD
in the first and slightly higher in the second. Sustained GPU throughput was
not measured. SIMD remains only 1.07–1.12× Pillow and therefore misses the
requested acceleration target. This standard RGB workload is declared in the
local manifest but absent from the clean published snapshot, so these are
local results only. Keep GaussianBlur open across the runner gaps, the other
modes and sizes, Parallel CPU, SIMD, and GPU-throughput targets. The published
matrices remain unchanged. The local ledger now has 1,722 rows; report/parity
pairs use `gaussian-rgb-standard-current-*` and
`gaussian-rgb-standard-direct-taps-*` under `build/migration-parity/`. No test
defect was found and no coverage was run.

### GPU shader-dispatch test isolation defect — 2026-10-10

The parallel `cargo test -p pillow-rs --lib luma` run once failed at the
`gpu_native_luma_affine_nearest_transform_reads_and_writes_packed_bytes`
assertion that expected one shader-dispatch record and received twelve. The
earlier assertions had already confirmed exact CPU/GPU pixels, actual GPU
execution, and one dispatch for this transform. Running the affected test alone
passed, and the 49-test luma filter passed with `--test-threads=1`. The cause
was shared test instrumentation: a process-wide enabled flag and dispatch map
let concurrent GPU tests contribute records to each other's receipt.

Unit-test builds now keep this counter thread-local, while non-test builds keep
the process-wide managed-run counter. A direct cross-thread isolation test
covers the boundary. The original one-dispatch assertion remains unchanged;
after the fix, the normal parallel 49-test luma filter passes. This was a test
isolation defect, not an affine-transform parity defect. No code-coverage tool
or coverage suite was run.
