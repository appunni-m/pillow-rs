# GaussianBlur RGBA four-channel step specialization — rejected

This focused CPU experiment tested an explicitly unrolled four-channel branch
in `blur_line_step` for the noisy 1024×768 RGBA `GaussianBlur(7.5)` workflow.
The exact workload was
`pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`; the parity case
was
`PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`.
The warm whole-workflow boundary included image construction, filter creation
and execution, and `tobytes`. Each run used one warmup and 3 iterations × 2
samples at concurrency one on macOS ARM64 with Pillow 12.2.0.

The candidate `filter.rs` SHA-256 was
`55156405c37f8e467f24ac0b467b7b77a4690a93fb29b0a315340aac88081af2`; its
loaded `_core.abi3.so` SHA-256 was
`74c983b39d5e70340444683d5bf46d2b80258b208f8f28cd356b9a00b71dd2b1`.
The reverted control source SHA-256 is
`6b063307eba475abeaad72f0c4bb55394ff0d57f204487748e933f26d36e139c`, and its
loaded core SHA-256 was `af985e7b98977920ed0ec21ba59092b9ee7e4b7e0d4a241cab0703ec9dd874bb`.
Both source variants used the same Python wrapper files and exact workload,
asset, Pillow oracle, host, and measurement boundary.

| Run | Pillow median | CPU median | Pillow / CPU | Parity | CPU execution receipt |
| --- | ---: | ---: | ---: | --- | --- |
| A1 control | 6.630375 ms | 8.968896 ms | 0.739× | Pass | CPU, terminal complete, no fallback |
| B1 candidate | 6.394666 ms | 8.438333 ms | 0.758× | Pass | CPU, terminal complete, no fallback |
| B2 candidate | 6.287895 ms | 8.400354 ms | 0.749× | Pass | CPU, terminal complete, no fallback |
| A2 control | 6.169562 ms | 7.409146 ms | 0.833× | Pass | CPU, terminal complete, no fallback |

The later control's six CPU samples ranged from 7.2515 to 7.7163 ms; the
candidate's twelve samples ranged from 7.9903 to 8.6612 ms. The first control
was noisy (7.8282–12.4644 ms), so its apparent candidate improvement is not
reliable. Against the stable later control, the candidate was about 13.6%
slower. The candidate did not improve the Pillow ratio and still missed Pillow
in every run, so its source and focused test were reverted. No SIMD or GPU
claim was measured.

The candidate's exact CPU parity report passed 1/1 as
`migration-parity-2aab8a6ea135491b9f674d3757dc8fb3`; a fresh control parity
report passed 1/1 as `migration-parity-7bf987d1dc4944d783a2e5c6e1a96f88`.
A failed attempt to reuse the older control parity was caused by a
volatile identity detail, not a parity mismatch: rebuilding produced the same
baseline extension SHA-256, but changed only its filesystem ctime, mtime, and
inode, which changed the runtime `stable_sha256`. The exact affected parity
case was rerun and passed. Track that attestation behavior as a proof-tool
identity defect; do not weaken parity checks or count the rejected reuse as a
parity failure.

The A1/B1/B2 reports, attestations, and raw samples, plus the fresh parity and
A2 control artifacts, are in
[`gaussian-rgba-noise-rgba-four-lane-abba-arm64-20261016`](../gaussian-rgba-noise-rgba-four-lane-abba-arm64-20261016/).
This README records the final decision; no generated result files were
duplicated.

No candidate code remains. The operation remains open and slower than Pillow.
Kata tracking: `pillow-rs#vc6w`. No coverage, commit, or push ran; the commit
batch remains 0/33.
