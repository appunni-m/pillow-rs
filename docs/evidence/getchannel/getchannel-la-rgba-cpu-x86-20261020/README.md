# GetChannel LA/RGBA serial CPU x86 current-source refresh

Date: 2026-10-20. This selected run refreshes two GetChannel mode rows from the
older operation snapshot. It is a measurement update, not a code optimization.

## Frozen scope

| Mode | Workload | Exact parity case |
| --- | --- | --- |
| LA | `pil-image-image.getchannel.materialized.la-1024x768` | `PIL.Image.Image.getchannel.nuanced.performance-la-1024x768` |
| RGBA | `pil-image-image.getchannel.materialized.rgba-1024x768` | `PIL.Image.Image.getchannel.nuanced.performance-rgba-1024x768` |

The run selected only these workloads and cases, with the serial CPU backend,
warm `call;observe-result`, 5 warmups, 20 iterations × 5 samples, and
concurrency 1. Pillow 12.2.0 and pillow-rs ran on the same x86_64 Linux host
under CPython 3.12.3. The runner identifies the machine generically as
`machine`; no more specific CPU identity is claimed.

## Results

| Mode | Pillow median / P95 | CPU median / P95 | Pillow ÷ CPU |
| --- | ---: | ---: | ---: |
| LA | 306.791 / 353.951 µs | 253.956 / 267.711 µs | 1.208× |
| RGBA | 305.101 / 354.911 µs | 271.721 / 296.531 µs | 1.123× |

Both selected parity cases passed (2/2). Benchmark and parity result schemas
validated. For both target rows, the execution receipt reports requested and
actual backend `cpu`, terminal completion, one `ExtractBand`, no fallback,
786,432 bytes of output allocation, and no upload or readback. The captured
host-buffer totals are 2,359,296 bytes for LA and 3,932,160 bytes for RGBA.

## Identity and limits

- Benchmark: `migration-benchmark-7c2af365a0b944e2b901d308905a6afb`
- Selected parity: `migration-parity-403983bbfaa14ad6975a4847428b4892`
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf` (dirty)
- Loaded `_core` SHA-256: `7b7433a32cd1a4c49dd4019d328b673f5cf960c16a6cef2133a6123ce118a066`
- Full benchmark input SHA-256: `fced232cf921915022a99786b4cc52a5bb4525cef6fa7cd55d82ae3ba2e12900`
- Full parity input SHA-256: `b138a899330e292b35365c2e0c9321168d2db6d3a92c1ee4dbc42b0ed1fd8743`

The GetChannel wrapper, binding, CPU kernel, and SIMD adapter source hashes
match the local worktree exactly; their hashes and all report attestations are
retained alongside this note. The full fixture hashes belong to the runner's
dirty checkout, so this is selected-object evidence, not a clean full-manifest
baseline. No generated baseline was promoted. The existing x86 checkout was
reused in place; no checkout or target directory was copied, and no remote
files were deleted. Four Pillow/CPU subject rows were added to
`docs/evidence/performance-optimization-local-runs.csv`.

This refresh clears the old LA/RGBA x86 CPU misses for these exact selected
rows. It does not close GetChannel: the current RGB x86 row remains 0.9519×
Pillow. SIMD, Parallel CPU, GPU, other modes/sizes, and Pillow-SIMD comparisons
receive no credit from this CPU-only run.
