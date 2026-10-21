# GetChannel LA serial CPU A–B–B–A

Date: 2026-10-21. This is a rejected candidate experiment for the x86 LA
GetChannel row. It does not change the generated baseline.

## Frozen scope

- Workload: `pil-image-image.getchannel.materialized.la-1024x768`
- Exact parity case: `PIL.Image.Image.getchannel.nuanced.performance-la-1024x768`
- Profile: `python-cpu`, default Cargo features, Parallel CPU disabled
- Boundary: warm `call;observe-result`, 5 warmups, 20 × 5 samples, concurrency 1
- Host: existing native x86_64 runner, generic runner identity `machine`
- Output: L mode, 1024 × 768, 786,432 bytes

The candidate moved LA extraction ahead of `GrayImage::new`, collecting one
selected byte per two-byte pixel into a Vec and wrapping it directly. It was
intended to avoid zero-filling an output image that the existing grouped loop
then overwrites. It preserved `take(pixel_count)` for trailing storage and the
existing `idx.min(1)` behavior. The code was restored after the trial.

## Results

| Leg | Pillow median | CPU median | Pillow ÷ CPU | CPU p95 |
| --- | ---: | ---: | ---: | ---: |
| A1 control | 291.687 µs | 247.3315 µs | 1.179× | 254.201 µs |
| B1 candidate | 294.8265 µs | 301.0115 µs | 0.979× | 309.331 µs |
| B2 candidate | 298.991 µs | 303.951 µs | 0.984× | 310.832 µs |
| A2 control | 294.401 µs | 247.336 µs | 1.190× | 260.171 µs |

The candidate was 21.7% and 22.9% slower than its adjacent controls. Pillow
medians varied by less than 2.5% across the four legs. Control target medians
were effectively identical, so the candidate regression is attributable to
the changed LA path for this workload. The expected zero-fill saving did not
offset the collection path's cost.

Every leg passed the exact LA performance parity case. Receipts show actual
CPU `ExtractBand`, terminal completion, no fallback, one output allocation,
and no transfer. B2 reused B1 parity for the same extension. The candidate
also passed the eight selected LA extraction, empty-size, and invalid-index
guards (8/8). No SIMD, GPU, Parallel CPU, or other modes were timed.

## Source and build identity

- Revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf` with a dirty worktree
- Control `color.rs` SHA-256: `1e0536eeccdde70eac33495bbdc1553749021f4f63a81a5e1d9acf6daa7dc732`
- Candidate `color.rs` SHA-256: `55cd7f9ae8320c09f4172c0d9cbac5d34fbbed466176e5b75318b478517ab884`
- Control loaded `_core` SHA-256 (A1/A2): `7b7433a32cd1a4c49dd4019d328b673f5cf960c16a6cef2133a6123ce118a066`
- Candidate loaded `_core` SHA-256 (B1/B2): `cf201f912b82a02a883a8392992c04cfa4ceae2ec6b22bd8eea6edc002e27c57`

The runner source and package extension are restored to the A control hashes.
The runner's venv did not contain Maturin, so `make build-parity` could not
start. The same locked release extension was built with Cargo's default
features and installed from the existing `target/release/lib_core.so` inside
the existing work folder. No target directory or parity corpus was copied,
and no remote files were deleted.

## Artifacts

The four compact benchmark reports, execution attestations, and raw timing
samples are retained beside this note. The selected eight-case parity report
is also retained. The three full performance-parity JSON reports (about 4.2 MB
each) remain on the native runner; the benchmark correctness gate and report
hashes are captured in their corresponding files and in the Kata comment on
`pillow-rs#jpj6`.

Benchmark runs, in order: `migration-benchmark-9e2b02c2cfe743d098e4446b50ea130d`,
`migration-benchmark-969c364939444536b91fc0f1c3ea4b19`,
`migration-benchmark-70e9410bd2d544d2a3eeb3c6af8f3b0e`, and
`migration-benchmark-3ae330a68624426a8ca9f63ed44b89af`.

Disposition: do not retain or retry this candidate. Continue to a different
ranked operation or a distinct cost hypothesis.
