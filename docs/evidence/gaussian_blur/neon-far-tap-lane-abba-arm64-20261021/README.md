# ARM64 RGBA GaussianBlur SIMD far-tap reuse — rejected — 2026-10-21

## Decision

Reject the one-pixel far-tap reuse candidate and keep the existing three-load
NEON block. Exact parity and actual SIMD execution passed, but the formal
whole-workflow A–B–B–A measured the candidate 10.33% slower on average than the
matched control. The GaussianBlur operation and its 5× SIMD target remain
open. No baseline was promoted.

## Frozen workload and proof

- Workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Exact parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- Input: RGBA, 1024×768, radius 7.5, deterministic noise
- Profile: single-thread AArch64 SIMD
- Boundary: warm whole workflow through terminal `tobytes` materialization
- Policy: one warmup, 3 iterations × 2 samples, concurrency 1
- Host: `lazys-macbook-pro-arm64`, macOS 15.7.7, CPython 3.12.13, Rust 1.96.1
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`; dirty worktree
- Benchmark producer SHA-256: `343c018ebd7464a4d57bebb8c898352bd1dae9e8e723437d3ccbab662ac6f2b2`
- Control kernel SHA-256: `7649aa692c011ed1d76d90f6845db5bffc56008cd7bb6a2296359871de2927ec`
- Candidate kernel SHA-256: `bd58d5b60f6caf4b7012daed28c5ef3bd897acd5a6a59d7d77d13614b06abbf1`

The control and candidate Cargo source snapshots differ only at
`pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba_neon.rs`.
The materialized control extension SHA-256 is
`83541d6d8c53d510a916005c66d13daced41148651aef6c0c9378750bc98484d`; the
candidate extension SHA-256 is
`b4e94dd63df7edf19d8704c6bc477ec7229845a7410301c1ff15271f4283720e`.

Each formal leg passed its selected exact parity case. All SIMD receipts show
actual `simd`, terminal completion, no fallback, 589,824 vector blocks, and
zero scalar tails.

## A–B–B–A results

Times are milliseconds; Pillow/SIMD is computed within each report. Throughput
is the median whole-workflow operations per second.

| Leg | Variant | Pillow median | SIMD median | SIMD P95 | Pillow/SIMD | SIMD ops/s | Benchmark run | Parity gate |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| A1 | Control | 6.242917 | 5.905313 | 6.196583 | 1.057× | 169.343 | `migration-benchmark-a12c5da2b4694ff4b02b551a46d02334` | `migration-parity-benchmark-gate-c62ad34698dc4f648dc8d05163318eb6` |
| B1 | Candidate | 6.275042 | 6.230334 | 6.342542 | 1.007× | 160.507 | `migration-benchmark-4e11a9aa90fd45b2842ffcc3691ad291` | `migration-parity-benchmark-gate-14f4574ac3c74549a31ab710a9beb9e7` |
| B2 | Candidate | 6.642896 | 7.086688 | 7.444583 | 0.937× | 141.115 | `migration-benchmark-adbd1b38534b42f8bdf10a467506b7db` | Reused B1 parity on the same candidate runtime |
| A2 | Control | 6.425792 | 6.164376 | 6.331417 | 1.042× | 162.224 | `migration-benchmark-7b4c13daa14b4166aa87d2d61cfda109` | `migration-parity-benchmark-gate-d2bf1d03c5414c50a2f657fb4c209f3d` |

Control mean SIMD latency was 6.034844 ms and candidate mean was 6.658511 ms,
10.33% slower. Control mean throughput was 165.783 ops/s and candidate mean
was 150.811 ops/s, 9.03% lower. Both candidate legs were slower than their
neighboring controls. Pillow-relative performance remained near 1×, far below
the required 5× SIMD target.

## Candidate and correctness limits

The candidate replaced the full far-right `vld4_u8` stream with a
`vld4_lane_u8::<0>` of the last tap at x+15 into a copy of the entering tuple,
then used `vext_u8::<1>` to form x+8..x+15. The original entering vector still
fed the signed rolling-sum deltas; Q24 multiplication, fractional arithmetic,
bias, carry, stores, edge handling, and tails were unchanged. This removes
28 source bytes per complete block in the source-level count, but did not
improve the public call.

The existing focused guard
`rgba_radius_seven_neon_prefix_blocks_match_scalar_edges_and_tails` passed on
the candidate across short, odd, full-block, and tail widths. No new test was
added and no full test, parity, benchmark, or coverage campaign ran.

## Identity notes

An earlier control report was not used as A1 because its source snapshot had a
different root `Cargo.toml`, even though its materialized extension bytes
matched. The formal A1 used a fresh current-source control build and parity.
When switching back to a previously built control or candidate extension,
Maturin rematerialization changed the full runtime fingerprint despite an
unchanged extension SHA-256. The benchmark's parity reuse gate correctly
rejected those stale attestations; fresh parity was run for formal A1, B1, and
A2, while B2 reused B1 parity without rebuilding. No runtime-identity gate was
weakened.

The first build-witness invocation passed Maturin where the tool requires a
Cargo build. Its `not_proven` receipt is retained as
`candidate-build-witness-invalid-maturin-command.json`; the separate valid
Cargo and materialization witnesses are the ones used here.

The selected-run packager also rejected the four formal reports because their
benchmark environments lack `MIGRATION_BENCHMARK_RUNNER_ID`. I did not patch
the signed reports or import them as stable-runner evidence. The generated
DuckDB baselines are unchanged; future reports intended for PerfDB admission
must set the runner ID before measurement.

Full reports, samples, parity, runtime/producer attestations, Cargo build and
Maturin materialization witnesses, and the retained diagnostics are in this
directory. `SHA256SUMS` covers all files in the directory except itself.
