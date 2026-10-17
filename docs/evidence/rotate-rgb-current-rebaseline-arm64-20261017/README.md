# Rotate RGB current-source rebaseline — ARM64, 2026-10-17

This rebaseline checks the existing serial RGB contiguous-copy path on one
current-source Rotate workload. It confirms a greater-than-2× result for this
specific case; it does not complete `PIL.Image.Image.rotate` across its modes,
angles, sizes, and backends.

## Frozen workload and measurement

- Workload: `pipeline-op.rotate.material-rgb-nearest-noise-recovered-20261016-1024x768`
- Parity case: `PIL.Image.Image.rotate.nuanced.rgb-nearest-1deg-noise-recovered-20261016-1024x768`
- Input: RGB noise, 1024×768, nearest 1° Rotate without expansion; asset SHA-256
  `8443b82a55b2f7f0c748b5a6572469e5232ef13b27c407a7781d55869e5d845d`.
- Boundary: observed `call` + `observe-result` (`tobytes`). Image setup is
  outside the timed boundary; Rotate and final byte materialization are timed
  for both Pillow and pillow-rs.
- Profile: serial CPU only. SIMD and GPU were not selected or measured.
- Policy: warm release; 5 warmups; 20 iterations × 5 samples (100 samples per
  subject); concurrency 1.
- Host: macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0.
- Benchmark: `migration-benchmark-5d3dc0b75abb4e7fb273df95f09c2ccc`.
- Exact parity: `migration-parity-benchmark-gate-eadc0ed04fd7464f8c7760ea2f707b64`
  passed 1/1.

| Subject | Median latency | P95 latency | Median throughput |
| --- | ---: | ---: | ---: |
| Pillow | 1.315063 ms | 2.578708 ms | 760.420 ops/s |
| Serial CPU | 0.496688 ms | 0.733834 ms | 2,013.342 ops/s |

The measured Pillow/CPU latency ratio is **2.648×**. The target execution
receipt confirms actual CPU execution, one completed `Rotate`, terminal
completion, and no fallback.

## Identity and parity reuse

- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.
- `geometry.rs` SHA-256:
  `f6b2603ec1c712f2274d7b02c7e162a30d7cc33d63cb50d7b439a108796721d8`.
- Loaded `_core.abi3.so` SHA-256:
  `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Stable target runtime SHA-256:
  `ebddb4a466d407c0ce5b7acb31d5b3ce4ad18c93ce4072eab55015ffb2a6702a`.
- Manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`.
- Benchmark input SHA-256:
  `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`.
- Parity input SHA-256:
  `c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`.

An initial attempt to reuse the larger Rotate B1 parity report was rejected
before timing because its target runtime fingerprint differed. Those partial
preflight artifacts remain at this directory's top level for audit and are not
used as this result. The valid run under `fresh-runtime/` generated a fresh
parity result for the single selected case on the current runtime.

## Artifacts

`fresh-runtime/` contains the benchmark and parity reports, raw samples,
execution and producer attestations, and their child receipts. The benchmark
schema validator passed. `SHA256SUMS.txt` covers the complete dossier,
including the rejected preflight attempt.
