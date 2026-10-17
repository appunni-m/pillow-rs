# GetColors I signed-run CPU trial

This local ARM64 trial evaluates the scalar-I repeated-run path against Pillow 12.2.0. The scanner preserves signed `i32` keys and uses the sampled byte-domain test only as an admission heuristic. If that specialized scanner reaches 4,097 distinct values, it abandons the attempt and restarts through the established general counter. The high-cardinality workload here has only a 64-sample repeated prefix and an 8,192-slot direct table, so it bypasses that scanner and uses the general per-pixel counter; it remains a CPU miss under read-only audit in Kata `pillow-rs#e1xn` (counter path) and `pillow-rs#reah` (Python result materialization).

The earlier bounded 0–255 histogram candidate was rejected: on its late-negative run it took 1.022 ms against Pillow at 0.767 ms because discovering an out-of-domain value forced a second scan. Subsequent Python result-materialization experiments were reverted. The measurements below use the rebuilt canonical extension and current source after that revert.

## Frozen workloads and execution

- Workloads: `pipeline-chain.terminal-read.getcolors.scalar-i-1024x768`, `pipeline-chain.terminal-read.getcolors.late-negative-signed-run-i-1024x768`, and `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`.
- Boundary: the public GetColors call and returned Python objects; the input image is materialized before timing. Five warmups, 20 iterations × 5 samples, concurrency 1, warm cache.
- Build: release; default `pillow-rs` and Python binding features. Host: macOS 15.7.7 ARM64, CPython 3.12.13; Pillow 12.2.0.
- For every workload, the receipt reports one actual `GetColors/cpu` operation, a complete terminal result, and no fallback. The benchmark gate passed 3/3; six focused parity cases passed 6/6.
- Resource counters do not include Rust result-Vec allocation or Python list/tuple allocation. Latency includes result construction. Concurrency-one throughput is reciprocal latency, not sustained throughput.

## Results

| Workload | Pillow median | CPU median | CPU p95 | CPU speedup vs Pillow |
| --- | ---: | ---: | ---: | ---: |
| I scalar, 1024×768 | 0.818291 ms | 0.117146 ms | 0.132042 ms | 6.985× |
| Late negative signed run, 1024×768 | 0.766917 ms | 0.097375 ms | 0.101958 ms | 7.876× |
| High-cardinality signed fallback, 128×64 | 0.168500 ms | 0.181354 ms | 0.208125 ms | 0.929× (CPU is 7.6% slower) |

The common I workload and late-negative edge workload are faster than Pillow in this run. The 4,097-distinct-color fallback is slower than Pillow, so GetColors I is not complete and this candidate cannot be counted as passing the CPU target. The Pillow p95 on the common workload is noisy; use these values as one host-local run, not as a portable speedup claim.

Benchmark run `migration-benchmark-bdda1b19d2e440edb60d5df5cbbf0b5d` has parity gate `migration-parity-benchmark-gate-bf026a55799d4c2da921e9bf23835cd2` (3/3). Focused parity run `migration-parity-034e673667324d27bacfbdc1cc95ab3e` passed 6/6, covering the scalar I workload, adjacent/non-adjacent run behavior, late negative, >4,096 distinct values, varied I slot order, and `maxcolors`. The benchmark selection contracts passed 38/38 during the bounded benchmark target. No full suite, broad parity campaign, or coverage ran.

The report is local dirty-tree evidence. It does not close GetColors across other modes/backends or update the published matrix. The local ledger has 7,954 data rows. No commit or push has occurred; the 33-item batch remains open.

## Artifacts

- `benchmark-result.json` — three selected full-call workloads with backend receipts.
- `benchmark-parity.json.gz` — correctness gate for those workloads.
- `focused-guard-parity.json.gz` plus the two focused attestation files — six selected guards and their execution identities.
- `extension-identity.json` — current source, extension, host, manifest, and input identities.
- `benchmark-result-two-workload-run.json`, `benchmark-parity-two-workload-run.json.gz`, `focused-guard-parity-five-case-run.json.gz`, and `extension-identity-two-workload-run.json` — preserved earlier signed-run evidence.
- `benchmark-initial-bounded-domain.json` and the `*-initial*` parity files — preserved rejected histogram attempt.
- [`getcolors-i-adjacent-runs-rejected-20261016`](../getcolors-i-adjacent-runs-rejected-20261016/README.md) — preserved and rejected follow-up coalescing trial.
- `SHA256SUMS` — hashes for all retained files in this directory.
