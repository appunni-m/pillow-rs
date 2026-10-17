# Sparse RGBA GaussianBlur CPU row specialization

This focused sequence uses `pipeline-chain.blur-guard.gaussian-rgba-sparse-512x512-radius-2` and exact parity case `PIL.Image.Image.filter.nuanced.performance-gaussian-rgba-sparse-512x512-radius-2`. The input is a 512×512 zero RGBA image with `[180, 120, 60, 128]` at `(2, 3)`, followed by `GaussianBlur(2)` and `tobytes`. The measured boundary is the public filter call plus result materialization; setup is excluded. All runs use warm state, 5 warmups, 20 iterations × 5 samples, concurrency one, Pillow 12.2.0, and macOS ARM64.

| Run | Pillow median | CPU median | Pillow / CPU | Exact parity |
| --- | ---: | ---: | ---: | --- |
| Baseline | 1.893938 ms | 2.5470625 ms | 0.743577× | Pass 1/1 |
| RGBA radius-one horizontal specialization | 1.8880625 ms | 1.0925 ms | 1.728204× | Pass 1/1 |
| Horizontal + vertical direct-tap specialization | 1.8543755 ms | 0.9949165 ms | 1.86385× | Pass 1/1 |

The candidate is 2.33× faster than the recorded baseline CPU median and is now faster than Pillow on this row. It remains below the 2× sequencing checkpoint and is evidence for this sparse radius-two workload only. The actual target receipt reports requested/actual CPU, terminal completion, no fallback, and `GaussianBlur path=cpu`. The focused Rust row-fusion test passed, including RGBA edge widths.

The source audit found the existing identity scan rejects this outlier after roughly 6.2 KiB, making the three full-frame vertical passes a more plausible residual cost. The accepted code path specializes only the three horizontal radius-one fractional RGBA passes; generic handling remains for other radii and short rows. Do not generalize this measurement to other image content, sizes, radii, modes, SIMD, Parallel CPU, or GPU.

## Vertical direct-tap follow-up

The follow-up keeps the horizontal specialization and uses five edge-clamped
source rows for the vertical radius-one fractional pass. Its focused Rust
contract matches the existing generic recurrence byte-for-byte for 2, 3, and 4
byte channel layouts, including one- and two-row images.

The same sparse workload and parity case passed again. CPU median was
0.9949165 ms versus Pillow at 1.8543755 ms (1.86385×); candidate p95 was
1.070833 ms, and concurrency-one reciprocal throughput was 1,005.109 ops/s.
The direct-tap follow-up lowered CPU median 8.91% versus the horizontal-only
candidate. The receipt confirms requested/actual CPU, terminal completion, no
fallback, and `GaussianBlur path=cpu`. Resource telemetry remains three host
buffers / 3,145,728 bytes, 2,097,152 peak live bytes, and two host allocations
/ 2,097,152 allocated bytes.

Run `migration-benchmark-aff7bfcc83984e3dbd277ab69c18ba4d`; parity gate
`migration-parity-benchmark-gate-4ad5935a097847759869aec924a3a74c`. Final
source `filter.rs` SHA-256 is
`c2a34f399e922faa6c2e8d728b031075ac51801eb4fc1c0085772e3a4875e056`; loaded
`_core.abi3.so` SHA-256 is
`0681c4d129533ec35a9f4e30d015f484ebe616ac46ead4a964abfbdd603a15bd`. This
single ARM64 sparse-image result remains below the 2× sequencing checkpoint.

Baseline run: `migration-benchmark-89e011b3e06f4e7c95fc55cbb882febb`; parity gate: `migration-parity-benchmark-gate-76137677e44f49448038b538e9428180`. Horizontal-only run: `migration-benchmark-3b490de44b2443929b269613afca8e1f`; parity gate: `migration-parity-benchmark-gate-10957fd90eeb473c9d981957cd436460`. Vertical follow-up: `migration-benchmark-aff7bfcc83984e3dbd277ab69c18ba4d`; parity gate: `migration-parity-benchmark-gate-4ad5935a097847759869aec924a3a74c`. All result schemas validated.

The baseline loaded `_core` SHA-256 `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749` with `filter.rs` SHA-256 `e224b1e8e3d9ac60df9512a3b011fa64a7fdb056f298651670e4861e2848da68`. The candidate loaded `_core` SHA-256 `c4851ac6ab715c6c2fc16ce1a0a72c1b1ae56bef2bd270897f1e7908a8cad907` with `filter.rs` SHA-256 `c146bc1e9ae482c77aba88080a0f619d4f8fd18078a7c61d0a388a6c0cc264d3`. Both report dirty source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`; source/build hashes distinguish the two states.

Parity result JSON is gzip-compressed. Decompress it before checking its report digest against the parity attestation. `SHA256SUMS.txt` covers every file in this directory except itself.
