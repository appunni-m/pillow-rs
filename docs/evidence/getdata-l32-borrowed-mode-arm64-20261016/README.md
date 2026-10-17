# GetData L/32 borrowed mode guard — 2026-10-16

## Change and boundary

This focused trial replaces the loaded-image eligibility check in
`Image::getdata_band_into` that called `self.mode()` and created an owned
`String`. The guard now borrows `explicit_mode`, falling back to the static
name for `decoded_mode`. It preserves `mode()` precedence and changes no
pixel loop, output buffer, or Python materialization.

The selected workload was
`pipeline-chain.terminal-read.getdata.l-band0-32x32`; exact parity case
`PIL.Image.Image.getdata.performance.material-l-band0-32x32`; profile
`python-cpu`; selected step `band-data`. It used the warm full public call,
including the 1,024-byte Python `bytes` result: five warmups, 20 iterations,
five samples per iteration, concurrency 1. Pillow was 12.2.0 on CPython 3.12,
macOS ARM64. Each target receipt showed serial CPU, one completed GetData, and
no fallback.

## A–B–B–A result

| Run | Subject | Median | Pillow / CPU |
| --- | --- | ---: | ---: |
| A1 control | Pillow | 10.500 µs | — |
| A1 control | pillow-rs CPU | 5.500 µs | 1.909× |
| B1 candidate | Pillow | 10.125 µs | — |
| B1 candidate | pillow-rs CPU | 5.209 µs | 1.944× |
| B2 candidate | Pillow | 9.958 µs | — |
| B2 candidate | pillow-rs CPU | 5.167 µs | 1.927× |
| A2 control | Pillow | 10.083 µs | — |
| A2 control | pillow-rs CPU | 5.500 µs | 1.833× |

The candidate mean CPU median is 5.188 µs; the control mean is 5.500 µs, a
5.7% reduction. Every A/B run passed its exact parity preflight (1/1). The
candidate also passed focused invalid-band, negative-band, and post-mutation
snapshot guards (3/3). No assertions or workloads changed.

## Identity and limits

The control source `image.rs` SHA-256 was
`e8670886bbf370900487407a782d6bdbd0a262dfec55dedf6289108bcd2ba009`; both
control receipts loaded `_core.abi3.so` SHA-256
`cf5edefafcee72aa0d2f9163f62f04a46cfd97d82a8d272779cdcaf49166c379`. The
candidate source SHA-256 was
`9bde8c50db45d4239a57e7ca3a52f37c279a9200f2d036f5875f66392c78e826`; both
candidate receipts loaded `_core.abi3.so` SHA-256
`b1619f942d490e573c532a5da8da0b2a709ceb3248d676272f110266b25c5fd1`. The
control and candidate each match within their own bracket. The checkout was
dirty at `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, so these are local ARM64
diagnostics, not clean-matrix credit.

Run IDs:

- A1 `migration-benchmark-452a8b512dfe4c408dc495a8479e87dc`
- B1 `migration-benchmark-bbc8f9ff6e3f473ab6ebdb7f47ccb25f`
- B2 `migration-benchmark-1909ae19bc4f4487887b89fa2d5004d9`
- A2 `migration-benchmark-a7921aaeb7b84d5cae55eab10edb1728`
- Edge guards `migration-parity-6dbfcb91943f4c3f8f39ffa287ffab06`

Full benchmark, parity, raw-sample, producer, and execution-attestation
artifacts are retained beside this README. This small allocation removal does
not reach the 2× checkpoint: the candidate ratios were 1.944× and 1.927×.
It does not close GetData or establish another mode, size, host, or backend.
No test defect was found. No coverage, full campaign, commit, or push ran.
