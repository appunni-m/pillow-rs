# PutPixel batch family layout move — ARM64 — 2026-10-20

## Change

Moved the existing consecutive RGBA PutPixel batch from
`adapters/native_put_pixel.rs` into `adapters/put_pixel.rs`. The family adapter
retains mode and bounds admission, the single frame copy, result materialization,
and operation receipts. The unchanged ordered masked-store loop lives in
`adapters/put_pixel/kernels/rgba_batch.rs`, reached through the family's narrow
`kernels.rs` entry point. No generic or dynamic dispatch was added to the pixel
path.

## Frozen public workloads and parity cases

Both selected workloads use the warm `whole_workflow` boundary with five
warmups, 20 iterations × five samples, and concurrency one. Each contains its
own exact Pillow preflight, and both report 2/2 selected cases passed:

| Workload | Parity case |
| --- | --- |
| `pipeline-chain.getchannel.seeded-corners-1024x768-all-bands` | `PIL.Image.Image.getchannel.nuanced.pipeline-seeded-corners-1024x768-all-bands` |
| `pipeline-chain.getchannel.seeded-corners-16x16-all-bands` | `PIL.Image.Image.getchannel.nuanced.pipeline-seeded-corners-16x16-all-bands` |

Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7 ARM64, Pillow 12.2.0, CPython
3.12.13. The benchmark reports actual SIMD, terminal completion, zero
fallbacks, and the final `ExtractBand` path as `neon-deinterleave` (49,152
blocks for 1024×768 and 16 blocks for 16×16). The full workflow records one
output allocation and no full-frame copies. Its final-operation receipt does
not retain the earlier PutPixel fusion receipt; the focused Rust test below
verifies four ordered vector stores and fused-operation accounting directly.

## Full-call results

| Workload | Leg | Pillow median / p95 (ms) | SIMD median / p95 (ms) | Pillow ÷ SIMD |
| --- | --- | ---: | ---: | ---: |
| 1024×768 | A1 control | 1.784605 / 2.532541 | 1.184938 / 1.907041 | 1.506× |
| 1024×768 | B1 moved family | 1.570376 / 2.606750 | 0.957813 / 1.773583 | 1.640× |
| 16×16 guard | A1 control | 0.115459 / 0.305125 | 0.118729 / 0.186167 | 0.972× |
| 16×16 guard | B1 moved family | 0.113688 / 0.185291 | 0.115605 / 0.171916 | 0.983× |

This one A1/B1 pair does not establish a performance gain: Pillow timing moved
between legs too. It found no end-to-end latency regression on either selected
workload. The SIMD target remains far below the 5× Pillow goal.

## Verification and identities

- `cargo test -p pillow-rs --release putpixel`: 9 passed. This includes the
  direct `SimdPool` batch receipt test, which checks four vector PutPixel
  receipts, ordered output equality, two host-buffer boundaries, and four
  fused operations.
- `make build-parity`: succeeded for A1 and B1.
- `scripts/validate_migration_parity_result.py benchmark`: passed for both
  reports.
- No full benchmark, parity campaign, or coverage ran.

| Leg | Benchmark run | Parity run | Loaded `_core.abi3.so` SHA-256 |
| --- | --- | --- | --- |
| A1 control | `migration-benchmark-17e4f25b3b6c45f99f60496652d932a9` | `migration-parity-benchmark-gate-e7d7348abc8547778affdf54ce83a716` | `fd6eb29222650095c88665ffe59d198fea8ebfea3eeff868cf090d805ecca0ce` |
| B1 moved family | `migration-benchmark-3647c1512c8a46998ebb3554556b0a82` | `migration-parity-benchmark-gate-bcd2cbed86d84410b9bf796f038a2d59` | `db0d0886f279c4134b306b93d8322fcbfa2fc0a0afee3308989f28645e9a1127` |

The candidate module and dispatch hashes are recorded here for source review:

```text
adapters.rs                                  9ba9fd26cae9a3b19d562f29a1e62b44ee6b39000adf0d36df7b67c9411eeaf3
adapters/put_pixel.rs                       71f00301d40e6325436c61f8b56131e8c86b1c0d49891427f09ca406b3385966
adapters/put_pixel/kernels.rs               1406970387ced8a14b87149c4d473cce826d097ffd13d0421b9e152f5c624822
adapters/put_pixel/kernels/rgba_batch.rs     37d35cd20df6299ed8ba6d7ffeedd340369569557e1e35ab86fd88d9d30ea337
```

The A1 control used `adapters.rs` SHA-256
`629838722aed63935bf1d80daf25c69a87a96dd7c3bf3882432aa508ccc446e0` and
`adapters/native_put_pixel.rs` SHA-256
`8021c36ddd1dac2ce5827b65b9db3c224554159e2a13a8538f0f283b73f82753`.

Full reports, parity output, execution and producer attestations, and raw
samples are retained in `A1-control/` and `B1-candidate/`. The source
crosswalk maps the public GetChannel route but does not yet trace the internal
composed-pipeline batch dispatcher to this helper; it remains an unmapped
review candidate, not a dead-code finding. Kata: `pillow-rs#rxcj`.
