# RGB GaussianBlur SIMD vertical recurrence trial

**Disposition: rejected.** The trial preserved exact parity and executed SIMD,
but its complete public-call median was 2.19% slower than the control. The
candidate code was removed from the local source. The exact candidate patch and
all benchmark artifacts are retained here for review.

## Frozen workload and execution

- Workload: `pil-imagefilter.gaussianblur.standard`
- Parity case: `PIL.ImageFilter.GaussianBlur.nuanced.performance-material-rgb-noise-1024x768-radius-2`
- Contract: 1024×768 RGB, radius 2, observed `apply-filter` and
  `observe-filter-result`, warm cache, 5 warmups, 20 iterations × 5 samples,
  concurrency 1. The complete call includes image setup and final result
  materialization.
- Backend: requested and actual `simd`; terminal complete; no fallback; the
  receipt reports one `GaussianBlur`, vector path, 882,432 vector blocks and
  36,864 scalar-tail units.
- Host: native x86_64, AMD Ryzen 5 2600X, Linux 7.0.0, CPython 3.12.11,
  Pillow 12.2.0. The loaded baseline extension SHA-256 was
  `6282923bed4284d8118f4d52260bf036420e485b1791e1b6cb3908886a293d7e`; the
  candidate extension SHA-256 was
  `83d8491c207a6eea16968ab5a30f62df3eb2a887c8dbd131cc17f3e8d2557053`.
- Source revision: `f2b63f559e91022a85cb43f7264bdf8b586a448d`, dirty
  experiment worktree. Baseline and candidate function hashes were
  `0c1f5dff31e9b8d4e2dc1c2b826253dcc73b04f27d47add2b1512a0d2d84989f` and
  `5957a079465875bc6f75084ae60b8eaec7d5d585b5cc572f7e5aacf09e16dc1c`.

The runner checkout did not contain 73 indexed fixture documents. To avoid
copying the fixture corpus, the run used a validated focused manifest containing
the unchanged selected workload and parity case. The workload and case records,
including their sample settings and image bytes, were preserved exactly. The
focused manifest SHA-256 is
`6d2799c2bc2bc97427f2f6c15c81c409ae5dde5a249a9f1d6d520ea62368689c`; its
benchmark and parity input SHA-256 values are
`9d951c7a7c0bc5b44686951c2069cece382b10bdc79bd2a29a30c4261be3c48d` and
`f1fd80c0fd1e5d83612d06e114816b7ec343208502c7e1bcc021eb77d340c06f`.

## Results

| Run | Pillow median | SIMD median | SIMD/Pillow speedup | Parity |
| --- | ---: | ---: | ---: | --- |
| Control A1 | 30.994395 ms | 12.547145 ms | 2.470× | pass 1/1 |
| Control A2 | 30.539222 ms | 12.512229 ms | 2.441× | reused exact A1 proof |
| Candidate B1 | 31.300543 ms | 12.716492 ms | 2.461× | pass 1/1 |
| Candidate B2 | 31.298750 ms | 12.890950 ms | 2.428× | reused exact B1 proof |

Control SIMD median: 12.529687 ms. Candidate SIMD median: 12.803721 ms
(2.187% slower). The change is not a performance improvement. The candidate
kept the same 882,432 vector-block receipt and the same public-call allocation
profile: one 2,359,296-byte host allocation and two 4,718,592-byte live host
buffers. It changed only `simd_rgb_vertical_radius_one_rows` to carry the
vertical accumulator while processing 16-byte column chunks across all rows.
The x-major traversal increased stride between writes; the measurement rejects
this loop shape. Cache or store locality is a plausible cause, not separately
proven.

Both parity runs completed with zero failures. The focused kernel test
`rgb_radius_one_direct_vertical_passes_match_transposed_simd` passed 1/1 and
covers short heights, odd RGB tails, three passes, and fractional radii.

The public [Pillow-SIMD benchmark snapshot](https://appunni-m.github.io/pillow-rs/assets/pillow-simd-benchmark.json)
selected this workload because its 2026-10-13 Ubuntu AMD EPYC row measured
10.987305 ms for pillow-rs SIMD and 9.241339 ms for Pillow-SIMD. That snapshot
is a prioritization signal only: its CPU differs from this Ryzen runner, so the
timings are not a same-host comparison. The current runner result measures
Pillow 12.2.0 and does not claim a current Pillow-SIMD comparison.

Run IDs: A1 `migration-benchmark-8775b6814cd447099fc58ae777f2e642`, A2
`migration-benchmark-f0a64943f737402389f5129fe432e64f`, B1
`migration-benchmark-9fb83769d8d643a982f554aaeaaff213`, B2
`migration-benchmark-8f0a3088dc244f13bf2d45f7d400195c`. Exact artifact hashes
are listed in `SHA256SUMS`.
