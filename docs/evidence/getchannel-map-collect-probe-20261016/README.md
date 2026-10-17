# RGB GetChannel `map`/`collect` probe — 2026-10-16

This follow-up tests the `chunks_exact(3).map(...).collect()` candidate that
was not part of
[`getchannel-scalar-loop-probe-20261016`](../getchannel-scalar-loop-probe-20261016/README.md).
It is standalone kernel evidence; the separate public-call cohort is in
[`getchannel-map-collect-arm64-20261016`](../getchannel-map-collect-arm64-20261016/README.md).

## Method and result

The probe extracts each RGB channel from a deterministic 1024×768 byte buffer
and allocates, consumes, and drops the output for each timed call. It compares
the current grouped gather with:

```rust
source
    .chunks_exact(3)
    .take(pixel_count)
    .map(|pixel| pixel[channel])
    .collect()
```

The `take(pixel_count)` bounds extraction to the logical raster even if the
backing buffer has trailing storage. The ARM64 run used Rust 1.96.1, native
target CPU, optimization level 3, one codegen unit, four warmups, nine samples,
and 20 calls in each sample. Output was checked against the grouped loop for
each channel, small dimensions, and trailing bytes.

| Channel | Current grouped median | `map`/`collect` median | Kernel speedup |
| --- | ---: | ---: | ---: |
| R | 124,275 ns | 30,031 ns | 4.14× |
| G | 123,958 ns | 30,852 ns | 4.02× |
| B | 124,345 ns | 29,785 ns | 4.18× |

The native x86_64 probe reported in Kata `pillow-rs#jpj6` comment 2732 measured
44–51 µs per channel for this candidate against 400–406 µs for the current loop
(7.9–9.2×). It used Rust 1.91.1 with `-C target-cpu=native` and one codegen
unit. These kernel-only measurements motivated a public-call test; they do not
by themselves establish Pillow parity or an end-to-end improvement.

## Limits

The ARM samples have a long upper tail, and the x86 and ARM compiler versions
differ. Treat these as candidate-selection evidence. They do not establish
performance for LA/RGBA, SIMD, Parallel CPU, GPU, other image sizes, or other
hosts. The public ARM64 B-A-B result remains diagnostic and does not update the
canonical operation matrix.

## Artifacts

- `probe.rs` — standalone loop, exact-output guards, and timing harness.
- `arm64.txt` — raw ARM64 toolchain and timing output.

SHA-256:

| File | SHA-256 |
| --- | --- |
| `probe.rs` | `0d5e5ffb0414d5d150d7d971b5f1a75444ce2f0c6ba0d6aea30e974c4726f983` |
| `arm64.txt` | `a8f87684ab1ecd83dd1065284dafc418995e7f39ba084df2fe4e7242a8c1bf94` |
