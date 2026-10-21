# GetChannel RGB x86 CPU rebaseline and phase probe — 2026-10-20

This selected run refreshed the only currently evidenced serial-CPU case below
Pillow, then separated its public `getchannel()` call from result observation.
It produced no implementation change or optimization credit.

## Selection and result

- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`
- Parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`
- Profile: `python-cpu`; actual backend `cpu`; default features.
- Boundary: warm public `call` plus `observe-result`, including terminal
  materialization and Python byte export.
- Policy: 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Fresh parity run `migration-parity-benchmark-gate-1602bcbbce3f4281b538c3c31d0f601a`
  passed its one selected case.

| Subject | Median | P95 | Pillow / target |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 297.076 µs | 372.701 µs | 1.0000× |
| pillow-rs serial CPU | 305.521 µs | 319.641 µs | 0.97236× |

The target is 2.84% slower by median. The new ratio is better than the prior
0.9519× report, but both Pillow and target timings moved; this is a refreshed
control, not evidence of a code speedup.

The 100 raw paired samples place target `getchannel()` at 7.725 µs median and
`observe-result` at 295.731 µs. Their medians do not add exactly because they
are calculated independently; the paired residual median is 2.135 µs.

## Per-call diagnostic telemetry

A separate target-only run enabled pipeline telemetry for 105 warm calls of the
same selected case and extension. It is not the scored timing run. All 105
receipts report actual CPU, completed terminal materialization, and no fallback.

| Diagnostic field | Median | P95 |
| --- | ---: | ---: |
| Core `backend_ns` | 264.491 µs | 274.656 µs |
| `python_bytes_copy_ns` | 20.040 µs | 22.895 µs |

The final Python bytes copy accounts for roughly 6.6% of the uninstrumented
full-call median. Most time remains in core backend work. `backend_ns` does not
isolate the inlined RGB extraction loop from pipeline materialization, so this
probe does not establish a new kernel candidate. Its instrumented full-call
median was 310.156 µs and is diagnostic only.

## Identity and evidence limits

- Revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty checkout.
- Runner: `native-x86-192.168.0.221`; AMD Ryzen 5 2600X; CPython 3.12.3;
  Rust 1.96.1; Linux x86_64.
- Loaded extension SHA-256:
  `7b7433a32cd1a4c49dd4019d328b673f5cf960c16a6cef2133a6123ce118a066`.
- Relevant source hashes matched the local checkout: `color.rs`
  `1e0536eeccdde70eac33495bbdc1553749021f4f63a81a5e1d9acf6daa7dc732`,
  `image.rs` `2cd8961958c97a2832ead1c823d1855b51abeee7a79be8e2fdade40d6ccb03d5`,
  PyO3 `lib.rs`
  `cff227d114b697d1f941ef4ba2788ce166d73e86d044f4b84e4edcefe511ef2c`, and
  Python `image.py`
  `da25efb014afe647295a81f6beae6e4864a5810f8e0b3ecc32f6f992eb22e1e8`.
- Canonical hashes of the selected workload object and parity case object
  matched local: `f0203987ebe336b57fe121e3a4d8a6f2cdb1e787d8c28c2d9192cb1a8d1dd5cf`
  and `ca5d07d413f695119196f9bdb980f2fc6bfb788173afed973c1f124907790e96`.
  The full manifest and fixture-file hashes differ from local, so this is
  selected-object evidence, not a strict whole-manifest baseline.
- Report run ID: `migration-benchmark-4da2f2d618bf4687be07a08ea17f166f`.
  The report's `identity.command.argv` says `make migration-parity-benchmark`,
  although the selected run invoked `scripts/run_migration_benchmark.py`
  directly. Preserve this invocation-provenance defect for a focused harness
  repair; the selector and child receipts remain explicit.

The x86 host's original reports remain under
`/home/appunni/work/pillow-rs-current-x86-20261020/build/migration-parity/`;
only these selected reports, compressed parity payload, and phase receipts were
copied here. Nothing was removed from the host. No full campaign, test suite,
coverage, baseline promotion, commit, or push ran.
## Tracking and baseline handling

The two Pillow/CPU rows from this selected run were appended to
`docs/evidence/performance-optimization-local-runs.csv`. The benchmark report
was imported into the generated DuckDB at
`/Users/lazytrot/work/pillow-perfdb/performance-results.duckdb`; no strict
baseline was promoted. PerfDB retains this report as a selected observation,
but its whole-manifest/input identities differ from the locally pinned
comparison cohort, so the verified rank still uses the earlier comparable
0.9519x x86 result. This refreshed 0.97236x result is recorded for investigation
and does not replace that strict comparison.

The invocation provenance mismatch is recorded in Kata task `pillow-rs#jpj6`
for harness follow-up. All evidence files in this directory are listed in
`SHA256SUMS`.
