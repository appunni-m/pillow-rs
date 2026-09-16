# Benchmarking protocol

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
