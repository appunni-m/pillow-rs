# GetChannel RGBA PutPixel prefix batching — ARM64 — 2026-10-18

## Finding

The 1024×768 seeded-corners workflow starts with four adjacent RGBA
`PutPixel` operations, then converts to RGB and extracts all three channels.
Each unbatched SIMD PutPixel cloned the full input frame. The candidate admits
only exact-sized, nonempty RGBA byte planes with two or more in-bounds,
non-palette writes. It validates the whole run, copies the frame once, and
applies the existing ordered masked SIMD store for each write. Other modes,
layouts, empty inputs, single writes, and out-of-bounds runs retain their
existing path.

## Matched whole-workflow results

Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7, ARM64, Pillow 12.2.0 / CPython
3.12.13. Workload: `pipeline-chain.getchannel.seeded-corners-1024x768-all-bands`.
Boundary: `whole_workflow`; warm cache; 5 warmups; 20 iterations × 5 samples;
concurrency 1. Each listed run used a fresh exact parity preflight (1/1).

| Leg | Report run | Pillow median | SIMD median | SIMD / Pillow speedup |
| --- | --- | ---: | ---: | ---: |
| A1 control, reused current-source report | `migration-benchmark-fad4b8b94c96406e970c229d0f07713c` | 1.5061 ms | 1.6159 ms | 0.932× |
| B1 candidate | `migration-benchmark-ce1b526b8fd94f63ae9b5d165082b4aa` | 2.0725 ms | 1.4733 ms | 1.407× |
| B2 candidate | `migration-benchmark-10b5882e31624a8c80d6c35569bfbcd3` | 1.4191 ms | 0.9452 ms | 1.501× |
| A2 rebuilt control | `migration-benchmark-c375fc8403484d9594c47486a130a839` | 1.6937 ms | 1.9160 ms | 0.884× |

B1 and B2 both beat their matched Pillow run; both controls are slower than
Pillow. B2 and A2 show wide timing movement on both Pillow and the target, so
retain the raw samples and treat the ratio change as evidence of a useful
candidate, not as a precise universal gain. A1 is the pre-batching report from
the adjacent RGBA→RGB kernel experiment on the same host, source revision,
workload, and policy. Its report did not retain a loaded-extension digest; A2
is the freshly rebuilt, identity-recorded control.

The candidate's 16×16 sibling guard also passed exact parity (1/1): Pillow
0.1150 ms, SIMD 0.1184 ms (0.971×). This is near parity, not acceleration.

All candidate/control 1024×768 target runs completed on actual SIMD with no
fallback. The benchmark's final operation receipt shows the NEON ExtractBand
kernel (49,152 vector blocks), but does not preserve receipts from the earlier
parent materialization; its reported `fused_operation_count` is zero. Do not
interpret that as evidence the prefix batching did or did not execute. A focused
`SimdPool` test exercises the batch branch directly and requires four `PutPixel`
receipts, one vector block per write, four fused operations, two host-buffer
boundaries, one output frame, and byte equality with sequential SIMD writes.

## Verification and remaining scope

- `cargo test -p pillow-rs --release rgba_putpixel_batch`: 2 passed.
- `cargo test -p pillow-rs putpixel`: 9 passed, including the `SimdPool`
  receipt and allocation-boundary test.
- `make build-parity`: candidate and control release builds succeeded. The
  build emitted the existing seven PyO3 missing-documentation warnings.
- Benchmark and parity result schemas validated for B1, B2, A2, and the 16×16
  guard. No full parity campaign or coverage ran.
- Candidate extension SHA-256: `8de7dfa2f15a01f3d107e25f3b10226d51c56277597041c5ddc33679962fc826`.
  Rebuilt control extension SHA-256: `7f7fb8bce13d6b7c83c740a64deeaa3eee32a8dc8667f86c9f1d76dca37ff724`.
- Candidate source hashes: `pool_simd/mod.rs`
  `3f5a4ac950e23fb11105f4145eb6411cfd6beb6b78d7d7b6577d9c2bbe19d03d`,
  `adapters.rs`
  `c313e768a24c3cb0dd13b0616958cd4490c0defd144882b61208695c0ea42ac2`,
  and `native_put_pixel.rs`
  `8021c36ddd1dac2ce5827b65b9db3c224554159e2a13a8538f0f283b73f82753`.

The benchmark artifacts and raw samples remain in
`build/migration-parity/getchannel-putpixel-batch-20261018/`; the A1 report is
`build/migration-parity/getchannel-rgba-rgb-neon-B3-final-20261018.json`.
Ten Pillow/SIMD subject rows, including A1 and the small guard, are in
`docs/evidence/performance-optimization-local-runs.csv`.

This is partial ARM64 evidence only. The 1024×768 candidate is about 1.4–1.5×
Pillow, far below the project SIMD target; the 16×16 result is near parity.
x86, Pillow-SIMD, other sizes/modes, GPU latency and sustained throughput
remain open. Kata: `pillow-rs#jpj6`.
