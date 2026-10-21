# AVX2 Transpose sparse-tail traversal — x86_64, 2026-10-19

## Status

The family move follows the repository path standard, and both aligned and
odd-size public parity guards pass. The direct edge traversal removes nearly
all scalar-region checks for the selected input. The measured full-call ratios
do not show a stable enough gain to claim a performance improvement, and SIMD
Transpose remains slower than Pillow. Keep the optimization open and do not
count this result toward the SIMD latency target.

## Family structure

```text
pool_simd/ops/adapters.rs
  -> adapters/transpose.rs                 # mode and orientation admission
     -> adapters/transpose/kernels.rs      # inline family kernel entry
        -> adapters/transpose/kernels/rgba_avx2.rs
```

The facade keeps selection and receipt attribution visible. The family adapter
selects RGBA and the supported transpose orientations; the AVX2 kernel retains
the 8×8 tiles, initialized output allocation, and direct backend path. Its
scalar edge traversal now visits the bottom incomplete rows, then the right
incomplete columns of full rows. It no longer scans the complete output and
branches once per pixel to skip the tiled interior.

For the selected 772×768 input, this changes the scalar traversal from 592,896
candidate-position checks to 3,072 actual edge-pixel copies (the bottom four
rows; height is tile-aligned). The odd-size guard exercises both edge loops and
their shared corner without overlap.

## Frozen workload and parity

- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`
- Main parity case: `PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`
- Odd-size parity guard: `PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0`
- Boundary: `transpose-0;transpose-1;materialize`
- Warm release; 5 warmups; 20 iterations × 5 samples; concurrency 1.
- Host: AMD Ryzen 5 2600X, Linux x86_64; CPython 3.12.11; Pillow 12.2.0.
- All selected parity runs passed. B2 reused B1's report-bound parity proof
  with unchanged source, extension, runtime, inputs, and profile identities.
- Full parity JSON reports (12.7 MB main, 22.4 MB odd) remain on the native
  runner at `/home/appunni/work/pillow-rs-current-local-x86-20261019/build/migration-parity/transpose-tail-sparse-20261019/`.
  Their report digests and identities are retained in the small local
  attestations. The reports were not copied into this checkout.

## End-to-end measurements

Latency is the median in milliseconds for the complete materialized public
pipeline. `SIMD/Pillow` above 1 means SIMD is slower. Each run has its own
same-report Pillow oracle measurement.

| Run | Treatment | Pillow | SIMD | SIMD/Pillow | Main parity |
| --- | --- | ---: | ---: | ---: | --- |
| A1 | Current-source control, previous kernel placement | 2.458304 | 2.782940 | 1.1321× | Pass |
| B1 | Family placement + sparse edge loops | 2.151482 | 2.606304 | 1.2114× | Pass |
| B2 | Same candidate binary; reused B1 parity proof | 2.439799 | 2.491364 | 1.0211× | Pass |

The candidate medians are lower in absolute time than A1, but the paired Pillow
time also varied substantially. B1's ratio is worse than A1 while B2's is
better; the two candidate ratios do not establish a repeatable gain. SIMD was
still slower than Pillow in both candidate runs, so the operation remains a
priority. No fallback or mode conversion occurred.

## Executed path and resources

Both candidate receipts report `actual_backend=simd`, no fallback, one SIMD
execution for the two-operation call, and the first operation path
`avx2-transpose` with 73,728 vector blocks and 12,288 scalar tail bytes. The
second operation remains `native-copy`. The call still makes one 2,371,584-byte
host allocation and materializes two host buffers totaling 4,743,168 bytes.
These counts match A1; no allocation, transfer, or materialization boundary
was removed from timing.

## Artifact identities

- A1 baseline extension SHA-256: `c80fbc6fed570bb2e61928ffe23fe73b8af0a0c052b8f0d96edc42c7543d0ff7`
- B1/B2 candidate extension SHA-256: `3df984038f8f5d2e320780f87c5ba87ab673f5ee4c241c59b69b14f05de72a4d`
- Main parity runs: A1 `migration-parity-benchmark-gate-9b3809d35d13457d83f489f496a9d09d`; B1 `migration-parity-benchmark-gate-724a4baf1bf44f3d8d93b702fb4506c3`; B2 reuses B1.
- Odd parity runs: A1 `migration-parity-55dc72435a57456fba0987f6f903b9f6`; B1 `migration-parity-e823013e598047c190a1025dc26e6e2b`.
- Benchmark runs: A1 `migration-benchmark-645e41240b264b0f8476c112c493d8ae`; B1 `migration-benchmark-d71adf01568143f6867dedc79fc28ff1`; B2 `migration-benchmark-71276f9ed97c4ce68102b04c304494f0`.
- Changed kernel SHA-256: `a66fb6b97212b0504bb355d2501c84113c1e787b08f58e0f28507722937bf3ff`.
- The runner's project venv lacks parity dependencies and Maturin; the runs
  used its existing managed Python (with the pinned Pillow and parity
  dependencies) and Maturin 1.13.3 with `--skip-install`. A matching
  `--locked` release build was used for A and B; the version warning is a
  reproducibility note, not a runtime dependency.

Compact benchmark reports, raw samples, and report-bound attestations are
stored beside this README. SHA-256 hashes for the local artifacts are in
`SHA256SUMS`.
