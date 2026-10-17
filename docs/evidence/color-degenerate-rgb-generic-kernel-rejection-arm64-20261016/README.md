# RGB Color degenerate generic-kernel trial — rejected

This is a focused local diagnostic for `PIL.ImageEnhance.Color.enhance` on
macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0. It is not a published
matrix replacement and receives no completed optimization credit.

## Frozen work

- Workload: `pil-imageenhance-color.enhance.material-rgb-520x512-factor-zero`
- Existing exact-parity case: `PIL.ImageEnhance.Color.enhance.nuanced.coverage-batch-enhance-large-color-rgb-0`
- Public call boundary: construct `Color(image)`, call `enhance(0.0)`, and
  materialize with `tobytes`; warm cache; 5 warmups; 20 iterations × 5
  samples; concurrency 1.
- The word `coverage` occurs in the pre-existing case ID. Only its single
  selected parity comparison was run for each benchmark cohort; no coverage
  lane ran.
- Benchmark input SHA-256:
  `7541ecb21a0dd42a6fc9fefb0a101e36f327bf3e6210ae7020779a60a06632e1`
- Parity input SHA-256:
  `ca83c5e32faeabeb55b500c08c427df5bbeb392b5450e070317585bdc00b72a8`
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`

## Results

| Order | Variant | Pillow ms | CPU ms | Pillow/CPU | Core extension SHA-256 |
| --- | --- | ---: | ---: | ---: | --- |
| A1 | Original RGB-to-L-to-RGB path | 0.328521 | 0.3608125 | 0.911× | `d9cb7176aadd058e07f3a3cb68b24a2e29360e08d782a9faa3aa81eff1c6b6b3` |
| B1 | RGB via `ColorSaturation(0)` | 0.342479 | 1.6924585 | 0.202× | `f1f7b203e23de17ffcae6cf35d295944c22a226e3025ac11340f45da17f1cd59` |
| B2 | Same candidate | 0.329646 | 0.9447915 | 0.349× | same B hash |
| A2 | Restored original path | 0.3468755 | 0.981667 | 0.353× | same A hash |

Each cohort passed its one selected exact Pillow parity comparison, used the
CPU profile, completed terminal materialization, and reported no fallback. All
four benchmark JSON files passed `validate_migration_parity_result.py`.

Run IDs:

- A1: `migration-benchmark-7fde093648234e1097a07f2c883d25af`
- B1: `migration-benchmark-f03d8efd7bd24b2fa069186a27e6efe3`
- B2: `migration-benchmark-54de8eb9b0924fdebcd075b47e4cda2a`
- A2: `migration-benchmark-394e50fb3cbc45d09e512ee6d18c0ce5`

Parity gate IDs:

- A1: `migration-parity-benchmark-gate-84f47737162242d182edf892f8a59c76`
- B1: `migration-parity-benchmark-gate-a8ea493d9a98457d8fc2173b3b7eacd9`
- B2: `migration-parity-benchmark-gate-7d77845d6a8041ce80700a38f33279f3`
- A2: `migration-parity-benchmark-gate-9eb97bfceadc4ddfa8ac158ff9d00081`

## Decision and proof limits

The generic `ColorSaturation(0)` RGB candidate is rejected. A1 and A2 controls
drift substantially, so this host bracket does not establish a causal speedup
or regression. No source optimization remains; `pillow-rs/src/ops/enhance.rs`
was restored, with SHA-256
`fc09b4b3967ce14f57462ce2fb59127665f1e2b3cb6f5493bbba9d76df32f18b`.

The benchmark receipt records `actual_backend=cpu`, complete materialization,
and no fallback, but operation telemetry names only setup `PutPixel`; it does
not name the eager Color construction/conversion work. Do not use these data as
SIMD/GPU execution evidence or claim a backend-specific Color kernel result.
A read-only follow-up in Kata issue `pillow-rs#vrzg` is checking whether eager
operation attribution is expected or a proof gap. The added material workload
is retained for future clean, identity-matched measurement.

No full campaign, coverage, unrelated tests, or remote-runner actions ran.
No commit or push was made; completed-item batch remains 0/33.

## Artifact checks

This directory contains gzip-compressed benchmark reports, exact parity
reports, execution and producer attestations, and raw timing sidecars. Verify
their bytes with `shasum -a 256 -c SHA256SUMS.txt`.
