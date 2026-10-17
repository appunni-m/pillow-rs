# GetColors Direct-table finalizer A/B/B/A — rejected (ARM64, 2026-10-17)

Tried writing final `(count, FormattedPixelValue)` rows directly while scanning occupied Direct-table slots, avoiding the intermediate `(slot, (pixel, count))` vector. Sparse-table finalization and the sample counter were unchanged. The exact source-matched public workload was `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`; it used 5 warmups, 20 iterations × 5 samples, warm cache, concurrency one, and 100 measured samples. I/F 1024×768 no-adjacent workloads were retained as guards. Each measured target completed actual CPU execution with no fallback.

| Run | CPU median (µs) | Pillow median (µs) | CPU p95 (µs) | Pillow p95 (µs) |
| --- | ---: | ---: | ---: | ---: |
| A1 control | 185.250 | 178.938 | 195.208 | 186.500 |
| B1 candidate | 189.542 | 178.000 | 203.250 | 184.208 |
| B2 candidate | 187.312 | 175.854 | 195.334 | 231.917 |
| A2 control | 181.333 | 176.625 | 316.042 | 180.833 |

Candidate CPU medians average 188.427 µs versus 183.292 µs for controls, 2.80% slower. Both candidate medians were slower than both control medians. The Pillow/CPU ratios were 0.939× and 0.939×. The candidate therefore fails its primary performance test and was rejected despite modest I/F guard reductions.

| Guard | A1 CPU (µs) | B1 CPU (µs) | B2 CPU (µs) | A2 CPU (µs) |
| --- | ---: | ---: | ---: | ---: |
| I no-adjacent | 480.875 | 476.021 | 477.125 | 481.083 |
| F no-adjacent | 861.520 | 856.688 | 857.980 | 865.229 |

Exact parity passed 4/4 on A1, B1, and A2. B2 reused B1’s identity-matched passing parity report. The selected proofs cover the 4,096/4,097 boundary with the trailing signed negative, varied I slot order, late negative I, and the shared no-adjacent I/F loop. The guard medians moved slightly faster on B, but those shifts are too small and noisy to offset the primary slowdown.

Baseline source SHA-256: `dcd38ebeeba2b76b0c65ee1430b109833af158bc11a932c451716f3417cd07c8`. Candidate source SHA-256: `c7b0062daa8a3068a5c2a559de9a24c91876eb224ebf45f41915b02ead2291ce`. Full benchmark reports, raw samples, producer/execution attestations, and parity reports are retained in the A1/B1/B2/A2 directories. The 24 subject rows are in `../performance-optimization-local-runs.csv`; the ledger now has 8227 data rows.

The candidate was removed and the A source rebuilt with `make build-parity`. The earlier Direct/Sparse dispatch-hoist improvement remains in the source. This trial adds no operation completion credit and no item to the 33-item commit batch. No coverage, commit, or push ran.
