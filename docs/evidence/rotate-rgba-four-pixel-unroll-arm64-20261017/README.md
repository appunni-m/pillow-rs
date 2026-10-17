# RGBA nearest Rotate four-pixel unroll — retained partial gain — ARM64, 2026-10-17

Status: **retained as a measured partial improvement; Rotate remains open**.
The candidate improves this serial RGBA nearest workload, but the measured CPU
ratio remains below the 2× prioritization checkpoint. No SIMD or GPU result was
measured, and this does not close other Rotate modes, sizes, or backends.

## Frozen workload and parity

The selected workload was
`pipeline-chain.geometry-material.rotate-rgba-1024x768`: RGBA 1024×768,
nearest Rotate at 37° with expanded output, followed by terminal `tobytes`.
The timed `whole_workflow` creates the image with `Image.new`, runs Rotate, and
materializes the result with `tobytes`; only external fixture-asset preparation
is outside the timed boundary. This workload has no external assets. All four
A–B–B–A legs used the same warm macOS ARM64 host, Pillow 12.2.0 / CPython 3.12,
one warmup, 3 iterations × 2 samples, and concurrency one.

Exact parity selected these 10 cases:

- `PIL.Image.Image.rotate.nuanced.expanded-red-fill`
- `PIL.Image.Image.rotate.nuanced.modes-CMYK-2-arbitrary`
- `PIL.Image.Image.rotate.nuanced.modes-RGBA-0-expanded`
- `PIL.Image.Image.rotate.nuanced.modes-RGBa-2-arbitrary`
- `PIL.Image.Image.rotate.nuanced.rgb-nearest-1deg-noise-recovered-20261016-1024x768`
- `PIL.Image.Image.rotate.nuanced.rgba-90-non-expand-materialized`
- `PIL.Image.Image.rotate.nuanced.rgba-fractional-nearest`
- `PIL.Image.Image.rotate.nuanced.rgba-nearest-37deg-noise-fill-20261016-1024x768`
- `PIL.Image.Image.rotate.nuanced.rgba-nearest-expanded-fill-alpha`
- `pipeline-chain.geometry.rotate-rgba-1024x768`

The candidate exact parity passed 10/10 in B1 (`migration-parity-0af970d305f24a7ca52f6282da6d4360`). B2 reused that proof because its source, loaded extension, input, producer, oracle, and CPU profile identities matched. Baseline exact parity passed 10/10 in A2 (`migration-parity-b22ba04e200f4a9ca4fce4455e91eaf0`); A1 had the same baseline source and extension identities. The benchmark result's `successful_execution` gate is not used as a substitute for these separate exact-parity proofs.

The copied `*-parity.json` files with zero selected cases are empty preflight
reports, not parity evidence. The exact reports are the B1 and A2 files named
above; the B2 and A1 ledger rows reuse those matching proofs.

All CPU receipts show actual `cpu`, one Rotate, terminal completion, and no
fallback. The Pillow oracle receipts show actual `pillow`. The benchmark input
SHA-256 is
`5518e26b243dc3708095e09b8998cc6f5976f6c6cbc1b047529c5dc3d2e46350`; the
manifest SHA-256 is
`ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`.

## Source-matched A–B–B–A

| Leg | Benchmark run | Core SHA-256 | Pillow median (ms) | CPU median (ms) | Pillow / CPU |
|---|---|---|---:|---:|---:|
| A1 control | `migration-benchmark-d1f89a178f6747b59edc432118086145` | `bdafdc95d73debbd2cb9c8969bccdc2b079ede28bb1c8a0e2c1c6e2d80a62360` | 2.449500 | 1.442854 | 1.698× |
| B1 candidate | `migration-benchmark-10e80b1f675f4be7a352f31b02c3dcae` | `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90` | 2.512208 | 1.361042 | 1.846× |
| B2 candidate | `migration-benchmark-7b713fd72edc4a008325dbccd5435023` | `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90` | 2.368792 | 1.245626 | 1.902× |
| A2 control | `migration-benchmark-01ad5b637a8947f4afd4f3c9c9d06d0b` | `bdafdc95d73debbd2cb9c8969bccdc2b079ede28bb1c8a0e2c1c6e2d80a62360` | 2.378334 | 1.462375 | 1.626× |

Control CPU medians averaged 1.452615 ms; candidate medians averaged 1.303334
ms, a 10.28% latency reduction. Both candidate medians beat both controls.
Candidate CPU latency is at or below Pillow, but still misses the 2× target in
both candidate runs.

## Candidate and disposition

The serial RGBA nearest span now copies four source pixels per loop iteration.
Each lane still derives its input pixel from the same sequential signed
16.16 coordinate additions and `>> 16` truncation as the scalar path. Fill
pixels and the scalar remainder retain their prior handling. SIMD and parallel
paths are untouched.

Baseline `geometry.rs` SHA-256:
`24645acf442d8356fe021f2baf861dd34275ac3e85ffda6b2913600e624696d9`.
Candidate `geometry.rs` SHA-256:
`f6b2603ec1c712f2274d7b02c7e162a30d7cc33d63cb50d7b439a108796721d8`.
The final local `make build-parity` rebuilt the retained source; the loaded
`pillow_rs._core` SHA-256 is
`3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.

The source delta is included as
[`rotate-rgba-four-pixel-unroll.patch`](rotate-rgba-four-pixel-unroll.patch).
Completed reports, raw samples, attestations, exact parity proofs, producer
receipts, and execution receipts are kept in this directory. An incomplete
preliminary B2 producer bundle is not included; the final B2 result bundle is.

This gain remains candidate evidence for one ARM64 workload. It does not earn
operation-completion credit or close the 33-item commit batch. No coverage,
commit, or push ran.
