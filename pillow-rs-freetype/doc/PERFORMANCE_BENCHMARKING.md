# FreeType Performance Benchmarking

This document defines how to produce and review performance numbers for
`fontdone`.

The goal is not to produce the largest possible speedup. The goal is to produce
numbers that another contributor can reproduce, audit, and challenge.

## Required Command

Use repeated samples and the standalone C helper:

```bash
make bench
```

The output is written to:

```text
target/fontdone-bench/latest.json
target/fontdone-bench/latest.md
```

The JSON contains:

- `metadata`: git SHA, dirty flag, toolchain versions, CPU/OS details, matrix
  version, selected workload profile, sample count, and timing notes.
- `rows`: every raw sample row. These are the source of truth.
- `summary.rows`: per-operation aggregate statistics derived from raw rows.
- `summary.overall`: aggregate operation count, Rust total time, C total time,
  total speedup, weighted workload speedup, and overall mean/median/p90/p99
  distributions for Rust time, C time, and speedup.
- `summary.groups`: the same aggregate and distribution values split by timing
  category. Font-load/path-dependent setup is reported separately from cached
  font operations.
- `summary_markdown`: the printable comparison table.

The Markdown report contains:

- benchmark configuration and reproduction command
- the same result table in review-friendly form
- aggregate Rust/C total time and speedup summary
- a group summary that separates cached font operations from font-load or
  path-dependent setup
- overall mean, median, p90, and p99 distributions for Rust time, C time, and
  speedup
- git/toolchain metadata
- CPU model, CPU governor, and detected CPU frequency range
- memory capacity and available memory
- memory speed/clock when the host exposes it; otherwise an explicit
  `not available` value with the source used
- C compiler and FreeType include/library paths

## Trust Labels

Every row in `tests/data/perf_operation_matrix.json` must declare
`comparison_trust`.

- `exact_sha256`: Rust and C output bytes are packed equivalently and exact
  hashes are checked before speedup is trusted.
- `timing_only`: the row has useful C timing and deterministic C fingerprint
  metadata, but C output SHA parity is not yet exact.

Rows marked `timing_only` must not be presented as correctness proof. Exact
correctness remains enforced by fixture parity tests.

## Timing Boundaries

Every matrix row must declare `timing_boundary`.

Examples:

- Font construction rows include font creation and size setup inside the timed
  loop on both Rust and C.
- Cached scalar rows construct the font before timing on both Rust and C.
- Render rows measure load, hint, rasterize, and public bitmap packaging.

Changing a timing boundary is a benchmark-contract change. Review it like a
test semantic change, not like formatting.

## Workload Profiles

The matrix defines named `workload_profiles`.

Use the default profile unless the report explicitly says otherwise:

```bash
make bench BENCH_PROFILE=default BENCH_SAMPLES=10
```

Available profiles currently include:

- `default`: balanced text measurement, glyph metrics, and mask workload.
- `interactive_text`: length, bbox, and grayscale mask dominated workload.
- `font_loading_heavy`: repeated font construction workload.
- `row_weight`: fallback profile using each row's direct `weight` field.

Weights are part of the benchmark contract. Do not change them to improve an
aggregate number. If a workload model changes, update the profile description
and explain the reason in the commit.

## Statistics

Per operation, report:

- Rust and C total milliseconds.
- Rust and C median, mean, standard deviation, p90, and p99 nanoseconds per
  iteration.
- Median, mean, standard deviation, p90, and p99 speedup versus C.
- Operation count across all samples.

Aggregate rows report:

- Total operation count.
- Rust total nanoseconds.
- C total nanoseconds.
- Total speedup versus C.
- Weighted workload speedup versus C.
- Overall mean, median, p90, and p99 Rust time, C time, and speedup
  distributions.

Overall distributions are weighted by operation count. A benchmark row with
40,000 measured operations therefore contributes more to the overall percentiles
than a 100-operation setup row. This avoids presenting row-average statistics as
operation-level behavior.

Speedup percentiles are distributions of per-row speedup ratios. They are useful
for spotting which operation families are faster or slower, but they are not a
replacement for aggregate speedup. Treat total speedup (`C total time / Rust
total time`) and weighted workload speedup as the headline values.

Font load/path-dependent setup is reported separately from cached font
operations. Review cached-operation performance first when evaluating common
text rendering paths; review the setup group separately because path-backed
`FT_New_Face` timing can include filesystem and OS page-cache effects.

The raw rows stay in JSON so reviewers can recompute all summaries.

## Environment Discipline

For publishable numbers:

1. Run a clean worktree, or clearly state why `metadata.git_dirty` is true.
2. Use release mode only. The runner invokes `cargo run --release --locked`.
3. Use at least `--samples 10`; use `--samples 30` for baseline updates.
4. Prefer an idle machine with stable thermals.
5. Prefer a performance CPU governor when available.
6. Keep the full `metadata` block in any shared report.

Do not compare numbers across different machines unless the report is about
cross-machine behavior.

## Validation Commands

Before accepting benchmark tooling changes:

```bash
make bench-self-test
make bench-quick
make test-perf
make test-ffi
```

Before accepting runtime performance claims, also run:

```bash
make test-parity
make test
make bench
```

Parent-project integration gates belong in downstream integration repositories.
They can catch adapter regressions, but they are not required to validate this
standalone crate.

## Non-Negotiable Rules

- Do not weaken fixture parity tests to improve performance numbers.
- Do not link FreeType C into runtime code.
- Do not report timing-only rows as output-parity proof.
- Do not change workload weights without reviewable rationale.
- Do not hide raw samples; summaries must be reproducible from `rows`.
