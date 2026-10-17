# MedianFilter LA 3×3 SIMD selection network — ARM64 A/B/B/A

This trial extended the existing 19-comparator median-of-nine SIMD network
from L to LA. It changed only the LA branch in the SIMD adapter; the LA window
gather, per-channel order, replicated borders, output layout and materialization
stayed the same. L and PA dispatch were unchanged. The candidate remains in the
working tree after a repeatable full-call median gain.

## Frozen workload and guards

- Workload: `pipeline-op.medianfilter.material-la-noise-1024x768`
- Main parity case:
  `PIL.ImageFilter.MedianFilter.nuanced.performance-material-la-noise-1024x768-size-3`
- Edge parity guards:
  - `PIL.ImageFilter.MedianFilter.nuanced.backend-noise-la-1x1-size-3`
  - `PIL.ImageFilter.MedianFilter.nuanced.backend-noise-la-1x3-size-3`
  - `PIL.ImageFilter.MedianFilter.nuanced.backend-noise-la-33x35-size-3`
- Input: LA noise, 1024×768, MedianFilter size 3 (rank 4)
- Timed boundary: `apply-filter;observe-filter-result`; the final step calls
  `Image.tobytes()`, so output byte materialization is timed
- Policy: warm cache, 5 warmups, 20 iterations × 5 samples, concurrency 1
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7, ARM64; Pillow 12.2.0
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty)

| Run | Pillow median | Serial CPU median | SIMD median | Pillow / SIMD |
| --- | ---: | ---: | ---: | ---: |
| A1 control | 85.828417 ms | 10.553396 ms | 15.804250 ms | 5.431× |
| B1 candidate | 88.140458 ms | 10.960480 ms | 12.680229 ms | 6.951× |
| B2 candidate | 87.208771 ms | 10.920771 ms | 12.699375 ms | 6.867× |
| A2 control | 97.385312 ms | 10.669250 ms | 15.967083 ms | 6.099× |

The candidate mean SIMD median was 12.689802 ms against 15.885666 ms for the
controls, a 20.12% reduction. Its mean p95 was 17.454521 ms against 18.229167
ms for the controls; individual tails varied, and the A2 Pillow p95 was noisy.
The candidate's SIMD receipts all prove requested=actual SIMD, vector path,
98,304 vector blocks, zero scalar-tail pixels, terminal completion, and no
fallback. Main parity passed for CPU and SIMD on A1, B1 and A2; B2 reused B1's
identity-matched proof. The three LA edge guards passed 3/3 on A1, B1 and A2.

The CPU rows remain 7.99×–9.13× faster than Pillow, though the B-run CPU
medians average 3.1% above the controls. The change is restricted to the SIMD
adapter, so treat that small cross-profile shift as a measurement caveat rather
than a CPU speedup claim. No CPU implementation changed. The local ARM64 SIMD
rows clear 5× Pillow, but this does not update the clean x86 matrix or prove a
current-source Pillow-SIMD comparison. The native x86 runner rejected SSH on
port 22 during this turn.

## Source and build identities

| Arm | Adapter SHA-256 | Loaded `_core` SHA-256 |
| --- | --- | --- |
| A1/A2 control | `96dcd31d15a047b09fe693d30407feada64be083de2186185e43a4eff5f19070` | `af985e7b98977920ed0ec21ba59092b9ee7e4b7e0d4a241cab0703ec9dd874bb` |
| B1/B2 candidate | `dea1423669102430dce5012010b813c8bc35bab9969d3029a9c16ab620fffcba` | `4e594a52264cda6166da3adf2dcb2241dcd9aec4e4ca3102d0e01a45d4204cf9` |

The median helper remains the existing fixed 19-comparator network. This trial
does not modify CPU, GPU, Parallel CPU, or PA paths and does not close
MedianFilter across modes, sizes, borders, or architectures.

## Run IDs

- A1: `migration-benchmark-34688ac940864e70a20911eec0ff6809`
- B1: `migration-benchmark-ac92710682214373a2f1bccf998d6b37`
- B2: `migration-benchmark-af7ea6b6616d4e84b14266e4cfb6785f`
- A2: `migration-benchmark-6e28d5d2cbe148bb917638ba7e0721e3`

Benchmark reports, attestations, parity outputs, producer bundles and raw
samples are stored in this directory. The local-run ledger has one Pillow,
CPU, and SIMD row for each arm.
