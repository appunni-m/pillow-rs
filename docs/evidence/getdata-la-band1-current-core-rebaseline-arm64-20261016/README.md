# GetData LA band 1 current-core rebaseline

This refreshes the existing LA 32×32 band-1 GetData row against the currently loaded parity extension. It is a measurement only; no implementation candidate was applied.

## Frozen scope

- Workload: `pipeline-chain.terminal-read.getdata.la-band1-32x32`
- Parity case: `PIL.Image.Image.getdata.performance.material-la-band1-32x32`
- Profiles: serial CPU and single-thread SIMD
- Input: LA, 32×32; band 1 returns the alpha bytes as one L-mode result
- Timed boundary: `observed_steps`, step `band-data`; returned bytes are materialized. Setup is outside timing.
- Policy: warm cache, 5 warmups, 20 iterations × 5 samples, concurrency 1.

## Result

| Subject | Median latency | Speedup vs Pillow | Execution proof |
| --- | ---: | ---: | --- |
| Pillow 12.2.0 | 0.010458 ms | 1× | reference |
| Serial CPU | 0.005667 ms | 1.845× | actual CPU `GetData`, terminal complete, no fallback |
| SIMD | 0.005583 ms | 1.873× | actual SIMD `GetData`, NEON `neon-deinterleave`, 64 vector blocks, zero tail, terminal complete, no fallback |

Both target profiles beat Pillow on this local case but remain just below the 2× checkpoint. This is one ARM64 cohort. The earlier 1.887× SIMD result used a different loaded extension and is not combined with this one.

## Identity and evidence

- Benchmark: `migration-benchmark-1a6ffd1bd285478dbcc5694cfa287c06`
- Parity gate: `migration-parity-benchmark-gate-6d6aef525d244571a328daa8ed4d6e8f` (2 selected/executed/passed across CPU and SIMD)
- Working-tree revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty
- Host: macOS 15.7.7 ARM64, CPython 3.12.13
- Loaded `_core` SHA-256: `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`
- `pillow-rs/src/image.rs` SHA-256: `b1c5fccaeea4395e02bd839a4d6e9e3deb5e8cdfab7aa75984304c0e849128ad`
- Python wrapper SHA-256: `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`
- PyO3 binding SHA-256: `068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24`
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`
- Benchmark input SHA-256: `abfb9fe15ce284f84cf775a62358906810ee50fd4fc6f57fc300d291f99ef18c`
- Parity input SHA-256: `bf0fc142ef5ebe4c6da85a6b549316a4ee6ac9d365c5eadb486121010702d9f0`

The current result does not justify a code edit: prior `pillow-rs#wbp0` audits bounded the no-op view probe and repeated metadata validation below the remaining sub-microsecond gap. Keep invalid/negative-band errors and the immutable post-mutation byte snapshot as guards if this path is revisited.

## Files

The benchmark result, compressed parity report, ordered timing samples, target/producer attestations, child receipts, and parity attestation retain full evidence.
