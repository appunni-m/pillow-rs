# GetBBox-I portable row-OR — native x86 A/B/B/A

Date: 2026-10-18 (Asia/Kolkata). This is a focused serial-CPU result for four
materialized 1024×768 `I`-mode inputs. It improves those workloads, but does not
complete `GetBBox`: other modes, sizes, SIMD, GPU, and sustained throughput are
still open.

## Frozen scope and parity

- Workloads: `pil-image-image.getbbox.materialized.i-filled-1024x768`,
  `...i-blank-1024x768`,
  `...i-single-center-negative-1024x768`, and
  `...i-single-bottom-right-negative-1024x768`.
- The timer covers the public `call` step. Image setup is outside the timed
  step; this is a direct-call comparison on already materialized images.
- Each benchmark leg used the existing policy: warm cache, 5 warmups,
  20 iterations × 5 samples, concurrency 1. Only `python-cpu` was selected.
- The candidate preflight passed all 10 selected parity cases: seven I-mode
  cases (including both negative full-size cases and the 8×8 pattern) and the
  three F-mode controls, including negative zero. A1, B1, and A2 also passed
  the four benchmark cases; B2 reused B1's matching parity proof.
- The focused Rust row-boundary test passed widths 1–19 with blank rows,
  signed values, single hits, and grouped-scan tails. No workloads, thresholds,
  or parity assertions were changed.

The native runner is an AMD Ryzen 5 2600X, Linux x86_64, CPython 3.12.11,
Pillow 12.2.0, rustc 1.91.1. The focused slice manifest retains the original
case and workload definitions while indexing only these 10 parity cases and
four workloads. The runner received those small inputs, not the 38.8 MB full
parity fixture or a Cargo target directory.

## Public-call medians

All values are microseconds. `A1` and `A2` are the baseline; `B1` and `B2` are
the portable row-OR candidate. Speedups compare each candidate target with the
Pillow median from the same leg.

| Workload | A1 CPU | B1 CPU / Pillow | B2 CPU / Pillow | A2 CPU | B1/B2 speedup vs Pillow |
| --- | ---: | ---: | ---: | ---: | ---: |
| Filled | 3423.088 | 7.320 / 8.010 | 7.545 / 7.995 | 3430.524 | 1.094× / 1.060× |
| Blank | 592.515 | 52.090 / 288.110 | 52.700 / 293.191 | 593.061 | 5.531× / 5.563× |
| Single center | 593.250 | 53.035 / 293.735 | 52.910 / 296.250 | 594.132 | 5.538× / 5.599× |
| Single bottom-right | 594.195 | 49.971 / 293.341 | 50.220 / 292.985 | 592.341 | 5.870× / 5.834× |

Every target receipt reports actual `cpu`, one completed GetBBox operation, and
no fallback. The candidate beats Pillow on all four selected workloads. The
A1/A2 baseline medians are stable; filled differs by 0.22%, blank by 0.09%,
center by 0.15%, and bottom-right by 0.31%.

The I-mode scan groups raw bytes into four independent `u64` OR accumulators
per 32-byte block, skips rows whose accumulated words are zero, and resolves
only nonempty rows with the exact `i32 != 0` predicate. Once the horizontal
bounds span the image, a reverse row scan finds the bottom edge. F mode is
unchanged, preserving negative-zero-as-data. The implementation uses portable
Rust source without intrinsics; receipts remain CPU and earn no SIMD credit.

## Identity and retained artifacts

Source SHA-256: baseline `016670bd1efa61429332de655c0289f019837a2082ad096f84352625cc86dc13`;
candidate `f6cd352a0a1b122eb4d086207e908bbd89b91d7fb3477da506f9e214ecf3746e`.
The candidate source archived on the runner has the same hash as the local
source. A1 and A2 use identical baseline source bytes, but rebuilding the
baseline produced different loaded-runtime fingerprints. The harness rejected
reuse of A1 parity for A2; A2 then ran fresh parity and completed. This is why
the A2 result is retained with its own parity attestation.

The runner JSON reports, parity reports, attestations, producer bundles, raw
samples, and the two source files are in `runner-artifacts/`; the compressed
snapshot is `runner-artifacts.tar.gz`. The exact focused manifest and inputs
are beside this README as `manifest-slice.yaml`, `selected-parity-cases.json`,
and `selected-workloads.json`.

No SIMD or GPU path was measured or credited. The operation remains open for
other supported modes and sizes, the SIMD profile, GPU latency, and GPU
sustained throughput. No coverage or full parity/benchmark campaign was run.
