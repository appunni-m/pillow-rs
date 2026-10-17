# RGB GetChannel current-core rebaseline

This is one current-extension baseline run for the existing RGB GetChannel workload. It refreshes the previously reported ARM64 SIMD ratio before any kernel experiment; no implementation change was applied.

## Frozen scope

- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`
- Parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`
- Profiles: serial CPU and single-thread SIMD
- Input: RGB, 1024×768
- Timed boundary: `observed_steps`, `call;observe-result`; setup is excluded and result materialization is included.
- Policy: warm cache, 5 warmups, 20 iterations × 5 samples, concurrency 1.

## Result

| Subject | Median latency | Speedup vs Pillow | Execution proof |
| --- | ---: | ---: | --- |
| Pillow 12.2.0 | 0.1773755 ms | 1× | reference |
| Serial CPU | 0.046083 ms | 3.850× | actual CPU `ExtractBand`, terminal complete, no fallback |
| SIMD | 0.083771 ms | 2.117× | actual SIMD `ExtractBand`, NEON `neon-deinterleave`, 49,152 vector blocks, zero tail, terminal complete, no fallback |

The fresh SIMD result clears the 2× sequencing checkpoint on this local cohort, but remains below the repository's 5× final SIMD target. This current extension differs from the older ARM64 SIMD receipt that measured 1.709×; do not combine their timings. The current run is not an optimization result, and does not compare ARM64 against Pillow-SIMD.

## Identity and evidence

- Benchmark: `migration-benchmark-c42f45900b11461ebb5249352e9cd412`
- Parity gate: `migration-parity-benchmark-gate-1ea3bcaa5fad4157b6cf6a512f8942ea` (2 selected/executed/passed across CPU and SIMD)
- Working-tree revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty
- Host: macOS 15.7.7 ARM64, CPython 3.12.13
- Loaded `_core` SHA-256: `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`
- `pillow-rs/src/compute/pool_simd/ops/native_channel.rs` SHA-256: `4778f65815df404e7de5a42ca11be07827bf88afcf09ea47a9f15ec9a0f68312`
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`
- Benchmark input SHA-256: `940458632a1af7be40e54b3458ef8fa6462fd5bafe1896f6bef7caae43c526cd`
- Parity input SHA-256: `bf0fc142ef5ebe4c6da85a6b549316a4ee6ac9d365c5eadb486121010702d9f0`

The ARM64 unroll-8 proposal is not tested. The change has medium-to-high codegen risk, and this current baseline no longer misses the 2× sequencing checkpoint. A profile or candidate trial should wait until RGB GetChannel is selected against other current-source gaps.

## Files

`A1-control-result.json`, compressed parity report, ordered samples, target/producer attestations, child receipts, and parity attestation retain the full run evidence.
