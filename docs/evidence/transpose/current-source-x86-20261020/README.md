# RGBA Transpose current-source x86 refresh — 2026-10-20

The later CPU-only refresh supersedes the serial CPU row in this report:
[current-source CPU refresh 02](../current-source-x86-refresh-20261020-02/README.md).
The SIMD measurement below remains the latest SIMD evidence for this workload.

## Frozen scope

- Public operation: `PIL.Image.Image.transpose`.
- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`.
- Input: RGBA, 772×768; `transpose-0;transpose-1;materialize`.
- Boundary: warm end-to-end call including result materialization and Python
  `bytes` output.
- Sampling: release build, 5 warmups, 20 iterations × 5 samples, concurrency 1
  (100 observations per subject).
- Host: AMD Ryzen 5 2600X, Linux x86_64, CPython 3.12.3, Pillow 12.2.0.
- Main parity case passed on both target profiles:
  `PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`.
- Odd-size parity guard passed separately on both profiles:
  `PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0`.

## Result

Ratios are computed from the Pillow median in this same report divided by each
target median. A value below 1 means the target was slower than Pillow.

| Profile | Pillow median | Target median | Target p95 | Pillow / target | Actual execution |
| --- | ---: | ---: | ---: | ---: | --- |
| Pillow | 2.665251 ms | 2.665251 ms | 3.588559 ms | 1.0000× | Pillow oracle |
| Serial CPU | 2.665251 ms | 3.045027 ms | 3.335818 ms | 0.8753× | CPU, 2 operations, terminal complete, no fallback |
| SIMD | 2.665251 ms | 2.696892 ms | 3.621239 ms | 0.9883× | SIMD, AVX2 transpose receipt, terminal complete, no fallback |

CPU misses the Pillow latency limit. SIMD is slightly slower than Pillow and is
far from its 5× target. The SIMD receipt records 73,728 AVX2 vector blocks and
12,288 scalar-tail bytes. Both target profiles report one 2,371,584-byte host
allocation and two live host buffers totaling 4,743,168 bytes, with no mode
conversion, upload, readback, or fallback.

The two operation receipt entries do not mean two image passes. The automatic
SIMD segment composes method 5 followed by method 0 into one `Rotate270` kernel
call; the receipt retains attribution for both public operations. The
composition path is already present and was not changed in this refresh.

## Diagnostic attribution

One opt-in diagnostic call used a deterministic synthetic RGBA buffer of the
same dimensions and the same `TRANSPOSE;FLIP_LEFT_RIGHT` chain. It reported:

- AVX2 executor: 1,718,604 ns
- Python byte materialization: 1,268,493 ns
- SIMD output allocation and zero-fill: 12,551 ns

This single instrumented call is diagnostic only. It is not a performance
comparison; the input pixels were synthetic. It shows that allocation is not
the useful target for this case. The later 2×2 AVX2 macroblock experiment
passed selected parity but regressed the full SIMD call by 6.69–7.87% in two
repeats; see the [2×2 macroblock no-go evidence](../avx2-2x2-macroblock-x86-20261020/README.md).
The already screened transpose-composition work and prior reverse-row,
sparse-tail, uninitialized-output, and tile-order variants are excluded. A
future experiment needs a different source-backed reason to reduce the AVX2
executor or required materialization cost.

## Source and artifact identity

- Git revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf` (dirty worktree).
- Local and runner source snapshots matched: 292 Rust files,
  SHA-256 `d3a428988512626d743e2351ed86ec074ceb166e5c1f17c31b84bc69e7ffdab1`.
- Release extension SHA-256:
  `13e9f40576c0956b96b8895285f2c4bd1dabda12f28d56c2f61f7988ff09a6b3`.
- Benchmark run: `migration-benchmark-8886fc8ae30e44c7b00140057579801b`.
- Benchmark JSON SHA-256:
  `2a81365f766ffaa78c15569662f3dee7212944b46c1258c79021b7d5a734daad`.
- Main parity evidence:
  `migration-parity-benchmark-gate-0c75173405684c3c926f469afa73a516`;
  JSON SHA-256
  `3d49c2cfa0c73d6488d44d1cef9928f3842403cc360404d08b37b3a1f256f4f3`.
- Odd CPU parity run: `migration-parity-a2b98ad0a56a4956a02958392f7ace15`.
- Odd SIMD parity run: `migration-parity-64dae6e385d3490d9d9632199c5bdc7e`.
- The selected-run rows are appended to
  `docs/evidence/performance-optimization-local-runs.csv`.

Raw benchmark and parity JSON remain in `/tmp` on the existing native runner;
no target directory or large report was copied. No full campaign, full parity,
or coverage ran. This is a one-run current-source refresh, not an A–B–B–A
candidate result.
