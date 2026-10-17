# GetColors high-cardinality current-core CPU rebaseline — 2026-10-17

## Scope and evidence

- Workload: `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`
- Parity case: `PIL.Image.Image.getcolors.nuanced.i-high-cardinality-signed-run-fallback-128x64`
- Input: mode I, 128×64 (8,192 pixels; the case returns 4,097 rows).
- Timed public step: `call-fallback-4097`; this includes Python result materialization.
- Policy: 5 warmups, 20 iterations × 5 samples, concurrency 1, warm cache.
- Benchmark: `migration-benchmark-73041cb53195411090213fd8341d02dc`; parity gate: `migration-parity-benchmark-gate-2a5cf113e73447d881a51da704dd6749`.
- Parity: 1/1 pass against Pillow 12.2.0.

## Result

| Subject | Median call | P95 | Samples |
| --- | ---: | ---: | ---: |
| Pillow | 176.333 µs | 188.292 µs | 100 |
| Serial CPU | 185.896 µs | 203.625 µs | 100 |

Pillow/CPU speedup is 0.9486×; this one current-core run leaves CPU latency about 5.4% higher than Pillow. The CPU receipt shows requested and actual CPU, one completed GetColors operation, terminal completion, and no fallback. Sample standard deviations are high because of occasional multi-millisecond outliers; retain this as a prioritization signal, not a stable optimization result.

## Identity

- Revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty worktree); macOS-15.7.7-arm64-arm-64bit, CPython 3.12.13, Pillow 12.2.0; default serial CPU features.
- Loaded `_core.abi3.so`: `e0e3e5373c0bb2c2779f2fc6b38631ce35b951528dac852808c6d3cf0cc3f847`.
- `pillow-rs/src/image.rs`: `dcd38ebeeba2b76b0c65ee1430b109833af158bc11a932c451716f3417cd07c8`.
- `pillow-rs-py/src/lib.rs`: `068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24`.
- `pillow-rs-py/python/pillow_rs/image.py`: `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`.
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`; parity input SHA-256: `c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`.

This is a focused current-source rebaseline, not an optimization trial or a clean published matrix row. No source was changed. The earlier PyO3 phase probe is diagnostic only and used a different instrumented extension.

## Runner-bound confirmation

The first report omitted the stable runner ID, so the separate DuckDB correctly
kept it out of cross-run ranking. A second one-workload capture set
`MIGRATION_BENCHMARK_RUNNER_ID=lazytrot-mac-local-arm64`; its exact parity
preflight passed 1/1 and used the same normal `_core` hash and fixture. The
report is `benchmark-runner-id-final.json`, run
`migration-benchmark-b3bf896243af42a2a168f0e8723576fc`, with parity gate
`migration-parity-benchmark-gate-6734c78f0672439395f01dadfca91d66`.

| Subject | Median call | P95 | Samples |
| --- | ---: | ---: | ---: |
| Pillow | 178.250 µs | 216.958 µs | 100 |
| Serial CPU | 186.937 µs | 337.833 µs | 100 |

Pillow/CPU speedup was 0.9535× (CPU latency 4.9% higher). This is consistent
with the first 0.9486× run but has a higher target P95; keep both as noisy
current-core miss signals. The measured boundary, actual CPU receipt, terminal
completion, and no-fallback status match the first run.

The reused parity proof from the unbound run was correctly rejected because
the oracle runtime identity did not match the runner-bound process. The first
retry then stopped before parity/timing because its rejected invocation had
left a producer-bundle file at the chosen output name; the successful run used
a new output stem. Keep the orphan bundle as an invocation artifact, but do not
treat it as a benchmark result or parity proof.
