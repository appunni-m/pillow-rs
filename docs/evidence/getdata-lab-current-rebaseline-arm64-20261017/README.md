# LAB band-2 GetData current-source rebaseline — ARM64, 2026-10-17

Status: **no optimization trial needed for this workload**. The historical
1.9× serial CPU/Pillow result no longer reproduces on the current dirty source.
This focused result clears the 2× checkpoint for LAB band 2 only; GetData's
other modes, sizes, and backends remain open.

## Measurement and parity

The frozen workload was
`pipeline-chain.terminal-read.getdata.lab-band2-1024x768`, using LAB 1024×768,
warm release, concurrency one, 5 warmups, and 20 iterations × 5 samples. The
timed `band-data` boundary includes the public GetData result and byte
materialization. Strict serial CPU parity passed the exact case
`PIL.Image.Image.getdata.performance.material-lab-band2-1024x768` (1/1).

Pillow 12.2.0 measured **5.435542 ms** median and serial CPU measured
**2.478188 ms**, for **2.193× Pillow/CPU**. The report contains 100 timed
samples per subject; CPU p95 was 2.824042 ms versus 6.480834 ms for Pillow.
The target receipt confirms actual CPU execution, terminal completion, one
GetData operation, and no fallback.

| Evidence | Run ID | SHA-256 |
|---|---|---|
| Benchmark | `migration-benchmark-366c91b8b8214211bb29703c099f81cf` | `6ad6dc31a8370cf8e442458088cecd22ff53f86147f65b39eed47b76a8fb1786` |
| Exact parity | `migration-parity-48e9e9e6cf374717aea9369abc75d348` | `499b5548ef1a2d9f9855ed56d7b2c96521c4beedf91ab91fb75d4aa789faaeec` |

The loaded `_core` SHA-256 is
`bdafdc95d73debbd2cb9c8969bccdc2b079ede28bb1c8a0e2c1c6e2d80a62360` for both
benchmark and parity. The checkout is dirty at revision
`9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, so this updates current-source
understanding but does not replace the clean published operation matrix.

The full reports, raw samples, execution attestations, producer receipts, and
their child artifacts are stored beside this README. Their two Pillow/CPU rows
are in [`../performance-optimization-local-runs.csv`](../performance-optimization-local-runs.csv).
The prior LAB band-2 optimization proposal was screened read-only in Kata
`pillow-rs#9as2`; the matching GetData mode-rank issue is `pillow-rs#t0qm`.
No code changed, no coverage ran, and no commit or push ran; the batch remains
0/33.
