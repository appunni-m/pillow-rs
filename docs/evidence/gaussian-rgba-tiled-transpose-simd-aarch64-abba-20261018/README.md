# AArch64 RGBA GaussianBlur SIMD tiled transpose — 2026-10-18

## Finding

For the large native RGBA radius-7.5 Gaussian SIMD path on AArch64, route its
two full-frame transposes through the existing 4×4 portable-SIMD tile
transpose. The dispatch requires the existing Gaussian RGBA kernel candidate,
native four-channel data, and dimensions at least 256×256. It is compiled only
for AArch64 with the default non-parallel feature set. BoxBlur, other modes,
small images, x86, and Parallel CPU keep their existing transpose paths. The
uniform-image identity return occurs before either transpose.

The source-matched A–B–B–A full-workflow cohort measured the noisy RGBA SIMD
median at 13.900 and 12.404 ms in controls, and 9.757 and 7.617 ms in candidate
legs. The candidate mean was 8.687 ms versus 13.152 ms for controls (1.51×, a
34.0% reduction). Both candidate medians were below both control medians, but
candidate B1/B2 varied noticeably. SIMD still missed regular Pillow in both
candidate legs: 9.757 ms vs 7.610 ms and 7.617 ms vs 6.677 ms (Pillow/SIMD
0.780× and 0.877×). This is partial progress; GaussianBlur remains open and
the SIMD target is not met.

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
- Each A–B–B–A leg selected and passed both exact parity cases (2/2).

Pillow/SIMD is the same-run Pillow median divided by the SIMD median; below 1
means SIMD is slower.

| Leg | Noise Pillow | Noise SIMD | Pillow/SIMD | Uniform Pillow | Uniform SIMD | Pillow/SIMD | Benchmark run | Parity gate |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| A1 control | 7.744 ms | 13.900 ms | 0.557× | 9.330 ms | 0.723 ms | 12.903× | `migration-benchmark-f15390b9b2e240118b8c2bcbd7376f76` | `migration-parity-benchmark-gate-0e53c619673845d1b23c875532c9c21d` |
| B1 candidate | 7.610 ms | 9.757 ms | 0.780× | 8.413 ms | 1.209 ms | 6.957× | `migration-benchmark-3ae3e2fc957741568a6b7021aa2c6ab4` | `migration-parity-benchmark-gate-e1fb1ec936264d19b7370af33c4f97d0` |
| B2 candidate | 6.677 ms | 7.617 ms | 0.877× | 6.444 ms | 0.591 ms | 10.908× | `migration-benchmark-42442c4e0ffa488ba6c90ea61ed1679f` | `migration-parity-benchmark-gate-4d00faf58b944cde94c7a8e9f317f794` |
| A2 control | 6.858 ms | 12.404 ms | 0.553× | 6.799 ms | 0.619 ms | 10.988× | `migration-benchmark-f3054f42c5074d38be2834b4df36bba2` | `migration-parity-benchmark-gate-243619a165ce4c7aadbe9ff7f1b29503` |

### One focused guard recheck

B1's uniform SIMD time was a high outlier even though that route returns before
the changed helper. A single fresh-parity recheck of the same two workloads
measured noisy RGBA at 7.334 ms vs Pillow 6.408 ms and uniform RGBA at 0.672 ms
vs Pillow 6.387 ms. The uniform result is close to the two controls (0.723 and
0.619 ms), so the 1.209 ms B1 guard value did not reproduce. Treat this as a
host-variance diagnostic, not another A–B–B–A leg. The recheck selected and
passed both exact parity cases; run `migration-benchmark-1ca09d89d53a41eb80c941467c29278e`,
parity gate `migration-parity-benchmark-gate-635e765a3c4940ec89e32b0a622c0474`.

The final control `adapters.rs` SHA-256 is
`764df53ceb19350702fae75d3eb665b5c2b02902ce430ead94477e8d88f7e8f0`, with
byte-identical `_core.abi3.so` SHA-256
`79392451ead50be774794c3c3166e6459eeea22629ec2f0ea3e30101756fd89d`. Both
candidate legs and the guard recheck used `adapters.rs` SHA-256
`c00f7e816b1dbcad995b530fad26824abf45c779f3e303e427bec8b2e9e73955` and
`_core.abi3.so` SHA-256
`37bff1bce65feaa0bacd62f550f142b04dd6259ea8cd9d7b2d59e5aa05cbd8fc`. The
only source change between control and candidate is the bounded AArch64-only
transpose dispatch.

Candidate noise receipts prove actual SIMD, terminal completion, no fallback,
and 589,824 vector blur blocks with no scalar tail. The transpose helper has no
per-call vector receipt; its invocation follows from the selected source
branch and existing `transpose_native_tiled::<4, 4>` implementation. Candidate
receipts report two host buffers (6,291,456 bytes) and one 3,145,728-byte host
allocation. The whole-workflow timer includes the buffers and final
materialization.

## Verification and limits

`make build-parity` succeeded for the control and AArch64 candidate release
builds. The A–B–B–A legs and guard recheck each ran only the two listed
workloads with `MIGRATION_BENCHMARK_BACKENDS=simd`; every fresh strict parity
gate passed 2/2. The five reports were accepted by
`scripts/validate_migration_parity_result.py benchmark`. No full test or parity
campaign, coverage, GPU profile, Parallel CPU profile, x86 Gaussian measurement,
or Pillow-SIMD comparison was run.

The current-source runtime change is limited to the AArch64 large RGBA
Gaussian SIMD path. Evidence covers one aligned 1024×768 shape; odd large
shapes, other modes/sizes, serial CPU, x86 performance, GPU latency/throughput,
and Pillow-SIMD remain open. An earlier portable-dispatch cohort is retained
at `gaussian-rgba-tiled-transpose-simd-abba-arm64-20261018/`; use this scoped
cohort for the current implementation. SHA-256 covers all retained artifacts
in this directory.
