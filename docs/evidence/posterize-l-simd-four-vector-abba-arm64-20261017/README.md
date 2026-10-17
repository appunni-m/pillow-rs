# Posterize L SIMD four-vector A/B/B/A — ARM64

This experiment evaluated an L-mode-only unroll that handled four 16-byte
vectors per loop iteration. It left the RGB and RGBA paths unchanged. The
candidate was reverted after the A/B/B/A run; the final extension digest
matches both control runs.

## Workload and controls

- Workload: `pipeline-op.posterize.material-l-noise-1024x768`
- Parity cases:
  - `PIL.ImageOps.posterize.nuanced.performance-large-l-noise-1024x768`
  - `PIL.ImageOps.posterize.nuanced.gpu-native-byte-tail-l-3x1-bits-4`
- Mode/size: L, 1024×768, bits=4, noise input
- Boundary: `call;observe-result`; warm cache; 5 warmups; 20 iterations × 5
  samples; concurrency 1; 100 timed samples per subject
- `observe-result` calls `Image.tobytes()` on the returned image, so the timed
  boundary includes the full public operation and output-byte materialization
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7, ARM64; Pillow 12.2.0
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty)
- Control core SHA-256: `af985e7b98977920ed0ec21ba59092b9ee7e4b7e0d4a241cab0703ec9dd874bb`
- Candidate core SHA-256: `f5fb44cee4a1a21afca95cf1c46df09753b2e3ca5111e8de70243b0557acb341`

| Run | Pillow median | SIMD median | Pillow / SIMD | Parity |
| --- | ---: | ---: | ---: | --- |
| A1 control | 0.338146 ms | 0.0754375 ms | 4.482× | main and tail pass |
| B1 candidate | 0.3760205 ms | 0.0787495 ms | 4.775× | 2/2 pass |
| B2 candidate | 0.3726045 ms | 0.0789585 ms | 4.719× | reused B1's exact parity proof |
| A2 control | 0.3561045 ms | 0.076875 ms | 4.632× | 2/2 pass |

The candidate's mean SIMD median was 0.078854 ms, 3.54% slower than the mean
of the two control medians (0.076156 ms). Both candidate runs were slower than
both control runs. Pillow timing varied across runs, so the higher B-run ratios
do not show a SIMD gain. Reject the unroll and keep the baseline implementation.

All four target receipts requested and executed SIMD, completed the terminal
step, and reported no fallback. Each reported the Posterize vector path with
49,152 vector blocks and zero scalar-tail pixels. The unchanged control core
digest in A1 and A2 confirms the candidate code was removed before the final
build. No broader operation target is claimed from this one workload.

## Evidence

Each run has its benchmark JSON, attestation, raw samples, parity proof, and
producer bundle in this directory. Run IDs:

- A1: `migration-benchmark-6253c56653504274892fc72a954dac41`
- B1: `migration-benchmark-1337911280554eb69dc373c4caab08a3`
- B2: `migration-benchmark-603a4cd68d14468999cebd7c1d86a392`
- A2: `migration-benchmark-4a5e7e7668d642a696b940ae8781be23`

The exact boundary, operation, mode, and input were held constant. B2 reused
B1's parity output because the input contract, producer, oracle, profile, and
variant build were unchanged. A2 ran parity against its rebuilt baseline
extension. The local-run ledger records Pillow and SIMD subjects separately.
