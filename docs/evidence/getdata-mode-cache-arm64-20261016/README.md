# GetData repeated-mode-lookup candidate evaluation — ARM64, 2026-10-16

The candidate read `self.mode` once after `getdata_formatted(band)` and reused that local when choosing the result mode. Existing unrelated `Image.show` work in the dirty checkout was preserved. After a second mode-specific bracket showed no general benefit, the candidate was rejected and the exact control source was restored.

## Primary small L case: source-matched A-B-B-A

The selected workload was `pipeline-chain.terminal-read.getdata.l-band0-32x32`, case `PIL.Image.Image.getdata.performance.material-l-band0-32x32`, profile `python-cpu`, with the `observed_steps: band-data` boundary. Every cohort used warm cache, five warmups, 20×5 measured calls, and concurrency one.

| Cohort | Pillow median | CPU median | Pillow/CPU | Parity |
| --- | ---: | ---: | ---: | --- |
| A1 control | 10.541 µs | 5.375 µs | 1.961× | 1/1 pass |
| B1 candidate | 10.417 µs | 4.791 µs | 2.174× | 1/1 pass |
| B2 candidate | 9.958 µs | 5.125 µs | 1.943× | same candidate proof reused after identity match |
| A2 control | 9.834 µs | 5.250 µs | 1.873× | same control proof reused after identity match |

Both candidate CPU medians were below their adjacent controls. The mean of cohort medians was 4.958 µs for B versus 5.313 µs for A, a 6.7% diagnostic improvement. B2 remains below 2× Pillow, so this does not demonstrate a stable 2× result or close GetData.

The loaded `pillow_rs.image` hashes were `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d` for both controls and `5afb7c5d9486c58fca58eb70065a6ecb469781b993a6341272d557e74a5bb922` for both candidates. All four loaded the same `_core` extension SHA-256 `e7a1ce76dd97149c6a86b0b69db926fee58b2b1a93740cbd60fe0ee6d6f6839e`. Host: Lazys-MacBook-Pro.local, macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0. The checkout was dirty at revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.

## Mode and size guards

A fresh focused parity preflight passed all four selected cases. Each benchmark receipt reported one completed `GetData` operation on actual CPU, terminal completion, and no fallback.

| Workload | Pillow median | CPU median | Pillow/CPU | Parity |
| --- | ---: | ---: | ---: | --- |
| L 1024×768 | 5.134 ms | 2.334 ms | 2.199× | pass |
| 1 1024×768 | 5.127 ms | 2.364 ms | 2.168× | pass |
| P 1024×768 | 5.085 ms | 2.400 ms | 2.119× | pass |
| RGB 32×32 | 9.958 µs | 5.375 µs | 1.853× | pass |

The large L/1/P workloads exceeded 2× in candidate-only guards, but have no matched controls and cannot attribute a gain to this line. The RGB candidate-only result was followed by the source-matched bracket below. SIMD, Parallel CPU, GPU, other sizes/modes, and native x86 public execution remain open. This local diagnostic does not update the published operation matrix.

## Cross-mode RGB small-case guard and rejection

Because the candidate-only RGB guard measured below 2× Pillow, a fresh source-matched A-B-B-A used the same RGB 32×32 workload and sampling policy:

| Cohort | Pillow median | CPU median | Pillow/CPU | Parity |
| --- | ---: | ---: | ---: | --- |
| A1 control | 10.542 µs | 5.625 µs | 1.874× | 1/1 pass |
| B1 candidate | 10.459 µs | 5.792 µs | 1.806× | 1/1 pass |
| B2 candidate | 10.542 µs | 5.500 µs | 1.917× | reused B1 proof after identity match |
| A2 control | 10.417 µs | 5.500 µs | 1.894× | reused A1 proof after identity match |

The candidate mean CPU median was 5.646 µs versus 5.563 µs for control, about 1.5% slower. B1 lost to adjacent A1 and B2 tied A2. The L 32×32 bracket showed a 6.7% candidate improvement, so results are mode-dependent and do not support retaining the generic wrapper change. The candidate was rejected; current `image.py` was restored to control hash `16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`.

The timer includes public `band-data` result construction and materialization. Runtime host-allocation telemetry reports zero for the Python-owned `PyBytes` output, so those allocation counters are incomplete for this call path.

## Receipts

L primary run IDs: A1 `migration-benchmark-986df8e8c7de400e8e2e970e28796c6e`; B1 `migration-benchmark-36f4cf5872b748c89aad175a07407686`; B2 `migration-benchmark-2ddcfafb7395441fb00039ea8689eca1`; A2 `migration-benchmark-0e356d5c933843c1a821d43b7abfecf4`. RGB cross-mode run IDs: A1 `migration-benchmark-0857c8089b804aa0926d9dd74e641f93`; B1 `migration-benchmark-251b69333ac3422ca48956105708815e`; B2 `migration-benchmark-2bf3c9f1042d4bd39daf54a1509c3fb8`; A2 `migration-benchmark-f05bc032472f408b818915d0345ea915`. Candidate-only mode guard run: `migration-benchmark-d7c13bba20b8405781972d21255407a9`; parity gate `migration-parity-benchmark-gate-5f0d73e153044830b23ada28e45aa344` (4/4).

Guard artifacts:

- Report SHA-256: `663220fcc14fbd6060e350b2112aaebf484de20df7bfee068668366383ee2b0d`
- Parity SHA-256: `2f0e4e2e8d908db10acb507f76a38cc0347c976911b9248e9f017ca19c0c8e05`
- Raw samples SHA-256: `c20d2b4294501b36d1ec3846d1487fa42c089f5d9770bab6294009e141f75770`
- Execution attestation SHA-256: `f8fc0653cb235ae75ca65b763246a5c4bf135cc062aa57edbbd0a3565f3cc238`
- Producer attestation SHA-256: `63e4e785c385df3ad135c83917a4e33575e3778c0b1eeb976bb36f41dfc315ca`

The local-run ledger records both A-B-B-A brackets and the four candidate-only guards as Pillow/CPU subject rows (24 rows total). No test defect was found. No full campaign, coverage, commit, or push ran.
