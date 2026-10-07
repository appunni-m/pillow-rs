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
curl -fsSL https://appunni-m.github.io/pillow-rs/assets/benchmark.json -o /tmp/pillow-rs-benchmark.json
curl -fsSL https://appunni-m.github.io/pillow-rs/assets/pillow-simd-benchmark.json -o /tmp/pillow-rs-pillow-simd.json
PYTHONPATH=pillow-rs-py/python .venv/bin/python scripts/build_public_api_inventory.py
python3 scripts/build_performance_optimization_matrix.py \
  --benchmark-snapshot /tmp/pillow-rs-benchmark.json \
  --pillow-simd-snapshot /tmp/pillow-rs-pillow-simd.json \
  --public-api-inventory docs/evidence/performance-optimization-public-api.csv
```

The committed workload matrix records 8,655 rows total: 8,619 full-benchmark
rows across 663 workloads from the clean 2026-10-07 snapshot and 36
Pillow-SIMD rows across 9 workloads from an older dirty snapshot. The operation
matrix has 1,320 rows for 330 public paths
and four profiles. The full snapshot has 201 `pipeline-op` workload IDs; only
workload rows with exact parity and completed backend receipts are eligible for
speed claims. The Pillow-SIMD snapshot holds 9 of the 34 declared cases, so its
ratios remain diagnostic until a complete clean cohort is published. The latest
Pillow-SIMD workflow failed with exit code 2; GitHub exposes its diagnostics only
to signed-in users, so the cause remains unverified.

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
