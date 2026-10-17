# Candidate r9: rolling integer rows for large RGBA Filter5x5

This dirty-tree x86_64 diagnostic targets the nonuniform path after the
existing uniform-identity predicate misses. It is limited to native RGBA,
explicit mode `None`/`RGBA`, the all-ones 5×5 kernel with scale 25 and offset
0, images above the small-uniform threshold, and serial CPU builds. The SIMD
adapter and Parallel CPU path are separate and were not changed or measured.

The implementation stores exact horizontal channel sums for five source rows
in a ring, updates each row with one leaving and one entering sample, then
sums those rows vertically. For a byte sum from 0 through 6,375, division by
25 cannot land on a half-integer; the minimum distance to a byte-rounding
boundary is 0.02. The normalized-f32 accumulation error is below 0.001, so
`(sum + 12) / 25` yields the same rounded byte. The caller retains the two-pixel
copied border. The existing content scan and identity shortcut still run
first; a miss reaches this rolling-row path.

The changed source is
[`filter.rs`](../../../../pillow-rs/src/compute/pool_cpu/ops/filter.rs),
SHA-256 `dc6297e356471217fdb37b8b52b6a43c99f7b47640eceb2fb95ac7775add8dbf`.
The candidate was built with `PATH=/home/appunni/.cargo/bin:$PATH make
build-parity` on the existing native runner; the loaded extension SHA-256 was
`6282923bed4284d8118f4d52260bf036420e485b1791e1b6cb3908886a293d7e`. Both
benchmark reports identify dirty source revision
`f2b63f559e91022a85cb43f7264bdf8b586a448d`. The focused unit test
`uniform5x5_native_rgba_tests::rolling_integer_rows_match_ordered_float_for_odd_width_and_copied_borders`
passed locally.

## Frozen workflows and results

Both workflows measure warm `whole_workflow` latency with five warmups and
20×5 observations at concurrency one. The public call includes image setup,
Filter5x5, and terminal `tobytes` materialization. Exact Pillow 12.2.0 parity
passed for both selected cases (2/2) in B1. B2 reused the same parity gate
because source, extension, manifest, input, and oracle identities matched.

| Workload | Run | Pillow median | CPU median | CPU/Pillow | Pillow throughput | CPU throughput | CPU P95 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `pipeline-chain.convolution.native.rgba-5x5-uniform.1024x768` | B1 | 46.446 ms | 5.330 ms | 8.71× | 21.530 ops/s | 187.615 ops/s | 5.796 ms |
| same | B2 | 46.664 ms | 5.256 ms | 8.88× | 21.430 ops/s | 190.273 ops/s | 5.779 ms |
| `pipeline-chain.convolution.native.rgba-5x5-nearuniform-tail.1024x768` | B1 | 46.826 ms | 14.388 ms | 3.25× | 21.356 ops/s | 69.503 ops/s | 15.980 ms |
| same | B2 | 48.206 ms | 14.470 ms | 3.33× | 20.745 ops/s | 69.111 ops/s | 15.136 ms |

The nearuniform input differs at the final channel of the final pixel, forcing
the identity scan to miss. Its candidate CPU median is about 12.3× below the
r8 miss median of 177.660 ms, though this is not an interleaved control and
must remain a diagnostic comparison. The uniform B1/B2 CPU medians are stable
around 5.3 ms. These results do not close Filter5x5 across modes, sizes,
kernels, or backends.

Execution receipts for every CPU subject show requested and actual `cpu`, one
`Filter5x5`, completed terminal materialization, no fallback, and zero SIMD
vector blocks. The workflow timing includes the output copy and five
`Vec<u16>` scratch rows (40,960 bytes at width 1024). The receipt's
`host_allocation_count` and `host_allocated_bytes` fields both report zero even
though these allocations occur; the host-buffer totals are 6,291,456 bytes for
the uniform case and 9,437,184 bytes for the nearuniform case. Treat the
allocation counters as incomplete and use the end-to-end latency, not those
zero fields, for comparison.

## Evidence

- B1: `migration-benchmark-946117afb0f44400ad06691d0d232f79`.
- B2: `migration-benchmark-1083c1245ff94998a41f4d7694ce162f`.
- Exact parity gate: `migration-parity-benchmark-gate-62dd6b9a50e24327af5d764ef9bfca65`.
- B1 report SHA-256: `8b384f6a891ae61442194411461469692d89dd0be574f5b33a73d25afba41d66`.
- B2 report SHA-256: `0fba3294943f9a7056751fde7312e4bcf924fbd96ad764c4250681a5160c1e58`.
- B1 compressed parity SHA-256: `352bda66ba0b88cd96ac5b6d8b27a939f3de646ef60ce6817d48e849124c7279`.
- B1/B2 execution-attestation SHA-256: `a342875f3f6e4ec24ae574755a111c3cc7e86f1e68b9a63378ca2b7ea8c2cc8e` / `8bdfcfcc0554871049a1f40794562a33542d4518f38829eb958c289441dd097c`.
- B1/B2 raw-sample SHA-256: `fb778976b92bc93a458b10be4b013d85111703268059b32557858c4eec195bfb` / `f26303deb2c90234ea3191ab21b667cb49cf56180b2cfd1b816e42b1b61eab75`.

The x86 checkout was not cleaned, and no target directory or full corpus was
transferred. No full campaign, coverage, commit, or push was run.
