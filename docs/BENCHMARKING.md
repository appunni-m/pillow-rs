# Benchmarking protocol

This is the contributor guide for collecting and interpreting measurements.
For comparisons, start with [benchmark results](https://appunni-m.github.io/pillow-rs/benchmarks/).

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
receipts must include histogram/LUT/remap execution and complete transfers.
`--check-only` verifies one window per depth without emitting timing summaries.
The transpose selector remains the default with its existing workload policy.
Use `--operation invert` and a separate output path for fresh L/RGB inversion
under the same window policy. Inversion requires one GPU dispatch and complete
transfers; it uses the original full-range input tile.
Use `--operation blend` for two fresh images blended at alpha 0.3. The second
image uses the next changing input frame; construction of both images is timed,
and GPU receipts must include its auxiliary image transfer.
Use `--operation add` or `--operation subtract` for the same two-image boundary
with default scale 1 and offset 0. Their input sequence, window policy, and
auxiliary-transfer checks are identical; each operation uses a distinct output
path.

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
