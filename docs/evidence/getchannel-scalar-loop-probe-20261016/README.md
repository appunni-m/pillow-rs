# RGB GetChannel scalar-loop probe — 2026-10-16

This is a standalone kernel probe for the current serial CPU RGB gather loop
in `pillow-rs/src/compute/pool_cpu/ops/color.rs`. It changed no production
source and is not public-call performance evidence.

## Method

The probe extracts one channel from a deterministic 1024×768 RGB byte buffer
(2,359,296 input bytes), allocating an output `Vec` for every call. It compares
the current four-pixel `extend_from_slice` loop with unrolled `push`,
per-pixel `chunks_exact(3)`, and `iter().skip(channel).step_by(3).collect()`.
Allocation, output construction, black-box consumption, and vector drop are
inside each timed call. Each sample runs 20 passes across all three channels;
five warmups and five samples are used for each candidate. Values are divided
by the 60 calls in each sample to report microseconds per one-channel call.

Before timing, every candidate is checked byte-for-byte against a replica of
the current grouped loop for channels 0–2 and pixel counts 0, 1, 2, 3, 4, 5, 7,
8, 9, 15, 1023, and 1024. The source contains five extra trailing bytes so the
checks exercise logical-raster bounds. This is an internal loop-equivalence
check, not a Pillow parity comparison.

The ARM64 probe used local Rust 1.96.1; the x86_64 host used Rust 1.91.1. Each
host ran the same `rustc --edition=2021 -O` source. Cross-host values should not
be compared directly because both architecture and compiler differ. Compare
candidates against the current-loop baseline on each host:

| Kernel | ARM64 median µs/call (max of 5) | x86_64 median µs/call (max of 5) |
| --- | ---: | ---: |
| Current grouped `extend_from_slice` | 190 (288) | 429 (503) |
| Unrolled `push` | 421 (430) | 1,298 (1,642) |
| Per-pixel chunks | 385 (387) | 1,403 (1,408) |
| Stride iterator and collect | 1,198 (1,213) | 413 (417) |

## Decision

This limited candidate set does not support a portable replacement: the stride
iterator's roughly 4% x86 gain is outweighed by its greater than 6× ARM
regression, and the two other alternatives are slower on both hosts. ARM
current-loop samples ranged from 151 to 288 µs, so small differences must not
be treated as stable gains. A separate `chunks_exact(3).map(...).collect()` probe
was not included here; its results and the focused public-call experiment are
recorded in
[`getchannel-map-collect-probe-20261016`](../getchannel-map-collect-probe-20261016/README.md)
and
[`getchannel-map-collect-arm64-20261016`](../getchannel-map-collect-arm64-20261016/README.md).

These measurements omit Pillow and Python overhead, backend receipts, SIMD,
GPU transfer/synchronization, and public result materialization. They do not
change GetChannel's operation matrix or establish results for other modes and
profiles.

## Artifacts

- `probe.rs` — standalone input generator, reference loop, candidates, guards,
  and timing harness.
- `arm64.txt` — local ARM64 output.
- `x86_64.txt` — native x86_64 output. The script was sent through SSH stdin;
  no checkout or build directory was copied, and no existing remote files were
  deleted or changed. The output binary was written to a new `/tmp` path.

SHA-256:

| File | SHA-256 |
| --- | --- |
| `probe.rs` | `8e5c5e4a363045894c2b790501e40df4d81baf3e87154662f86ed1e232339a39` |
| `arm64.txt` | `62ac212be7d3d03c4c5b5635bcd2699cb41dab35a7a3642ac0e862128ebadd13` |
| `x86_64.txt` | `a36b7233e923db57f69eb485ba17b4b3b2662069c006745e6229a743307f41c1` |
