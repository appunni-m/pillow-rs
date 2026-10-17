# RGBA nearest Rotate zero-fill store skip — rejected, ARM64 2026-10-17

## Decision

The serial CPU candidate skipped writing transparent-black pixels to the RGBA out-of-bounds spans because `rotate_arbitrary_generic` allocates the output with `CheckedDims::alloc_buffer`, which zero-fills it. Exact parity passed, but the candidate did not demonstrate a repeatable public-call latency reduction against the source-matched controls. The candidate was reverted; the original source and loaded extension hashes are restored. Do not retain this change based on these runs.

## Frozen workload and identities

- Workload: `pipeline-chain.geometry-material.rotate-rgba-1024x768`; exact workflow case: `pipeline-chain.geometry.rotate-rgba-1024x768`.
- RGBA 1024×768, nearest Rotate at 37° with expanded output, solid input `[83, 149, 17, 191]`, default zero fill, warm `whole_workflow` boundary.
- 1 warmup, 3 iterations × 2 samples, concurrency 1; Pillow 12.2.0 / CPython 3.12; macOS 15.7.7 ARM64; serial CPU only.
- Dirty revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`; benchmark input SHA-256 `97a4ec9b8fa1704fb77518abae0dbb52e9afd4bf1e8fd459d40c64580fc74966`; manifest SHA-256 `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`.
- Baseline `geometry.rs` SHA-256 `f6b2603ec1c712f2274d7b02c7e162a30d7cc33d63cb50d7b439a108796721d8`, loaded `_core` SHA-256 `3a16bed165a0133179e8b9f594572520cf3489753b54c8c969e4315dffbf6d90`.
- Candidate `geometry.rs` SHA-256 `a266595c2b114283d2c6eb13b21dbfcd73f82d691e496d7df8adb6ad01b3c95c`, loaded `_core` SHA-256 `f1d7e9283baaf76097bd3e8d16788a65f8b197f081e2170c77f1d46af9090595`.

## A–B–B–A results

| Leg | Run | Pillow median | CPU median | Pillow / CPU |
| --- | --- | ---: | ---: | ---: |
| A1 baseline | `migration-benchmark-7d41dfb11c5742869d3977b3488e6727` | 2.675104 ms | 1.255000 ms | 2.132× |
| B1 candidate | `migration-benchmark-1855c8e371014c67b09beb242e979e19` | 2.713812 ms | 1.275146 ms | 2.128× |
| B2 candidate | `migration-benchmark-5d8374605c7b4fd4a7aaf70ded61a6de` | 2.565167 ms | 1.245063 ms | 2.060× |
| A2 baseline | `migration-benchmark-b7e20efb1b1644bb825740ca25dfffa0` | 3.006104 ms | 1.516813 ms | 1.982× |

Every target receipt reports actual CPU, terminal completion, no fallback, one Rotate operation, and no mode conversion. It records one 6,297,600-byte output allocation and 9,443,328 bytes across two host buffers. Both candidate legs exceed 2× Pillow on their same-run controls, but the unchanged baseline ranges from 2.132× to 1.982×. Candidate CPU medians (1.275 and 1.245 ms) do not improve over the adjacent A1 control (1.255 ms) by a repeatable amount, while the A2 control is a high-latency outlier. This sample contract cannot distinguish a candidate effect from host/timing variation.

## Exact parity

Candidate parity run `migration-parity-6efc38f4e1864249bb84046ffeeba65d` passed all 12 selected CPU cases. It includes the main workflow, zero-fill CMYK and RGBa cases, nonzero fill guards, and other nearest Rotate cases. The two additional baseline mode guards passed 2/2 after restoring the original source (`migration-parity-96d9e73947fe4aebbc82843c38a79314`). The original ten-case proof `migration-parity-0af970d305f24a7ca52f6282da6d4360` is reused for the unchanged baseline source/build/input/oracle/profile identities. The exact reports and report-bound attestations are included or referenced in this folder.

The benchmark result schema passed validation for all four legs; candidate and restored-source parity schemas also passed. No SIMD, Parallel CPU, or GPU run was justified because the edit touched only the serial CPU branch. No full test suite, full parity or benchmark campaign, or coverage campaign was run. No commit or push was made; the batch remains 0/33.

Verify copied artifacts from this directory with:

```sh
shasum -a 256 -c SHA256SUMS.txt
```
