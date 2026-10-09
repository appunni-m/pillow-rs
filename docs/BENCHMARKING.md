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
resource, and timing data where the adapter exposes them. A result is usable
only when its manifest/input hashes, runtime identity, requested/actual backend,
and terminal receipts are compatible with the comparison.

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
`19d8a1a5ef2c4cbc53895561902ce099c28ed000`, measured
`2026-10-09T06:34:26.573078Z`, from the [published benchmark JSON](https://appunni-m.github.io/pillow-rs/assets/benchmark.json).
It contains 201 `pipeline-op` workload IDs. The operation matrix has 1,320 rows
for 330 public paths and four profiles; 209 paths map to the benchmark manifest
and 121 remain explicit inventory gaps. Only rows with exact parity and
completed backend receipts are eligible for speed claims.

Across the 120 verified serial CPU workload-runner pairs, none is slower than
Pillow and 55 are below 2×. The corresponding SIMD counts are 22 slower than
Pillow, 41 below 2×, and 86 below 5×. The separately built Parallel CPU profile
uses the opt-in Cargo `parallel` feature; 17 of its 120 verified pairs are
slower than Pillow, 46 are below 2×, and 83 are below 5×. GPU latency is slower
than SIMD in 22 of its 38 verified pairs; sustained GPU throughput remains
unmeasured.

The c24 hosted confirmation of the pre-sized native-F thumbnail reducer leaves
no verified serial CPU pair below Pillow. The weakest CPU result is the
1024×768 RGBA alpha-composite-plus-mirror pipeline on Ubuntu ARM at 1.029×
Pillow; native-F thumbnail is next on Ubuntu x86_64 at 1.113×. Both have exact
parity and complete actual-CPU receipts. The Pillow-SIMD snapshot remains dirty
and partial (9 of 34 declared cases), so its ratios are excluded from the
optimization ranking.

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
declared x86 cases (36 rows across 9 workloads). The full-benchmark matrix
therefore retains those rows as dirty evidence instead of treating them as
verified Pillow-SIMD comparisons. Use the published JSON as the source for
rankings, but do not begin the Pillow-SIMD-only phase until the serial CPU
priority cohort has cleared its 2× target and a clean, complete matched
Pillow-SIMD cohort is available.

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
therefore improves the local CPU/Pillow ratio by 8.5–9.8% over the local
baseline, but still misses the 2× sequencing target. SIMD remains about
1.04–1.08× Pillow, below its half-Pillow target. The GPU single-request rows
remain faster than SIMD, but sustained throughput is unmeasured. Keep this as
a local candidate pending clean hosted confirmation. The published matrix
remains the clean c24 snapshot at revision
`19d8a1a5ef2c4cbc53895561902ce099c28ed000`; GitHub benchmark run [#124](https://github.com/appunni-m/pillow-rs/actions/runs/37894520226)
for candidate revision `3e559e4` is still in progress. No genuine test defect
was found; no coverage was run.
