# GetColors I adjacent-run coalescing trial — rejected — 2026-10-16

This read-only Kata proposal was implemented as a narrowly scoped experiment: after existing byte-domain and long-prefix scanners decline an input, the generic counter coalesces adjacent signed-I samples only for large images with a direct table of at most 65,536 slots and a repeated-pair prefix. It keeps `PillowColorCounts::count_repeated`, signed pixel bits, insertion/probe order, and scalar replay on count overflow.

## Measured result

The two-workload release run `migration-benchmark-e0b6122a8da847458c535f5ed63a45a7` passed its parity gate 2/2. The exact no-adjacent guard passed 1/1 in `migration-parity-580242ebbc0c4eeb87ec32333962c246` (run identity is in `extension-identity.json`).

| Workload | Pillow median | CPU median | CPU speedup vs Pillow |
| --- | ---: | ---: | ---: |
| I scalar, 1024×768 | 1.894333 ms | 0.128333 ms | 14.761× |
| High-cardinality signed fallback, 128×64 | 0.177041 ms | 0.179646 ms | 0.985× |

The prior unmodified run measured the high-cardinality case at 0.181354 ms CPU / 0.168500 ms Pillow. The candidate run is only 0.9% faster on CPU and still 1.5% slower than its same-run Pillow median. The scalar-I comparison is not useful as a stable control: its Pillow median moved from 0.818291 ms to 1.894333 ms between runs. No matched A/B/A result establishes a real gain. This candidate was reverted and is not accepted as an optimization.

All candidate receipts show actual CPU execution, terminal completion, one GetColors operation, and no fallback. The targeted no-adjacent parity case passed. The input contract and thresholds were unchanged. The proposal and its no-go status are recorded in Kata `pillow-rs#e1xn`.

## Artifacts

- `benchmark-result.json` — full-call two-workload candidate run.
- `benchmark-parity.json.gz` — parity gate for the candidate run.
- `no-adjacent-guard-parity.json.gz` and `no-adjacent-guard-attestation.json` — one exact guard case.
- `extension-identity.json` — source, binary, host, and run identities.
- `SHA256SUMS` — hashes of retained artifacts.
