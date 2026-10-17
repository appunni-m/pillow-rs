# Current-source GetChannel native x86 baseline — 2026-10-15

## Scope and identity

This run selected one public workflow and its exact parity case:

- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`
- Parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`
- Input hashes: workload `f0203987ebe336b57fe121e3a4d8a6f2cdb1e787d8c28c2d9192cb1a8d1dd5cf`; case `ca5d07d413f695119196f9bdb980f2fc6bfb788173afed973c1f124907790e96`
- Host: AMD Ryzen 5 2600X, Linux 7.0.0-38, x86_64; CPython 3.12.11
- Oracle: Pillow 12.2.0. Target: pillow-rs 12.2.1, Rust 1.96.1, Maturin 1.14.1

The target worktree used base commit `f2b63f559e91022a85cb43f7264bdf8b586a448d` plus the tracked source/harness diffs and small untracked source files from the current local dirty tree. `source-overlay.tar.gz` preserves that overlay. `input-slice.tar.gz` contains the current manifest in `scope: slice` mode and only the selected case/workload documents. Their selected object hashes match the current full input files. No `target/` directory or full parity corpus was copied.

## Correctness and execution

The focused fixture contract validator passed. The benchmark's parity preflight passed 1/1 comparison against regular Pillow. The timed boundary was `call` plus `observe-result`, with warm cache, five warmups, 20 iterations × five samples, and concurrency one. Each subject contributed 100 timing observations. Image construction stayed outside the measured steps; result materialization was included.

The target receipt reports one completed `ExtractBand` on actual CPU, no fallback, one 786,432-byte output allocation, and one mode conversion. The loaded target extension was `pillow_rs._core` at version 12.2.1, with SHA-256 `0a5b618df3e87b75d65c5ead9860bb9205d42b30251d28bf15d688bf7b1faa8f`. Parallel CPU was disabled. SIMD and GPU were not selected.

## Result

| Subject | Median latency | P95 latency | Median throughput |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 0.313946 ms | 0.360251 ms | 3,185 ops/s |
| Serial CPU | 0.307346 ms | 0.485192 ms | 3,254 ops/s |

The median ratio is 1.0215× Pillow, a 6.6 μs difference. This single dirty-tree cohort barely clears the median CPU threshold for this one RGB size; its P95 is slower than Pillow and it misses the 2× checkpoint by a wide margin. It does not close GetChannel, replace the pinned clean matrix, or attribute a gain to a particular source change. Keep the existing below-Pillow rows for other source/mode comparisons visible until they are rebaselined.

## CPU 4 affinity sensitivity check

A timing-only follow-up used `taskset -c 4` with the same worktree, extension,
workload, case, Pillow oracle, and timing conditions. The runner validated and
reused the exact passing parity report from the first run; it did not execute
parity a second time. The benchmark selector was explicitly restricted to
`MIGRATION_BENCHMARK_BACKENDS=cpu` to match that CPU-only proof.

| Subject | Median latency | P95 latency | Median throughput |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 0.377907 ms | 0.759554 ms | 2,646 ops/s |
| Serial CPU | 0.311687 ms | 0.456062 ms | 3,208 ops/s |

The CPU median ratio is 1.2125× for this cohort. The receipt again reports
actual serial CPU, one completed `ExtractBand`, terminal completion, no
fallback, one 786,432-byte output allocation, and one mode conversion. The
selected core SHA-256 matches the first run. This one pinned cohort contains
load-sensitive timing outliers and is only an affinity sensitivity check; it
does not replace the first measurement, establish a stable matched speedup, or
meet the 2× checkpoint.

Run `migration-benchmark-4346db456fd14a1e8cb940b92346ae3c` used the validated
parity proof `migration-parity-benchmark-gate-7d3653849f2e4ae78ec8f6b93a388215`.
The attempted `r3` run stopped at reused-proof profile validation before any
image execution; its output reservation was preserved. The successful timing
used fresh `r4` paths. No remote files were deleted.

## LA and RGBA current-source rebaseline

A separate CPU 4 cohort selected only the two still-below-2× published
GetChannel rows: LA and RGBA at 1024×768. It used the same target extension,
Pillow 12.2.0, warm full `call` plus `observe-result` boundary, five warmups,
20×5 samples, and concurrency one. Fresh exact parity passed 2/2.

| Mode | Pillow median / P95 | Serial CPU median / P95 | Median speedup | CPU median throughput |
| --- | ---: | ---: | ---: | ---: |
| LA | 0.347711 / 0.412392 ms | 0.311196 / 0.405062 ms | 1.1173× | 3,213 ops/s |
| RGBA | 0.359021 / 0.475842 ms | 0.349877 / 0.433392 ms | 1.0261× | 2,858 ops/s |

Both CPU rows are above Pillow for this dirty-tree native x86 source and host,
but remain far below 2×. Each CPU receipt proves one completed `ExtractBand`
on serial CPU, terminal completion, no fallback, one 786,432-byte output
allocation, and one mode conversion. The two-case input slice is
`input-slice-la-rgba.tar.gz`; no full fixture or target directory was copied.
This evidence is diagnostic and does not replace the clean matrix or close
GetChannel across all sizes, architectures, and profiles.

Run `migration-benchmark-65fb0975ca7c4c2581f1181490093ced` used parity proof
`migration-parity-benchmark-gate-8fa924e307fd426c83ef495795a869a8`.

## Runner notes

The first attempt stopped before image execution because the remote checkout has sparse checkout enabled and omitted a manifest-indexed parity file. The selected-scope manifest allowed contract validation and the focused run without materializing the 37 MB parity index. A second attempt stopped before parity because the runner had already reserved the producer-attestation output name before loading the manifest; that reservation is preserved. The successful run used a fresh `-r2` output directory. Neither failed attempt ran the image workload.

## Evidence files

- `benchmark-result.json`: validated focused benchmark result; SHA-256 `dc197e8702d2d6520a13a53fc0288a1666d9785a4defce1b196b70370142ad95`
- `parity-result.json.gz`: compact copy of the passing parity report; uncompressed SHA-256 `f81c23a97b31cc29893e0c8352ad3f0d4d2cdc1c6210808e5c88b0b42b39269f`
- `raw-samples.json`: ordered raw samples; SHA-256 `a97cd4b3396dc96ee93aa6e5949afb54add6c803844e877278474ecc3ca242da`
- `attestations.tar.gz`: execution attestations, child receipts, producer identity, and producer bundle
- `benchmark-result-cpu4.json`: validated CPU 4 timing result; SHA-256 `53b71fe875d6baba2b570bced332e9632899b007f7f26399f02565810d069f25`
- `parity-result-cpu4.json.gz`: reused exact passing proof; compressed SHA-256 `520a14e8ff4f51eba6fd3cff2f08bf7dfa1d515e93354492cf40226c570a9297`
- `raw-samples-cpu4.json`: ordered CPU 4 samples; SHA-256 `aa9bced9d5ffec87e9cd6ded34c6130c8be54b9616fc2b6e887a98f223543c7d`
- `attestations-cpu4.tar.gz`: CPU 4 execution and producer attestations; SHA-256 `130c890972415f7cf783e0aec45a8ee59f9bb790a27998ae4e59e0d8ed143f5b`
- `benchmark-result-la-rgba.json`: two-mode current-source result; SHA-256 `58652bea3b0a369ecfabaa6e697c3e6d919026790d4d19c247bd44fd2693f0c2`
- `parity-result-la-rgba.json.gz`: focused 2/2 parity report; SHA-256 `84397d424556b08864aca76880c9a225f92496f59b0f260dd04df25e7769b328`
- `raw-samples-la-rgba.json`: ordered LA/RGBA samples; SHA-256 `8575f63f6975522fda54f5ffd55d7ff36f8d7824820bbe9840c6e6c5e20f43a4`
- `attestations-la-rgba.tar.gz`: execution and producer attestations; SHA-256 `eb4498dc3697ab48abb25ab3b41b31d0287cdab19054494b8d9eed48976e5ca4`
- `input-slice-la-rgba.tar.gz`: only the selected workload rows, parity cases, and slice manifest; SHA-256 `76d5346a29563fc9b16af6e7f5d07e49b6d75d16ed748b005d21962bd82e5253`
- `source-overlay.tar.gz` and `input-slice.tar.gz`: compact source and input snapshots for this run

The remote worktree and its generated build/evidence files remain at `/home/appunni/pillow-rs-getchannel-x86-baseline-20261015`; existing runner directories and artifacts were left untouched.
