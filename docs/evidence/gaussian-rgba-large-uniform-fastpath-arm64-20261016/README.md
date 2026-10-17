# Large uniform RGBA GaussianBlur fast path

This evidence covers `pipeline-chain.blur-material.gaussian-rgba-1024x768`
and parity case
`PIL.Image.Image.filter.nuanced.performance-material-rgba-gaussian-1024x768-radius-7-5`.
The input is 1024×768 RGBA filled with `[83, 149, 17, 191]`; the public call
creates the image, applies `GaussianBlur(7.5)`, and materializes with
`tobytes`. Measurements use Pillow 12.2.0, warm release mode, one warmup,
three iterations across two samples, and concurrency one.

## CPU A–B–B–A

| Run | Build | Pillow median | Rust CPU median | Pillow / CPU | Parity |
| --- | --- | ---: | ---: | ---: | --- |
| A1 | Control | 6.697 ms | 8.045 ms | 0.83× | Pass |
| B1 | Candidate | 6.161 ms | 0.707 ms | 8.71× | Pass |
| B2 | Candidate | 6.268 ms | 0.951 ms | 6.59× | Pass |
| A2 | Control | 6.586 ms | 8.208 ms | 0.80× | Pass |

The control extension loaded in A1/A2 has SHA-256
`d69ec731b5f0fa54b7a3d9d9a3e6c6211a1c5d085fbe75cc57dc0ba3ab3ac6e9`.
The candidate extension loaded in B1/B2 has SHA-256
`8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`.
The control and candidate `filter.rs` source hashes are respectively
`27618281f1089b194c7c5e9e62f568c01e88bd17665308815b5b4827ee5e87fb` and
`e224b1e8e3d9ac60df9512a3b011fa64a7fdb056f298651670e4861e2848da68`.
Execution receipts for all four legs show requested and actual CPU, terminal
completion, and no fallback. B1/B2 average 0.829 ms; A1/A2 average 8.126 ms.

The candidate is guarded to serial CPU, RGBA8, and three passes. It calculates
the exact Q24 rounded output for each byte in the first pixel under both axis
weights, checks a 7×5 grid only as a cheap rejection filter, then scans every
tuple before returning a clone. The full scan is the proof of uniformity. A
67×67 adversarial unit case changes pixel `(1,1)` outside the grid and matches
the explicit three-horizontal/three-vertical pass reference byte-for-byte.
The benchmark-sized uniform case also passed the focused Rust contract.

The strict parity case passed for every A/B leg (one CPU comparison for
A2/B1/B2; A1 also ran CPU, SIMD, and GPU comparisons). A1's extra profile rows
were unintended: `MIGRATION_TARGET_BACKEND=cpu` does not filter this benchmark
runner. Its CPU profile ran first. B1, B2, and A2 used the correct
`MIGRATION_BENCHMARK_BACKENDS=cpu` filter and selected only `python-cpu`.
The A1-only SIMD and GPU measurements are diagnostic controls, not candidate
results; no SIMD/GPU claim follows from this CPU-only optimization.

The nonuniform RGBA path has only rejection-only grid checks before its usual
blur when the probes differ. A large noisy RGBA timing guard has not been run,
so no general claim is made about that overhead. This result is local dirty-tree
evidence for the named input and does not complete the `Image.Image.filter`
operation matrix.

Raw parity JSON is gzip-compressed to keep the committed evidence small. Verify
the original canonical file hash by decompressing a report and comparing it
with the corresponding report attestation. `SHA256SUMS.txt` covers every file
in this directory, including the compressed parity reports and attestation
children.

## Files

- `A1` and `A2`: exact control reports, parity proofs, timing samples, and
  execution/producer attestations.
- `B1` and `B2`: exact candidate reports, parity proofs, timing samples, and
  execution/producer attestations.
- `SHA256SUMS.txt`: integrity hashes for the preserved evidence.
