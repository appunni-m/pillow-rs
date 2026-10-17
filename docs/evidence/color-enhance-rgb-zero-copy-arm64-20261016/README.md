# RGB Color factor-zero copy shortcut — retained

This focused Apple ARM64 trial compares the current `Color.enhance(0)` path
with a narrow wrapper shortcut. The shortcut returns an independent copy of
the already-built RGB degenerate image, which is the first operand of
`Blend(degenerate, image, 0)`. It keeps the eager `Color` constructor and
routes every case outside the guarded native RGB contract through the existing
`operations.blend` path.

## Frozen workload and identity

- Workload: `pil-imageenhance-color.enhance.material-rgb-520x512-factor-zero`
- Case: `PIL.ImageEnhance.Color.enhance.nuanced.coverage-batch-enhance-large-color-rgb-0`
- Extra exact-parity guards: `PIL.ImageEnhance.Color.enhance.nuanced.audit-RGB-varied-0`
  and `PIL.ImageEnhance.Color.enhance.nuanced.audit-RGB-varied-0.3`
- Boundary: `whole_workflow` (`Color(image)`, `enhance(0)`, and byte
  materialization); warm cache; 5 warmups; 20 iterations × 5 samples; 100
  measured calls; concurrency 1.
- Host: macOS 15.7.7, ARM64, CPython 3.12.13, Pillow 12.2.0.
- The word `coverage` is part of an existing case ID. Only the three selected
  parity cases ran; no coverage lane ran.
- Manifest SHA-256:
  `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`
- Benchmark input SHA-256:
  `7541ecb21a0dd42a6fc9fefb0a101e36f327bf3e6210ae7020779a60a06632e1`
- Parity input SHA-256:
  `ca83c5e32faeabeb55b500c08c427df5bbeb392b5450e070317585bdc00b72a8`
- The compiled core extension was unchanged across all four phases. Its SHA-256
  is recorded in [source-hashes.txt](source-hashes.txt).

## A/B/B/A results

| Order | Variant | Pillow median | Serial CPU median | Pillow/CPU | CPU P95 | Parity gate |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| A1 | HEAD wrapper control | 0.324021 ms | 0.365271 ms | 0.887× | 0.448750 ms | `migration-parity-benchmark-gate-59ccaf7c160142978ba41195d546ea0c` |
| B1 | Factor-zero copy shortcut | 0.336479 ms | 0.140521 ms | 2.394× | 0.202958 ms | `migration-parity-benchmark-gate-11da23bf8b914e1b96c1a447e04520c9` |
| B2 | Same candidate | 0.335896 ms | 0.143938 ms | 2.334× | 0.177042 ms | `migration-parity-benchmark-gate-1c73840cd93b47e7afc7fcbff03ee646` |
| A2 | Restored HEAD wrapper | 0.360500 ms | 0.364584 ms | 0.989× | 0.400375 ms | `migration-parity-benchmark-gate-f73200e16ba540148104d21a59dfd521` |

The two candidate CPU medians average 0.142229 ms versus 0.364927 ms for the
bracketing controls, a 61.0% latency reduction. The paired Pillow medians stay
near 0.336 ms in B1/B2. Each candidate run is 2.33–2.39× faster than its
same-run Pillow result. The selected benchmark parity gate passed in every
phase, and the separate three-case CPU parity slice passed 3/3.

## Backend and behavior proof

The candidate benchmark receipts report `actual_backend=cpu`, terminal
materialization complete, and no fallback. This is serial CPU end-to-end
evidence. No SIMD or GPU profile ran, and no accelerated-backend credit is
claimed. The optimization removes the final Blend operation; it does not
replace the eager degenerate-image construction or claim a faster pixel
kernel.

Focused runtime checks confirmed that factor zero bypasses `operations.blend`,
nonzero factor 0.3 still calls it, and mutating the returned image does not
mutate the degenerate image. If the source changes mode or size after
construction, Pillow and pillow-rs both raise
`ValueError("images do not match")`.

The candidate is a local dirty-tree result on revision
`9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`; it is not a clean published
snapshot. The focused implementation and experiment are tracked in Kata issue
`pillow-rs#vrzg`. No commit or push was made; the 33-item batch is still in
progress.

## Run IDs and artifacts

- A1: `migration-benchmark-b5775221e5e64175a4b2086acbb3eb2e`
- B1: `migration-benchmark-f929a9a6a2964ee2b86eaac33637cda5`
- B2: `migration-benchmark-05e03f8f5149476e9970113e1f578dbc`
- A2: `migration-benchmark-274732fd1553439590f83a9bf4928102`

Each phase includes its benchmark report, timing samples, exact parity report,
execution attestation, producer attestation, and parity attestation as
gzip-compressed artifacts. The three-case guard report is also included.
Verify all artifact bytes with `shasum -a 256 -c SHA256SUMS.txt`.
