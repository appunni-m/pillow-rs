# RGB GetChannel output initialization A–B–B–A

Date: 2026-10-19

## Change

The SIMD adapter previously allocated a zero-filled output image and then
overwrote every byte while extracting an RGB channel. The candidate allocates
capacity for the final output and lets the selected SIMD gather initialize
every output byte, including the scalar tail. Channel dispatch remains
const-specialized outside the pixel loop. The public call and result
materialization are included in the timed boundary.

Candidate source hashes:

- `extract_band.rs`: `01e7b0da0007ab1d816442f4df58f0b70902931e1733e819807da74ef698bbbc`
- `native_channel.rs`: `68beef7f015a267c1fffaff3afb7d5a2a24cfea394d4bca88074e731e58b416a`
- ARM64 extension: `466b6ca4d1391a5bb7332b882b89a16c627d3bae2134bef89c17dc4bfe03ba20`

## Full-call results

Primary workload: `pil-image-image.getchannel.materialized.rgb-1024x768`,
RGB 1024×768, warm release build, one concurrent caller, five warmups and
20×5 samples. The boundary observes both `call` and `observe-result`.
Values below are median microseconds from each report; each ratio uses Pillow
from the same report.

| Host | Run | Pillow | SIMD | Pillow / SIMD |
| --- | --- | ---: | ---: | ---: |
| ARM64 | A1 control | 203.937 | 89.188 | 2.287× |
| ARM64 | B1 candidate | 193.126 | 85.646 | 2.255× |
| ARM64 | B2 candidate | 182.979 | 78.896 | 2.319× |
| ARM64 | A2 control | 158.270 | 85.250 | 1.857× |
| x86_64 | A1 control | 297.263 | 95.011 | 3.129× |
| x86_64 | B1 candidate | 291.437 | 81.366 | 3.582× |
| x86_64 | B2 candidate | 283.087 | 81.306 | 3.482× |
| x86_64 | A2 control | 295.166 | 93.321 | 3.163× |

Across the two control and two candidate target medians, ARM64 candidate
latency is 5.67% lower (87.219→82.271 µs), and x86_64 candidate latency is
13.63% lower (94.166→81.336 µs). Both candidate runs beat their adjacent
control. The result supports keeping this focused allocation change, but it
remains below the repository-stated 5× SIMD/Pillow target and does not
establish operation-wide completion. This experiment did not benchmark
Pillow-SIMD.

The 16×16 ExtractBand small guard is flat on ARM64 (19.438 µs mean on both
variants) and 4.68% lower on x86_64 (53.351→50.853 µs mean). The shared
Grayscale YCbCr guard is noisy on ARM64: target medians average 52.427 µs for
control and 56.531 µs for candidate, while the average within-report Pillow
speedup is effectively unchanged (2.375× vs 2.376×). A flagged B3 repeat
measured 311.229 µs Pillow and 73.876 µs SIMD while unrelated host load was
high; it is retained here but excluded from the comparison. The x86_64
Grayscale guard target medians average 808.759 µs for control and 826.576 µs
for candidate, a 2.20% increase on code whose x86 path is unchanged. Keep
these guards visible; they do not establish a clean Grayscale gain or
regression.

## Parity and executed paths

The primary full-size workload passed its exact parity gate in every retained
benchmark report. The focused RGB edge corpus passed 8/8 cases on both hosts
for A1 and B1, including incomplete vector tails. Candidate execution receipts
show actual `simd` execution through `neon-deinterleave` on ARM64 and
`ssse3-gather` on x86_64, with 49,152 vector blocks and no scalar tail or
fallback on the full-size image. The candidate initializes one final output
buffer; no output-format conversion is used.

ARM64 A and B extension hashes were respectively
`b80535f7292b0f60d552b8788f359969c1100fc15ec3f67cc0e2cb9e44777361` and
`466b6ca4d1391a5bb7332b882b89a16c627d3bae2134bef89c17dc4bfe03ba20`.
x86_64 A and B core hashes were respectively `9d2d192f…` and
`d5ffbfa7d6e0e515936b7ffbcc11dd36fce65a44edcf5b611aece679c14865ed`.
These identities distinguish the corrected x86 candidate from an earlier
stale-build attempt described in `x86/stale-unrebuilt/README.md`.

The A2 parity reuse was rejected because the runtime fingerprint includes
volatile file stat metadata, which changed on a byte-identical rebuild. A2
therefore has a fresh selected-case parity preflight, which passed. This is
the existing runtime-fingerprint issue tracked as Kata `pillow-rs#bz5d`;
the guard was not relaxed.

The x86_64 runner had Rust 1.98.1, while the repository pins Rust 1.96.1, so
the native x86 builds used `RUSTUP_TOOLCHAIN=1.98.1`. The x86 runner used
Pillow 12.2.0 on CPython 3.12.3. ARM64 used the local Apple ARM host. This
toolchain mismatch is a limitation on cross-host comparison; A and B remain
paired within each host.

## Retained artifacts and open scope

The JSON reports, raw samples, parity reports, producer attestations, and
benchmark attestations are retained beside this README. The invalid first
x86 attempt is quarantined under `x86/stale-unrebuilt/` and is not speed
evidence. No operation-wide claim follows from this RGB result: other modes,
serial CPU, Pillow-SIMD, GPU latency, sustained GPU throughput, and the full
GetChannel matrix remain open.
