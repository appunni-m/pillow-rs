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

The latest release-profile evidence (2026-09-10) contains three complete fixed
11-workload runs at source revision
`519226b62b545717c2169be7cbef9141754de91e`. Every run measured 11/11
workloads, produced 44/44 comparable rows, and had 33/33 requested-to-actual
terminal receipts across CPU, SIMD, and native Metal GPU. The first adjacent
comparison reports two timing-only violations and the next reports 16, with
identical execution structure. The result and budget hashes are recorded in
the [pending checklist](benchmark-backend-pending-2026-09-03.md). Earlier
host-access series at `a5a678401` and `de571ae57` remain historical evidence.
These are observations on one arm64 machine, not correctness failures; the
zero-violation acceptance item remains open under the unchanged five-percent
policy.

A fresh clean pair at branch head
`13ea3dc177baa44a5d76ed7d86240aee53402ca8` selected and measured 11/11
workloads, with 44/44 comparable rows and 33/33 terminal requested=actual
CPU/SIMD/native-Metal-GPU receipts in each run. Result SHA-256 values are
`e7d59eacb8af90e3995a5ff22e38b5db1a16c2b6f4684c9bde9bd8c7b2e77ca8` and
`1bc65d682852064f71b74dcfb3becb243cc00d01f74bf17164368cbda0653115`.
The unchanged budget report
`13f1649a545ebafbe17623931bd9a14e6e5a4eb2861f8871975a1b3f1c94551e`
reports 12 statistically credible timing-only violations; the zero-violation
acceptance item remains open.

The release-candidate dependency-pin replay at source commit
`031edd3f5425df2c6931939deb0ad0d93dad376c` (captured before the current
documentation updates; runtime and fixture inputs are unchanged) selected and measured the same
11-workload cohort twice. Each run produced 44/44 comparable rows and 33/33
terminal requested-to-actual receipts across CPU, SIMD, and native Metal GPU.
The unchanged five-percent comparison reported nine timing-only violations;
the result, parity receipts, and budget receipt are retained with the local
first-release bundle. This current-commit evidence also leaves the
zero-violation acceptance item open.

The current release-preparation commit
`027c1e7710b8ac99ea1eb9ccbcc41566b6260cd2` was then measured in two immediate
fixed-ID runs with native Metal GPU access. Both runs measured 11/11 workloads,
44/44 comparable rows, and 33/33 terminal requested-to-actual CPU, SIMD, and
GPU receipts. The unchanged five-percent comparator reported six timing-only
violations; the result and budget hashes are recorded in the [pending
checklist](benchmark-backend-pending-2026-09-03.md). This remains host timing
evidence, and the zero-violation acceptance item is still open.

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

## Website data path

The future benchmark site should consume validated JSON result artifacts, not a
hand-edited Markdown table. A GitHub Pages build will:

1. validate manifest and result schemas;
2. select only compatible runs and retain their source hashes;
3. render correctness, backend, timing, and budget views separately; and
4. publish the generated site as an artifact from a scheduled or manually
   approved workflow.

Until that site exists, `BENCHMARKS.md` is a landing page and the JSON under
`build/migration-parity/` is the evidence record.
