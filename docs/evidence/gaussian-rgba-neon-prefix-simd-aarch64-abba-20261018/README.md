# AArch64 RGBA GaussianBlur SIMD 8-lane signed-prefix scan — 2026-10-18

## Finding

For RGBA radius 7.5 GaussianBlur on AArch64, replace each channel’s two four-lane u32 prefix scans with one eight-lane signed-i16 inclusive prefix scan in complete middle blocks. The byte delta is bounded by ±255, and every eight-lane prefix by ±2040, so signed 16-bit arithmetic is exact. The scan widens with sign extension before the existing u32 Q24 multiply, fractional taps, bias, shift, and byte narrowing. The final block accumulator adds the signed lane-7 prefix. Existing vld4/vst4 memory access, block admission, clamped pixels, short lines, and scalar tails are unchanged. This is specific to AArch64 NEON; x86, BoxBlur, and Parallel CPU are unaffected.

## Frozen cohort

- Workloads: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768` and `pipeline-chain.blur-material.gaussian-rgba-1024x768`.
- Exact parity cases: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5` and `PIL.Image.Image.filter.nuanced.performance-material-rgba-gaussian-1024x768-radius-7-5`.
- Profile: `python-simd`, release, default Cargo features; no Parallel CPU.
- Boundary: warm `whole_workflow` (setup/frombytes, filter, tobytes materialization), 1 warmup, 3 iterations × 2 samples, concurrency 1.
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0; revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty tree.
- Each leg freshly ran and passed the same 2 strict parity comparisons.

## A–B–B–A results

| Leg | Noise Pillow | Noise SIMD | Pillow/SIMD | Uniform Pillow | Uniform SIMD | Pillow/SIMD |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A1 control | 6.771 ms | 8.101 ms | 0.836× | 6.946 ms | 0.617 ms | 11.263× |
| B1 candidate | 7.513 ms | 6.684 ms | 1.124× | 6.870 ms | 0.677 ms | 10.149× |
| B2 candidate | 8.988 ms | 6.480 ms | 1.387× | 8.013 ms | 0.636 ms | 12.600× |
| A2 control | 6.705 ms | 8.191 ms | 0.819× | 6.791 ms | 0.789 ms | 8.606× |

Noise SIMD mean fell from 8.146 ms across A1/A2 to 6.582 ms across B1/B2: 1.238×, 19.2% lower median. In both candidate reports SIMD was faster than same-run Pillow (ratios 1.124× and 1.387×). Pillow’s noise median shifted from 6.738 ms in controls to 8.251 ms in candidate legs, so the comparison has host variance; the measured gain versus the matched SIMD controls is clearer than the extra margin over Pillow. The result remains below both the 2× prioritization threshold and the 5× SIMD target.

The uniform workload returns through an identity path before the changed block. Keep it as a no-regression guard only; its per-leg latency varies and no gain is attributed to the scan.

All four benchmark reports are schema-valid. SIMD receipts report `actual_backend=simd`, terminal completion, no fallback, `path=vector`, 589,824 vector blur blocks, and zero scalar tail for the noise workload. The candidate code path is selected by the AArch64-gated NEON module and existing full-block dispatch; the receipts do not separately count each NEON intrinsic. Whole-workflow timing includes allocation and result materialization.

`cargo test -p pillow-rs rgba_radius_seven_neon_prefix_blocks_match_scalar_edges_and_tails -- --nocapture` passed 1/1 on the candidate source; this existing scalar-reference contract exercises multiple widths, complete blocks, edges, and tails. `make build-parity` built the candidate release extension successfully. The build reported the existing seven `pillow-rs-py` missing-documentation warnings and `block 0.1.6` future-incompatibility notice. No full suite or coverage ran.

Control `box_blur_rgba_neon.rs` SHA-256: `281dd28dd32de75d6c0eaf7ebcc4ed9d596a0806605a89db05afe1a215cfc33e`; control extension SHA-256: `37bff1bce65feaa0bacd62f550f142b04dd6259ea8cd9d7b2d59e5aa05cbd8fc`. The measured candidate source SHA-256 was `e30ada3ceaebd2ce51e9cf623ab4e8e91bc6821007868e8fb9deb5c65a237bcc`; after the cohort, rustfmt changed only whitespace, giving the final source SHA-256 `7649aa692c011ed1d76d90f6845db5bffc56008cd7bb6a2296359871de2927ec`. Rebuilding produced the byte-identical measured candidate extension SHA-256 `8754d766b60e2ef55705aaf1f7cceee52b08dd52e42834a8e4244fe50a8bec15`, so the full-call measurements and strict parity remain bound to the current executable. The focused block test passed again on the final formatted source. `adapters.rs` was unchanged at SHA-256 `c00f7e816b1dbcad995b530fad26824abf45c779f3e303e427bec8b2e9e73955`.

Limit: one ARM64 host, one large RGBA radius-7.5 noise image and one identity guard. x86, other sizes and modes, serial CPU, Parallel CPU, Pillow-SIMD, and GPU remain unmeasured. This is a local SIMD improvement, not GaussianBlur operation completion. See Kata `pillow-rs#vc6w`; its parent issue remains open.

Each leg retains benchmark, parity, execution, producer, raw-sample, and parity-attestation artifacts under the original basenames referenced by the reports. `SHA256SUMS.txt` covers all retained files except itself. No commit or push was made.
