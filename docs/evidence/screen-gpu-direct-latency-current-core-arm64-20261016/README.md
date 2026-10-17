# Screen mapped-secondary GPU probe (2026-10-16)

This probe tested whether a second `MAP_WRITE | STORAGE` buffer can avoid
`Queue::write_buffer_with` staging for direct native-byte `ImageChops.screen`
on unified-memory Metal. The benchmark boundary stayed the public call plus
terminal `tobytes()` materialization.

## Frozen workload and guards

- Workload: `pil-imagechops.screen.standard`
- Parity case: `PIL.ImageChops.screen.nuanced.performance-materialized-rgb-noise-1024x768`
- Input: RGB, 1024×768, warm cache, concurrency 1, 5 warmups, 20 iterations × 5 samples
- Timed steps: `call`, `observe-result`
- Guard cases: `gpu-native-byte-tail-l-5x5`, `-la-5x5`, `-rgb-5x5`, and `-rgba-5x5`
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7, arm64
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty tree)
- Control `_core` SHA-256: `8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`
- Candidate `_core` SHA-256: `bd85d04bcdf678fa9cc089d7a69a2256b1ee9321bd8558ea8a020d82addac872`

The mapped candidate passed its full RGB benchmark parity gate. The four strict
GPU tail guards passed 4/4 and their receipts show actual GPU execution, one
dispatch, terminal completion, and no fallback. Candidate timed receipts report
`gpu-mapped-secondary`. Primary upload, auxiliary upload, and readback each
remained 2,359,296 bytes; parameters remained 16 bytes. No transfer was
eliminated.

## A–B–B–A results

Medians are milliseconds; each cell represents 100 measured samples.

| Profile | A1 control | B1 candidate | B2 candidate | A2 control |
|---|---:|---:|---:|---:|
| Pillow | 4.198479 | 4.288917 | 4.058021 | 4.580459 |
| Serial CPU | 0.403021 | 0.468834 | 0.472834 | 0.354687 |
| SIMD | 0.461375 | 0.527917 | 0.491021 | 0.457646 |
| GPU | 1.655938 | 1.487021 | 1.247354 | 1.497812 |

The median of the two report medians is 1.576875 ms for control GPU and
1.367187 ms for candidate GPU (15.3% faster). Candidate GPU remains about
2.68× slower than candidate SIMD. Candidate CPU and SIMD medians are about 24%
and 11% slower than the control medians, respectively. The host varied across
runs, so this does not prove those cross-profile changes were caused by the GPU
code; it also does not rule them out. GPU P95 did not show a repeatable gain.

## Decision

Reject the mapped-secondary implementation for now. Its GPU median gain is
narrow, it does not approach the GPU≤SIMD latency target, and the observed
CPU/SIMD regressions cannot be ruled out with this cohort. Keep the original
queue-staged path until a more isolated experiment can prove the gain without
cross-profile regressions. The source and loaded extension were restored to the
A control state; the current extension SHA-256 is
`8b0dda44c94c7c998a511aebd8718c1d244ad60650086ebcb2bb21d26409d749`.

A1 and B1 each ran fresh parity. B2 reused B1's report only after runtime and
extension identity checks. A2's reuse of A1 was refused because the recorded
`python-cpu` runtime identity differed; A2 therefore ran the same single-case
parity fresh and passed. No full parity campaign, full test suite, coverage,
commit, or push ran. The incomplete `benchmark-a2-control.*` sidecars are from
the refused reuse attempt; use `benchmark-a2-retry.*` as the canonical A2 run.

## Evidence files

- `benchmark-a1-control.*`: first control report, attestations, and raw samples
- `benchmark-b1-candidate.*`: first candidate report, attestations, and raw samples
- `benchmark-b2-candidate.*`: second candidate report, attestations, and raw samples
- `benchmark-a2-retry.*`: restored control report, attestations, and raw samples
- `parity-a1-control.*`, `parity-b1-candidate.*`, `parity-a2-retry.*`: full RGB parity evidence
- `parity-b1-tail-gpu.*`: strict GPU tail-guard results and execution receipts
