# RGBA Filter5x5 rolling rows — source-matched A-B-B-A

This 2026-10-16 native x86_64 trial brackets the serial-CPU rolling-row r9
candidate with control builds. The candidate is in
[`filter.rs`](../../../../pillow-rs/src/compute/pool_cpu/ops/filter.rs); it
keeps the existing uniform-image identity shortcut, then uses five rolling
horizontal channel-sum rows for the exact native RGBA, all-ones 5×5 kernel,
scale 25, offset 0 path when that shortcut misses.

## Frozen scope

The only workloads were:

- `pipeline-chain.convolution.native.rgba-5x5-uniform.1024x768`
- `pipeline-chain.convolution.native.rgba-5x5-nearuniform-tail.1024x768`

The exact parity cases were
`PIL.Image.Image.filter.nuanced.performance-rgba-kernel-5x5-uniform-1024x768`
and
`PIL.Image.Image.filter.nuanced.performance-rgba-kernel-5x5-nearuniform-tail-1024x768`.
The nearuniform image changes the final channel of its final pixel, forcing the
content-identity scan to miss. All four cohorts used `python-cpu`, warm
`whole_workflow`, five warmups, 20 iterations × five samples, concurrency one,
and terminal `tobytes` materialization. The runner used Pillow 12.2.0 on an
AMD Ryzen 5 2600X, Linux x86_64.

The source revision was `f2b63f559e91022a85cb43f7264bdf8b586a448d` and was
dirty in every cohort. A1/A2 disabled only the rolling-row dispatch and used
`filter.rs` SHA-256
`4e7ac2bc6a695ba18710dd8b961fd3265a96cd2972b1defeb7856fef19357a4a` with
loaded extension SHA-256
`e6c49f8774745e499946fe6d7b8aa54f683fede77cf091e21f00b45885ad89a1`.
B1/B2 used the candidate `filter.rs` SHA-256
`dc6297e356471217fdb37b8b52b6a43c99f7b47640eceb2fb95ac7775add8dbf` with
loaded extension SHA-256
`83d8491c207a6eea16968ab5a30f62df3eb2a887c8dbd131cc17f3e8d2557053`.
After A2, the remote checkout was restored to the candidate source and rebuilt;
its loaded extension hash matches B1/B2. No target directory or broad fixture
corpus was copied from the runner, and no remote files were deleted.

## Measurements

Medians and CPU/Pillow ratios are whole-workflow values. P95 is the CPU P95.

| Workload | A1 Pillow / CPU | B1 Pillow / CPU | B2 Pillow / CPU | A2 Pillow / CPU |
| --- | ---: | ---: | ---: | ---: |
| Uniform RGBA | 46.622 / 5.128 ms (9.09×) | 46.845 / 5.394 ms (8.69×) | 46.278 / 5.138 ms (9.01×) | 47.451 / 5.127 ms (9.26×) |
| Nearuniform final-pixel miss | 46.425 / 174.682 ms (0.266×) | 46.942 / 15.584 ms (3.01×) | 47.181 / 14.208 ms (3.32×) | 47.113 / 175.713 ms (0.268×) |

The nearuniform control CPU mean was 175.197 ms; the candidate mean was
14.896 ms, an 11.76× reduction. The unchanged uniform shortcut stayed within
the observed cohort spread. Candidate nearuniform CPU P95 was 16.918 ms in B1
and 14.535 ms in B2.

Each report records requested and actual `cpu`, terminal completion, empty
fallback counts, and CPU `Filter5x5` telemetry. The candidate nearuniform path
also records `PutPixel` for workflow setup. This is serial CPU evidence; it
does not count as SIMD, Parallel CPU, or GPU execution. The timed call includes
the output and five 40,960-byte scratch rows. Runtime host-allocation counters
remain incomplete: they report zero allocations/bytes despite those buffers,
so the end-to-end timings, rather than those counters, are the comparison.

## Correctness and artifacts

A1 executed exact Pillow parity for both cases (2/2 pass), gate
`migration-parity-benchmark-gate-6316df3500814422b8b132f48819df14`. B1
executed both cases (2/2 pass), gate
`migration-parity-benchmark-gate-368dfb35dc2c4ea6b52552afb99efaee`. B2
reused B1's proof after source, extension, manifest, input, oracle, and profile
identities matched. A2 reused A1's proof under the same identity checks. The
control and candidate parity proofs are distinct because their loaded
extensions differ.

| Cohort | Benchmark run | Correctness gate |
| --- | --- | --- |
| A1 | `migration-benchmark-28466277ede44891a1da96d38c6cbe01` | `migration-parity-benchmark-gate-6316df3500814422b8b132f48819df14` |
| B1 | `migration-benchmark-b2b0db246a214ee5ae5a7012a9c79593` | `migration-parity-benchmark-gate-368dfb35dc2c4ea6b52552afb99efaee` |
| B2 | `migration-benchmark-c311a8ca05be41369c96b67126511ef4` | reused B1 |
| A2 | `migration-benchmark-bada106a0f904032bd747938a209fb89` | reused A1 |

Each cohort directory contains the benchmark report, compressed parity result,
execution and producer attestations, attestation children, and raw samples.
`evidence-transfer.tar.gz` is the compact transfer archive; its uncompressed
parity payloads were 33 MiB each and were compressed to about 68 KiB each.
`SHA256SUMS` covers the retained files. The focused manifest and input slices
are reused from `../candidate-r8/`; their SHA-256 values are `40bb4ee333f797cdf73fb4efcb276331fd009489a4c856c6f1fb4a32a94538a3`,
`1313949032c0c85a8b7d3d3f2d0b72dd3ebdb14610d202729b9adbdf18c67bc5`, and
`1f2277bbdbb2c0786bc9792d4c0f429be9549e9ba7518219751238f6598ff42e` for the
manifest, benchmark input, and parity input respectively.

The trial supports retaining this narrowly gated serial-CPU candidate for the
tested RGBA 1024×768 all-ones kernel. It does not close Filter5x5 across other
modes, sizes, kernels, SIMD, Parallel CPU, or GPU. These dirty-tree results do
not update the pinned clean operation matrix; the 16 Pillow/CPU subject rows
are recorded in `docs/evidence/performance-optimization-local-runs.csv`.
No full campaign, test suite, coverage, commit, or push was run.
