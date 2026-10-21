# RGBA GaussianBlur SIMD tiled transpose — ARM64 — 2026-10-18

> This is the preliminary portable-dispatch cohort. The final implementation
> restricts the optimization to AArch64; its exact source/build A–B–B–A and
> guard recheck are in
> [`gaussian-rgba-tiled-transpose-simd-aarch64-abba-20261018`](../gaussian-rgba-tiled-transpose-simd-aarch64-abba-20261018/README.md).
> Use that scoped evidence for the current code.

## Finding

For the large native RGBA radius-7.5 Gaussian SIMD path, route its two
interleaved-frame transposes through the existing 4×4 portable-SIMD tile
transpose. The dispatch requires the existing exact Gaussian RGBA kernel
candidate and both dimensions at least 256 pixels. BoxBlur, other modes, small
images, and the opt-in Parallel CPU feature retain their existing transpose
path. The uniform-image identity return occurs before either transpose.

The source-matched A–B–B–A full-workflow trial cut the noisy RGBA median from a
12.347 ms control mean to a 7.527 ms candidate mean (1.64× faster, 39.0% lower
latency). It still misses regular Pillow: candidate SIMD was 7.40–7.65 ms and
same-run Pillow was 6.49–6.80 ms. The SIMD/Pillow ratios were 0.876× and 0.889×;
the operation remains slower than Pillow and far from the campaign's 2×/5× SIMD
targets. Keep GaussianBlur open.

The uniform workload guards the existing identity path and does not exercise
the new transpose. No uniform-input gain is attributed to this change.

## Frozen cohort

- Workloads: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768` and
  `pipeline-chain.blur-material.gaussian-rgba-1024x768`
- Exact parity cases:
  `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
  and
  `PIL.Image.Image.filter.nuanced.performance-material-rgba-gaussian-1024x768-radius-7-5`
- Profile: `python-simd`, release build, default features (no `parallel`)
- Boundary: warm `whole_workflow`, including setup/frombytes, filter, and
  tobytes materialization
- Policy: one warmup, three iterations × two samples, concurrency one
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7 ARM64, CPython 3.12.13, Pillow
  12.2.0
- Revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty tree
- Each leg selected and passed both exact parity cases (2/2); no selected cases
  were skipped.

Pillow/SIMD is the same-run Pillow median divided by the SIMD median. A value
below 1 means SIMD is slower.

| Leg | Noise Pillow | Noise SIMD | Pillow/SIMD | Uniform Pillow | Uniform SIMD | Pillow/SIMD | Benchmark run | Parity gate |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| A1 control | 6.134 ms | 12.083 ms | 0.508× | 6.236 ms | 0.733 ms | 8.506× | `migration-benchmark-8fbc8a10bb6949debf76074e1091e6f2` | `migration-parity-benchmark-gate-5c87ed60ad8b49a4ac697d4755a2efb2` |
| B1 candidate | 6.486 ms | 7.403 ms | 0.876× | 6.480 ms | 0.563 ms | 11.509× | `migration-benchmark-a37e1b45d8bb4869bc4571b0ef597c18` | `migration-parity-benchmark-gate-c0bdf5e1a16c450a8369f54301127289` |
| B2 candidate | 6.798 ms | 7.650 ms | 0.889× | 6.527 ms | 0.588 ms | 11.094× | `migration-benchmark-1f79e114ef9847f182e4184b67f21415` | `migration-parity-benchmark-gate-c3a471314f6d49c6a249f0009a6624ae` |
| A2 control | 6.505 ms | 12.611 ms | 0.516× | 6.872 ms | 0.662 ms | 10.376× | `migration-benchmark-0e6c8e965c05416c83442d9f9e206bb1` | `migration-parity-benchmark-gate-f73289afdee6460ebb8f3d4355c5d078` |

A1/A2 used `adapters.rs` SHA-256
`764df53ceb19350702fae75d3eb665b5c2b02902ce430ead94477e8d88f7e8f0` and
byte-identical `_core.abi3.so` SHA-256
`79392451ead50be774794c3c3166e6459eeea22629ec2f0ea3e30101756fd89d`. B1/B2
used `adapters.rs` SHA-256
`4d2a1305c29dbfcf17834fe81b47daf8c9518de96f9b0eb7c656c97dbcc8e461` and
byte-identical `_core.abi3.so` SHA-256
`4ffd74bd26d741056abead3fd42d6655b3b1cf393836c87b73ce31c4f4f874e9`. The
only source change between control and candidate was the bounded transpose
dispatch in `adapters.rs`.

Both candidate noise receipts prove actual SIMD, terminal completion, no
fallback, and 589,824 vector blur blocks with no scalar tail. The existing
transpose helper has no per-transpose receipt, so vector execution for those
two calls is established by the selected source branch and the existing
`transpose_native_tiled::<4, 4>` SIMD implementation, not by a separate
transpose counter. The whole-workflow timing includes the existing work and
scratch buffers and final materialization; candidate receipts report two host
buffers (6,291,456 bytes) and one 3,145,728-byte host allocation.

## Verification and limits

`make build-parity` succeeded for the control and candidate release builds.
The four focused benchmark legs each ran only the two listed workloads with
`MIGRATION_BENCHMARK_BACKENDS=simd`; every leg's integrated strict parity gate
passed 2/2. `scripts/validate_migration_parity_result.py benchmark` accepted
all four reports. No full test or parity campaign, coverage run, GPU profile,
Parallel CPU profile, or Pillow-SIMD comparison was run.

This is evidence for one large RGBA Gaussian workload on ARM64 only. Other
modes, sizes, x86, serial CPU, GPU latency/throughput, and Pillow-SIMD remain
open. Detailed reports, samples, compressed parity results, and attestation
sidecars are retained beside this README and covered by `SHA256SUMS.txt`.
