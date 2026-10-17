# GetColors I current-core rebaseline

This is a focused rebaseline of the existing high-cardinality mode-I GetColors workload after the current core extension was rebuilt. No implementation candidate was applied.

## Frozen scope

- Workload: `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`
- Parity case: `PIL.Image.Image.getcolors.nuanced.i-high-cardinality-signed-run-fallback-128x64`
- Target: serial CPU only (`MIGRATION_BENCHMARK_BACKENDS=cpu`)
- Input: mode I, 128×64
- Timed boundary: `observed_steps`, step `call-fallback-4097`; image setup is outside timing. The public GetColors call and Python result construction are inside timing.
- Policy: warm cache, 5 warmups, 20 iterations × 5 samples, concurrency 1. The benchmark correctness gate selected and passed the exact parity case (1/1).

## Result

| Subject | Median latency | Median throughput | Execution proof |
| --- | ---: | ---: | --- |
| Pillow 12.2.0 | 0.166208 ms | 6,016.56 calls/s | reference |
| pillow-rs serial CPU | 0.1782495 ms | 5,610.12 calls/s | requested=actual CPU, terminal complete, no fallback |

The current core is 0.932446× Pillow and 7.24% slower on this local ARM64 cohort. This refreshes the older extension identity but does not clear the CPU target or the 2× sequencing checkpoint. It is one local cohort, not a portable performance claim. The latency includes result construction; allocation telemetry does not account for the Rust result Vec or Python list/tuple allocations.

## Identity and evidence

- Benchmark: `migration-benchmark-f03a99e215e34c60bff667bc6de0a844`
- Parity gate: `migration-parity-benchmark-gate-6c2c16892e1d42fcb49f19fefd3395e9` (1 selected, executed, passed)
- Working-tree revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty
- Host: macOS 15.7.7 ARM64, CPython 3.12.13
- Full loaded target extension SHA-256: `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`
- `pillow-rs/src/image.rs` SHA-256: `b1c5fccaeea4395e02bd839a4d6e9e3deb5e8cdfab7aa75984304c0e849128ad`
- `pillow-rs/src/compute/pool_cpu/ops/filter.rs` SHA-256: `e224b1e8e3d9ac60df9512a3b011fa64a7fdb056f298651670e4861e2848da68`
- Benchmark input SHA-256: `abfb9fe15ce284f84cf775a62358906810ee50fd4fc6f57fc300d291f99ef18c`
- Parity input SHA-256: `bf0fc142ef5ebe4c6da85a6b549316a4ee6ac9d365c5eadb486121010702d9f0`

The focused source audits in Kata `pillow-rs#e1xn` and `pillow-rs#reah` found no untried core counter or safe PyO3 materialization change justified by the available cost evidence. Prior trials remain rejected; this rebaseline adds no optimization claim.

## Files

`benchmark-result.json`, `benchmark-parity.json.gz`, ordered `raw-samples.json`, `execution-attestation.json`, `producer-attestation.json`, and `parity-attestation.json` retain the result and execution identities.
