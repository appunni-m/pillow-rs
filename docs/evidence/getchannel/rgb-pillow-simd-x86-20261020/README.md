# GetChannel RGB — Pillow and Pillow-SIMD — native x86 — 2026-10-20

This is selected current-worktree evidence for
`pil-image-image.getchannel.materialized.rgb-1024x768`, paired with exact case
`PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`. It is a lead for
ranking and triage, not a promoted baseline or an accepted optimization.

## Measurement

Both cohorts used warm `call;observe-result`, 5 warmups, 20 iterations × 5
samples, concurrency one, and 100 latency observations. The timer includes the
public call and observed result materialization. The runner was
`native-x86-192.168.0.221`, Linux 7.0.0-38 generic x86_64, CPython 3.12.3,
release build. CPU model, power state, and toolchain details beyond the Python
runtime were not recorded. Pillow was 12.1.1; Pillow-SIMD was 12.1.1.post0
(default SSE4 build).

| Cohort | Profile | p50 latency | p95 latency | Baseline / target | Actual backend |
| --- | --- | ---: | ---: | ---: | --- |
| Pillow | Pillow | 303.151 µs | 355.362 µs | 1.000× | Pillow |
| Pillow | Serial CPU | 316.321 µs | 335.762 µs | 0.958365× Pillow | CPU |
| Pillow | SIMD | 85.885 µs | 97.100 µs | 3.529732× Pillow | SSSE3 gather |
| Pillow-SIMD | Pillow-SIMD | 215.221 µs | 402.272 µs | 1.000× | Pillow-SIMD |
| Pillow-SIMD | Serial CPU | 314.462 µs | 324.302 µs | 0.684501× Pillow-SIMD | CPU |
| Pillow-SIMD | SIMD | 86.510 µs | 97.110 µs | 2.487816× Pillow-SIMD | SSSE3 gather |

The CPU result is slower than both reference implementations in their respective
cohorts. SIMD is faster than both references but remains below the campaign's
5× Pillow target. These are separate benchmark cohorts: use only the paired
baseline shown in each row. The Pillow-SIMD compatibility preflight completed
with no output differences for this workload. No GPU result or sustained
throughput result was collected.

Pillow-cohort parity passed 2/2 selected comparisons (serial CPU and SIMD).
Both targets completed on their requested backend without fallback. The CPU
receipt reports one `ExtractBand` operation and one 786,432-byte output
allocation. The SIMD receipt reports `ssse3-gather`, 49,152 vector blocks,
zero scalar tail, and one 786,432-byte output allocation. Both have zero upload
and readback bytes.

## Identity limits

The selected workload and case objects match the local tree, and the relevant
Rust and Python operation-path source files have matching host/local hashes.
However, the full manifest and fixture input files differ between the reused
host checkout and this checkout. The 38.85 MB parity input was intentionally
not copied. The reports therefore document the selected object and path, but
are not strict whole-manifest baseline evidence. The measured checkout was
dirty at revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`; the reports do not
bind every dirty source file to the built extension. Do not promote these rows
to the generated regression baseline or claim a source change caused the
result.

| Input identity | x86 host | Current local checkout |
| --- | --- | --- |
| Manifest | `f740fbfba2f2ea927a3b14413e1fab65bc9da1daeddef061612f68fed155ee41` | `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885` |
| Benchmark input | `fced232cf921915022a99786b4cc52a5bb4525cef6fa7cd55d82ae3ba2e12900` | `ef9306991a7b85ec1c1f2f3e5d32a6cd8ff6269c96bea3f0c85f4016e4720ba5` |
| Parity input | `b138a899330e292b35365c2e0c9321168d2db6d3a92c1ee4dbc42b0ed1fd8743` | `fffed899cc0391dfdc2e33d89a5059263b5b3410520e252835ffe1ac31b942d9` |

The following operation-path files have matching host/local SHA-256 values:

| File | SHA-256 |
| --- | --- |
| `pillow-rs/src/compute/pool_cpu/ops/color.rs` | `1e0536eeccdde70eac33495bbdc1553749021f4f63a81a5e1d9acf6daa7dc732` |
| `pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs` | `e5f5f930138b9bea14b27130ffbbef30c671a3f9b4e334e72c977d6bad3ecdde` |
| `pillow-rs/src/compute/pool_simd/ops/native_channel.rs` | `1b745c4b56e1ff40471d5a7e965dca6d044a1f024ac204cf0392d3e32138a68e` |
| `pillow-rs-py/src/lib.rs` | `cff227d114b697d1f941ef4ba2788ce166d73e86d044f4b84e4edcefe511ef2c` |
| `pillow-rs-py/python/pillow_rs/image.py` | `da25efb014afe647295a81f6beae6e4864a5810f8e0b3ecc32f6f992eb22e1e8` |

No code was changed for this comparison, no operation optimization is
accepted, and GetChannel remains incomplete across its other modes, sizes,
architectures, and backends.

## Artifact identity

Selected workload object SHA-256:
`f0203987ebe336b57fe121e3a4d8a6f2cdb1e787d8c28c2d9192cb1a8d1dd5cf`.

Selected parity case object SHA-256:
`ca5d07d413f695119196f9bdb980f2fc6bfb788173afed973c1f124907790e96`.

| Artifact | SHA-256 |
| --- | --- |
| `benchmark-result-pillow.json` | `577d7f09a104bae6be151999e12854ee65b2698bc7c3256fbe7900ac5b92092d` |
| `benchmark-result-pillow-simd.json` | `2b4836239b9ee66665b50ca5611c73cae9f50a4051ca9ad9d872bac3ecf75f52` |
| `benchmark-parity-result-pillow.json` | `4f5175a16a7dd7e07dcb2f461390e92e73a227def69a4ce49a5493cb26e387d9` |
| `benchmark-source-compatibility-pillow-simd.json` | `2e2ec90940684a9a7522997f194fc6dd72177c0125bc50e4c8c80816fa8cd569` |
| Host manifest | `f740fbfba2f2ea927a3b14413e1fab65bc9da1daeddef061612f68fed155ee41` |
| Host benchmark input | `fced232cf921915022a99786b4cc52a5bb4525cef6fa7cd55d82ae3ba2e12900` |
| Host parity input (not copied) | `b138a899330e292b35365c2e0c9321168d2db6d3a92c1ee4dbc42b0ed1fd8743` |

Run IDs: Pillow cohort `migration-benchmark-b23f9868c64f4c49aac70931588d43e5`;
Pillow-SIMD cohort `migration-benchmark-b9c7fb6598434ed78dc4d876d67acf7b`;
Pillow-cohort selected parity
`migration-parity-benchmark-gate-47ff3130381a4d8eb9efc1051ca2f353`. The
Pillow-SIMD compatibility preflight references selected parity runs
`migration-parity-3a1bf11b2e9444a19d5f856b47542074` (CPU) and
`migration-parity-93a6537be1f646d7a38785762867a280` (SIMD).

Only selected reports, attestations, manifest, and benchmark input were streamed
from the existing x86 worktree. No target directory or large parity input was
copied, and no remote files were changed or deleted.
