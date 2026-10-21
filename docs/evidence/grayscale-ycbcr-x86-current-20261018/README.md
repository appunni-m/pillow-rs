# Current-source YCbCr Grayscale rebaseline on native x86_64

This focused measurement revisits the 1024×768 materialized YCbCr
`ImageOps.grayscale` workload on the native x86_64 runner. It updates the local
result ledger with current-source observations; it does not attribute a gain to
a code change.

## Frozen scope

- Workload: `pipeline-op.grayscale.material-ycbcr-noise-1024x768`.
- Exact parity cases: `PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`
  and `PIL.ImageOps.grayscale.nuanced.performance-ycbcr-odd-byte-tail`.
- Input: materialized 1024×768 YCbCr noise image; asset SHA-256
  `8d09ced0ef27917e84162a9bc924b41913252d85adfb87a2464a307c2a8cd110`.
- Boundary: public call plus result observation (`tobytes()`), warm release,
  five warmups, 20 iterations × five samples, concurrency one.
- Host: native x86_64, AMD Ryzen 5 2600X, Linux; Pillow 12.2.0 / CPython 3.12.
- Target revision: dirty worktree at `418d5f0cde51ca64a97cbd007ea731da72d68abf`.
  The loaded `_core.abi3.so` SHA-256 is
  `2783466546095acdff1a353d9e60763524a3163cfba176268dc4003d55ccdbcf` for
  both target profiles.

## Results

Each row below compares subjects within its own report. CPU and SIMD were run
in separate reports; their Pillow medians differ, so the small CPU-versus-SIMD
difference is not a controlled profile comparison.

| Report | Pillow median | Target median | Pillow / target | Backend receipt |
| --- | ---: | ---: | ---: | --- |
| CPU `migration-benchmark-9967f3d9d19a404f8de21378ce6e53ae` | 1.160800 ms | 0.851483 ms | 1.363× | requested and actual CPU; complete; no fallback |
| SIMD `migration-benchmark-85852d42968942ca8a6db1af69305c76` | 1.1880045 ms | 0.855338 ms | 1.389× | actual SIMD, `ssse3-deinterleave`, 49,152 vector blocks, zero tail; complete; no fallback |

Both parity runs passed 2/2 selected cases. The SIMD receipt proves the x86
SSSE3 path actually executed. Both target profiles include one 786,432-byte
output allocation and one mode conversion. The SIMD result is faster than
regular Pillow, but it remains below the repository's 2× triage checkpoint
(and well below its 5× SIMD completion target).

The previously published x86 CPU slowdown does not reproduce in this
current-source observation. This is a rebaseline on a different source/build
and report identity, not proof that an implementation change fixed it. The
current x86 Pillow-SIMD environment does not have Pillow-SIMD installed, so no
fresh same-host Pillow-SIMD ratio is claimed; the published Pillow-SIMD JSON
remains historical ranking evidence only.

## Retained artifacts

The directory retains both benchmark reports, raw sample reports, execution
attestations and their child receipts, compressed exact-parity reports, and
parity attestations. The four benchmark subject rows are appended to
`../performance-optimization-local-runs.csv`. The reports share the input,
workload, manifest, and wrapper hashes recorded in their identity metadata.
All 20 retained report, raw-sample, parity, attestation, and child-receipt
artifacts were imported into the adjacent `pillow-perfdb` local DuckDB; the
committed evidence files and ledger remain the durable record.

The family path follows the repository standard in
`pillow-rs/src/compute/pool_simd/ops/adapters/grayscale.rs` and
`pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs`.
The x86 path delegates its vector work to
`pillow-rs/src/color.rs::ycbcr_luma_ssse3_bytes`; this measurement retained
that path unchanged.

**Next:** keep Grayscale in the under-2× SIMD queue, but first finish CPU
operations that remain slower than Pillow. Revisit a Grayscale kernel only
with a matched current-source candidate measurement and a usable same-host
Pillow-SIMD oracle. Kata tracking remains open under `pillow-rs#xjn8`.
