# GaussianBlur RGBA fractional-step specialization, serial CPU, ARM64

**Disposition:** rejected; the specialization was removed from the source tree.

## Frozen scope

- Workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- Mode and size: RGBA, 1024×768
- Profile: serial CPU; actual receipt `cpu`, terminal completed, no fallback
- Boundary: whole public workflow, including materialization
- Policy: warm; 1 warmup, 3 iterations × 2 samples (6 measured samples), concurrency 1
- Pillow oracle: 12.2.0 on CPython 3.12; target revision `418d5f0cde51ca64a97cbd007ea731da72d68abf` (dirty source)
- Both in-process parity gates passed 1/1. The large parity reports are not duplicated here; their SHA-256 values are recorded below and their bound parity attestations are retained.

## Result

| Variant | Pillow median | Rust CPU median | CPU / Pillow | CPU p95 | Exact parity |
| --- | ---: | ---: | ---: | ---: | --- |
| Pre-candidate source | 6.614708 ms | 7.527750 ms | 0.879× | 7.727625 ms | pass 1/1 |
| Four-lane RGBA update | 6.502354 ms | 11.298729 ms | 0.575× | 11.450125 ms | pass 1/1 |

The candidate was about 50.1% slower than the pre-candidate CPU result, while same-run Pillow medians were close. The added per-step specialization is therefore rejected. This workload's six-sample policy is intentionally small; the measured gap is large enough to stop this candidate, but these rows are not promoted as a stable performance baseline.

## Run identities and hashes

- Baseline run: `migration-benchmark-ab04038ccd31480ca35e02e3fa17e0fc`; parity gate `migration-parity-benchmark-gate-00ca1a00f33d42139c3edcac1e313753`
- Candidate run: `migration-benchmark-bc033da25075428193c176b914a1b95c`; parity gate `migration-parity-benchmark-gate-3c258e78f8f946a4b3647231b6120b54`
- Baseline benchmark SHA-256: `31842a42725f2a3ef703603eb2b1727c84ff20f57e9c68cf692291852dd518d0`
- Baseline raw samples SHA-256: `9804c8e928594bb0c5f551ce628c5e3ed8eecc3e6a13898b393ab6c97a9f8820`
- Baseline parity report SHA-256 (omitted because it is 16 MB): `3a7fdc78faec0c786df327bd00e7961cb440c735b6219c3ba5245b53aad8b70c`
- Candidate benchmark SHA-256: `17ffa24e7f9b29cc6f93a2af49a298b07949c71ec59f9976a44c2ff5d61553d2`
- Candidate raw samples SHA-256: `90dbb85469c3d2135cb078f36d9bd08ed0acb3269c4648b9220503445e7f2c7b`
- Candidate parity report SHA-256 (omitted because it is 16 MB): `12ee6920b8d44d56a837fa447dd7cee9f5f4ce21995d1525d35a2d7b5ddd9727`
- Candidate source was a temporary four-lane scalar update in `pool_cpu/ops/filter/box_blur/kernels/rgba.rs`; it was removed after the slowdown.

Benchmark reports, ordered raw samples, benchmark/producer attestations and parity attestations are stored under `baseline/` and `candidate/`.
