# RGBA Transpose packed 4×4 serial CPU trial — x86_64, 2026-10-19

## Status

Measured partial median improvement; keep Transpose open. The candidate is a
serial CPU specialization for large, 4-byte-aligned RGBA `Rotate270` tiles. It
reduces the mean of the two A-run CPU medians by 2.44%, but the candidate p95
mean is 1.54% slower than the controls. It also remains slower than same-report
Pillow in both candidate runs. This evidence does not establish a stable
operation-wide win or close the CPU target.

## Frozen scope

- Workload: `pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized`
- Public operation: `PIL.Image.Image.transpose`, mode RGBA, 772×768.
- Timed boundary: `transpose-0;transpose-1;materialize`, including result
  materialization and the normal per-call output allocation.
- Backend: serial CPU, x86_64, release build, concurrency 1. Each leg selected
  and measured exactly one workload with 5 warmups and 20 iterations × 5
  samples (100 observations per subject).
- Host: AMD Ryzen 5 2600X, Linux x86_64 (`Linux-7.0.0-38-generic`, glibc 2.39),
  CPython 3.12.11, Pillow 12.2.0.
- Main parity case:
  `PIL.Image.Image.transpose.nuanced.tiled-aligned-rgba-772x768-methods-5-0`.
- Odd-size guard:
  `PIL.Image.Image.transpose.nuanced.tiled-odd-rgba-1025x1023-methods-5-0`.
- Focused Rust test:
  `transpose_tiled_rows_preserve_native_pixels_edges_and_trailing_storage`
  passed 1/1 (636 filtered).

No full benchmark/parity campaign or coverage ran. The four benchmark reports
passed `scripts/validate_migration_parity_result.py benchmark`. No workload,
threshold, or parity assertion changed.

## Candidate and scope limits

The change is in `pillow-rs/src/compute/pool_cpu/ops/geometry.rs` with the
kernel in
`pillow-rs/src/compute/pool_cpu/ops/geometry/kernels/transpose_rgba/x86_64.rs`.
It dispatches only for x86_64 serial CPU builds when the mode has four
channels, the method is `Rotate270`, the existing large-image tiling policy
admits the image, and both dimensions are divisible by four. The kernel moves
native RGBA pixels as packed 32-bit words through 4×4 blocks. Other modes,
methods, dimensions, architectures, and Parallel CPU builds retain the
existing loop. The SIMD adapter is not changed by this candidate.

The runner checkout was at revision
`418d5f0cde51ca64a97cbd007ea731da72d68abf` with unrelated dirty files present.
Only the candidate geometry change and its small kernel were toggled between
legs. Source SHA-256 values:

| Source | SHA-256 |
| --- | --- |
| Baseline `geometry.rs` from HEAD | `8e2852743becd04ad63e607a897c9fcfc56cfef4b4c27a0dafb580fd71f89500` |
| Candidate `geometry.rs` | `83e261db28927037b9cf461be6f1b911932bec9648b9567de5cabe2e07f38017` |
| Candidate `transpose_rgba/x86_64.rs` | `c21ee89acb167489d8b0b21823dfeb5c373c3bf91e3811581a72e2780a42a81c` |

## A–B–B–A results

Times are medians in milliseconds for the full materialized call. Ratios use
the Pillow result from the same report; a CPU/Pillow ratio above 1 is slower
than Pillow.

| Leg | Treatment | Pillow | CPU | CPU/Pillow | CPU p95 | Main parity |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| A1 | Baseline | 2.009802 | 2.949546 | 1.4676× | 3.182661 | Pass |
| B1 | Packed 4×4 candidate | 2.423924 | 2.846834 | 1.1745× | 3.284285 | Pass |
| B2 | Same candidate | 2.284039 | 2.861755 | 1.2529× | 3.103465 | Reused B1 proof |
| A2 | Baseline | 2.284630 | 2.902112 | 1.2703× | 3.108047 | Pass |

The baseline CPU median mean is 2.925829 ms; the candidate mean is 2.854294
ms, a 2.44% reduction. Both adjacent pairs favor the candidate by 3.48% and
1.39%. The control p95 mean is 3.145354 ms and the candidate p95 mean is
3.193875 ms, so this small median gain does not extend to the tail. Pillow
latency drifted between reports; use only the per-report ratios above for
Pillow comparisons. No confidence interval or statistical significance claim
is made.

## Execution and resource receipts

All four CPU subjects report `actual_backend=cpu`, a complete terminal receipt,
two transpose operations, and an empty fallback-reason map. Each retains the
same measured resources: one 2,371,584-byte host allocation; two host buffers
totaling 4,743,168 bytes; two fused operations; zero upload/readback bytes.
The selected call includes allocation and materialization, so the comparison
is end-to-end. The Pillow subjects are separate oracle processes in the same
report.

## Parity and runtime identity

The aligned main case passed on A1, B1, and A2. B2 reused B1's report-bound
proof because the candidate target runtime identity matched B1. The odd-size
guard passed on A1, B1, and A2; B2 reused B1's proof. The strict reuse check
rejected A1 parity for A2 after the rebuilt baseline's target runtime identity
changed, even though the baseline extension SHA was the same, so A2 received
fresh parity rather than bypassing identity validation.

| Leg | Benchmark run | Main parity run | Odd-size parity run | Target runtime identity |
| --- | --- | --- | --- | --- |
| A1 | `migration-benchmark-f000fff5a5764fe0ad0e23b3a63f48ff` | `migration-parity-benchmark-gate-5135830566034f12be42123830c0c908` | `migration-parity-98a2398420ce4e6c83bcdc1d032a1477` | `5d8957a05a90215cf40fd191e9b8e8f65e48ada1bf889ae6fe14935c8066025e` |
| B1 | `migration-benchmark-9930818b094e455492403fe8ab498b7c` | `migration-parity-benchmark-gate-4be1c026ae7a4c738f83dfe88bb1d46c` | `migration-parity-238ccb4dd9c4490ea07c18c1df1b5f58` | `05e3f63642df6e106b1cb458ab85cdded009ed1e741a1d691e89866e20afdf33` |
| B2 | `migration-benchmark-8fdbcca60088450184324e00d841c002` | Reused B1 | Reused B1 | `05e3f63642df6e106b1cb458ab85cdded009ed1e741a1d691e89866e20afdf33` |
| A2 | `migration-benchmark-3cbd91bad2644bc8812167d94a8ebdd5` | `migration-parity-benchmark-gate-f99dda41d3d64cf3a8d141f98be7e424` | `migration-parity-ab627c4289b246c99280fc752bda6990` | `27809b9d796b9d8861cffd05e7e9ffed66a02c641df8a47981260dec1e3313d4` |

The large full parity reports remain on the native runner and were not copied.
Their SHA-256 values are:

| Leg | Main parity report SHA-256 | Odd-size parity report SHA-256 |
| --- | --- | --- |
| A1 | `65a1d19e81cbc470c55b67251ea69132d59d19623c01f6e0d620e58bec797fac` | `43d00fc3ff4df6d949b43b4d17f82689391c9b15796c22270d3ad63d59ae6afe` |
| B1/B2 | `11aaa0fadbd264ae2367a1cc25b24caa2370e50568007525779b5a55de31510d` | `2608dbe5d714ba25f7fcf40cc1c2049b3f1aefdb68d9f0025126ada1530c5374` |
| A2 | `a7c3625a6d0bae49e37f35755bddc92f2d18aa2f3d44794add4682a962dd1db3` | `69f132e0b062405f28a47f79f51cec4e0a96d9d1719b6008e56a1d80177f0846` |

## Retained artifacts

Each leg directory retains its compact benchmark report, raw samples,
benchmark attestation, and producer attestation/bundle. A1, B1, and A2 also
retain main and odd-size parity attestations and odd-size producer receipts.
B2 has no new parity sidecar because it reused B1's proof. Full parity reports
(12.7 MB main and 22.4 MB odd-size per run) remain on the runner. SHA-256
checksums for all retained files are in `SHA256SUMS`.

The native runner was left intact. No target directory was copied; no runner
files were deleted.
