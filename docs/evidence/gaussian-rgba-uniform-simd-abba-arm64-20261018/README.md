# SIMD GaussianBlur RGBA uniform-image fast path — ARM64 — 2026-10-18

## Finding

A narrow three-pass RGBA GaussianBlur SIMD fast path now proves uniformity
for the exact radius 7.5 fixed-point kernel. It checks exact constant-byte
preservation, uses a small probe grid only to reject likely nonuniform
inputs, then scans the complete image with 16-byte SIMD comparisons before
returning a newly materialized copy. The path is restricted to native RGBA8,
GaussianBlur, three passes, radius 7.5, and images at least 256×256.

This is a retained partial gain for the uniform 1024×768 workload. The
nonuniform noise workload remains slower than Pillow, so Filter is still
open and this result does not close any full operation/backend target.

## Frozen cohort

- Uniform workload: pipeline-chain.blur-material.gaussian-rgba-1024x768
- Noise guard: pipeline-chain.blur-material.gaussian-rgba-noise-1024x768
- Exact cases: PIL.Image.Image.filter.nuanced.performance-material-rgba-gaussian-1024x768-radius-7-5 and PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5
- Boundary: whole_workflow, including setup, filter, and result materialization
- Policy: warm cache; one warmup; three iterations × two samples; concurrency one
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7 ARM64; CPython 3.12.13; Pillow 12.2.0
- Target profile: python-simd, release/default features
- All A/B/B/A legs used the same manifest and benchmark/parity input hashes.

Pillow/SIMD is same-run Pillow median divided by SIMD median. Values above
1.0 mean SIMD is faster.

| Leg | Workload | Pillow median | SIMD median | Pillow/SIMD | Benchmark run | Parity run |
| --- | --- | ---: | ---: | ---: | --- | --- |
| A1 | pipeline-chain.blur-material.gaussian-rgba-1024x768 | 7.352 ms | 13.108 ms | 0.561× | migration-benchmark-1bc98dc62bc94ffbab7daa91130aa385 | migration-parity-benchmark-gate-c955860c8f4b4e0eb362198109809706 |
| A1 | pipeline-chain.blur-material.gaussian-rgba-noise-1024x768 | 7.191 ms | 12.940 ms | 0.556× | migration-benchmark-1bc98dc62bc94ffbab7daa91130aa385 | migration-parity-benchmark-gate-c955860c8f4b4e0eb362198109809706 |
| B1 | pipeline-chain.blur-material.gaussian-rgba-1024x768 | 7.421 ms | 0.677 ms | 10.956× | migration-benchmark-e244229ae4e14b26aa893590efd5af7a | migration-parity-benchmark-gate-980e76f716cf4640b36531cc3bbd079d |
| B1 | pipeline-chain.blur-material.gaussian-rgba-noise-1024x768 | 8.175 ms | 13.980 ms | 0.585× | migration-benchmark-e244229ae4e14b26aa893590efd5af7a | migration-parity-benchmark-gate-980e76f716cf4640b36531cc3bbd079d |
| B2 | pipeline-chain.blur-material.gaussian-rgba-1024x768 | 7.479 ms | 0.653 ms | 11.462× | migration-benchmark-501586801881459486a55868ffba886a | migration-parity-benchmark-gate-ca81926a577d4a9d8e5e39eec778da5e |
| B2 | pipeline-chain.blur-material.gaussian-rgba-noise-1024x768 | 7.075 ms | 12.546 ms | 0.564× | migration-benchmark-501586801881459486a55868ffba886a | migration-parity-benchmark-gate-ca81926a577d4a9d8e5e39eec778da5e |
| A2 | pipeline-chain.blur-material.gaussian-rgba-1024x768 | 7.418 ms | 12.450 ms | 0.596× | migration-benchmark-65253a7d1d224f1e91b040662547e05e | migration-parity-benchmark-gate-c151805faefa4db8b8518f5017c4a326 |
| A2 | pipeline-chain.blur-material.gaussian-rgba-noise-1024x768 | 7.006 ms | 12.778 ms | 0.548× | migration-benchmark-65253a7d1d224f1e91b040662547e05e | migration-parity-benchmark-gate-c151805faefa4db8b8518f5017c4a326 |

The uniform candidate legs are 10.96× and 11.46× faster than same-run Pillow;
the controls are 0.56× and 0.60×. The noise candidate remains 1.71× and 1.77×
slower than Pillow, essentially the existing gap. Candidate noise medians were
12.55–13.98 ms versus control medians of 12.78–12.94 ms; same-run Pillow ratios
do not show a repeatable slowdown. Host variance makes small noise-path overhead
inconclusive, so no broader no-regression claim is made.

A1/A2 loaded the same control _core.abi3.so SHA-256
2ec48c4ab1b27c809a839e01fd720a298242eafb747ed45563c07e5343ecff7b.
B1/B2 loaded the same candidate _core.abi3.so SHA-256
79392451ead50be774794c3c3166e6459eeea22629ec2f0ea3e30101756fd89d.
The control adapters.rs SHA-256 is c0dc80396e0f0a836be7cd7822555e805f6d4a9837784014416ba6545744a975;
candidate adapters.rs is 764df53ceb19350702fae75d3eb665b5c2b02902ce430ead94477e8d88f7e8f0;
the benchmark-built gaussian_rgba_uniform.rs was
b806cc076417e6a74c0336d1b31f406984ad2ba9f180b8fc8df2d59b29eef002. After the
cohort, only a cfg(test) threshold-boundary case was added; the current module
hash is bf7e45fa56ebab76b6263f97463722088a527529b2d5fed7a8a7947076840689 and
the production implementation is unchanged. The working extension still has
the B1/B2 SHA-256.

Each parity leg passed both selected cases (2/2; zero failures). Candidate
receipts report actual SIMD, terminal completion, and no fallback. The uniform
candidate reports native-copy and 393,216 vector blocks: 196,608 for the full
16-byte SIMD uniformity scan and 196,608 for the materializing copy. The
nonuniform guard retained the normal vector blur path with 589,824 blocks.
The candidate uniform receipt also reports two host buffers totaling 6,291,456
bytes and 6,291,456 peak live host bytes. Its separate allocation-event fields
are zero, even though native_copy_image_bytes creates the output Vec. The
whole-workflow timing includes that allocation and final materialization; do not
read the zero allocation-event count as zero allocation. This distinction is
being audited in Kata pillow-rs#w00k.

Focused module tests passed 4/4, including a changed pixel outside the probe
grid, fixed-point identity rejection, and just-below/at the 256×256 threshold.
Candidate and control make build-parity builds succeeded with the existing
seven PyO3 missing-documentation warnings. No full parity campaign or coverage
was run.

The raw reports, timing samples, execution/producer attestations, attestation
children, and gzip-compressed parity reports are preserved here. SHA256SUMS.txt
covers every retained artifact.

Kata: pillow-rs#ttfz; read-only audits: pillow-rs#wz8p and pillow-rs#w00k.
