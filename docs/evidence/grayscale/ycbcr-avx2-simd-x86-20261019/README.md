# YCbCr Grayscale AVX2 SIMD trial — x86_64 — 2026-10-19

**Disposition: no-go.** The AVX2 path passed parity and executed real vector
work, but the measured full public call did not demonstrate a gain over the
existing SSSE3 observation. The candidate has been removed from the local
working tree. The runner checkout and its build artifacts remain untouched.

## Frozen scope and source identity

- Public operation: `PIL.ImageOps.grayscale`, input mode YCbCr.
- Workload: `pipeline-op.grayscale.material-ycbcr-noise-1024x768`.
- Exact cases: `PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`
  and `PIL.ImageOps.grayscale.nuanced.performance-ycbcr-odd-byte-tail`.
- Host: native x86_64 Linux, AMD Ryzen 5 2600X, CPython 3.12.3, Pillow 12.2.0.
  Stable run label: `x86_64-ryzen-5-2600x-native-01`.
- Revision: dirty `418d5f0cde51ca64a97cbd007ea731da72d68abf`.
- Loaded target extension SHA-256: `3d4752653f2f7b9a3aef3f91f0bd479f18b14930f6c777c39aae9b7b031589eb`.
- Measured runner kernel SHA-256:
  `c476f26c24d06e5922997547c852997cfbfb79dd2c9d22a0054767d73ccd6234`.
- Pillow-SIMD was not installed on this host, so this experiment makes no
  same-host Pillow-SIMD comparison.

## End-to-end results

Both profiles used the public call plus result observation (`call`,
`observe-result`), warm release execution, 5 warmups, 20 iterations × 5
samples, and concurrency 1. Each run materialized the 786,432-byte output.

| Profile | Pillow median | Target median | Pillow / target | Receipt |
| --- | ---: | ---: | ---: | --- |
| CPU | 1.204291 ms | 0.867135 ms | 1.389× | actual CPU; one mode conversion; no fallback |
| SIMD | 1.212048 ms | 0.862937 ms | 1.405× | actual SIMD; `avx2-deinterleave`; 24,576 vector blocks; zero tail; no fallback |

The CPU and SIMD reports each passed their fresh benchmark parity gate (1/1).
Separate focused parity runs passed both exact cases on CPU and SIMD (2/2 for
each profile). The SIMD report records one 786,432-byte output allocation and
the required result materialization. SIMD throughput median was 1,158.8
operations/s; Pillow was 825.1 operations/s.

The preceding SSSE3 report on this Ryzen host recorded 0.855338 ms SIMD and
1.188005 ms Pillow. The new AVX2 report recorded 0.862937 ms and 1.212048 ms.
Those are separate cohorts, not a paired A–B–B–A; the roughly 0.9% higher
candidate latency is not a causal regression estimate, but it also provides
no demonstrated gain. Do not promote this trial as an optimization. Current
SIMD remains below the 2× sequencing checkpoint and the 5× completion target.
Serial CPU remains faster than Pillow on this host. Earlier reports without a
stable runner label are retained as historical observations, not source-matched
controls.

## Parity and implementation finding

The focused 32-pixel AVX2 block test checked lengths around 32-pixel boundaries
and odd tails. It caught an initial byte-lane mask error: the third 32-byte
load starts at byte offset 64, so its first lane selects indices
`[2, 5, 8, 11, 14]`; its high lane selects `[1, 4, 7, 10, 13]`. The corrected
kernel passed the focused test, the existing YCbCr vector-boundary guard, and
both exact public parity cases. This was an implementation defect caught by a
valid test, not a test defect.

The measured benchmark invocation initially used system Python with Pillow
10.2.0 and stopped at the pinned-oracle identity preflight before timing. The
successful runs used the project `.venv` with Pillow 12.2.0. No benchmark
result was produced by the failed invocation.

## Retained artifacts

The benchmark JSON, raw samples, execution and producer attestations, and
compressed parity reports are retained in this directory. Benchmark run IDs:

- SIMD: `migration-benchmark-4392b23ed30b40e4b1c7e36fb2023430`.
- CPU: `migration-benchmark-02787eec4add43e1b6e9be94a3463685`.
- Focused CPU parity: `migration-parity-aedfeceb93694457aae9b6b0ba5c5aab`.
- Focused SIMD parity: `migration-parity-b801fc6ac3844477a279dd5af7b35c8a`.

The AVX2 candidate is not retained in the working implementation. No baseline
was promoted. GPU, Parallel CPU, other sizes/modes, and current-source
Pillow-SIMD remain unmeasured here. Kata tracking: `pillow-rs#xjn8`.

The local per-run ledger includes the CPU guard and rejected SIMD candidate.
Import into the 6.7 GiB PerfDB could not finish its DuckDB checkpoint because
the local filesystem was at 100% capacity. The database now has a 17 MiB WAL;
leave it untouched and retry status/import only after storage is available.
