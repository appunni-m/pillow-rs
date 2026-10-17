# Filter5x5 L vertical-total recurrence — ARM64, 2026-10-16

Status: **keep as a scoped CPU candidate** for the tested native L, normalized all-ones 5×5 kernel, scale 25, offset 0, and 1024×768 workflows. This does not complete Filter5x5 across modes, kernel shapes, sizes, SIMD, Parallel CPU, or GPU.

The L fast path retains five horizontal `u16` rows and adds one vertical total per column. Each output row uses that total; after output, the outgoing ring row is replaced and each total is updated by subtracting the outgoing horizontal sum and adding the incoming one. The exact `(sum + 12) / 25` rounding, native L bytes, copied two-pixel border, and fallback predicates remain unchanged. At width 1024, the additional vertical-sum row is 2,048 bytes.

## A–B–B–A timings

Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7 ARM64; Pillow 12.2.0; release build; warm cache; concurrency 1. Values are per-run medians. The CPU receipt reports actual `cpu`, terminal completion, and no fallback in every target run.

| Leg | Workload | Pillow (ms) | pillow-rs CPU (ms) | Pillow / CPU |
|---|---|---:|---:|---:|
| A1 | `pipeline-chain.convolution.native.l-5x5-scale.1024x768` | 4.159667 | 2.320000 | 1.793× |
| B1 | same full workflow | 4.331062 | 1.261084 | 3.434× |
| B2 | same full workflow | 4.360834 | 1.263604 | 3.451× |
| A2 | same full workflow | 4.726521 | 2.458209 | 1.923× |
| A1 | `pipeline-op.filter.material-l-noise-flat-5x5-1024x768` | 3.606855 | 1.566896 | 2.302× |
| B1 | same varied-data filter + materialize | 3.667292 | 0.540313 | 6.787× |
| B2 | same varied-data filter + materialize | 3.619063 | 0.549521 | 6.586× |
| A2 | same varied-data filter + materialize | 3.981688 | 1.728646 | 2.303× |

The mean of the two control medians divided by the mean of the two candidate medians is **1.893×** for the full workflow and **3.024×** for varied-data filter + materialize. The corresponding candidate means are 1.262344 ms and 0.544917 ms; control means are 2.389104 ms and 1.647771 ms.

The full workflow used 1 warmup × 3 iterations × 2 samples (6 samples), and includes setup, Filter5x5, Scale, and terminal `tobytes` materialization. The varied-data workload used 5 warmups × 20 iterations × 5 samples (100 samples), boundary `call;materialize`. These contracts were not changed. The matching Pillow medians are shown per run to account for host drift.

## Parity and identity

Focused L workflow parity passed 1/1 on A1 control, B1 candidate, and A2 control for `convolution-uniform-color-trial-20261014.workflow.native-l-scale-1024`. The varied-data filter parity case `PIL.Image.Image.filter.nuanced.l-mode-flat-kernel-5x5-noise-1024x768` also passed 1/1 on each leg. B2 reused B1's exact candidate proof; B1 and B2 used the same extension hash. The candidate rebuild at the end reproduced that extension hash.

- Control `filter.rs` SHA-256: `ef8915c87e1098783dca56858d71e0cab61a6af42b74760b6b19648aa07a17fd`
- Control `_core.abi3.so` SHA-256: `2fc9fba649933e195c08b95312f948e0757190f62ca6978c38b7688d04081c95`
- Candidate `filter.rs` SHA-256: `4f174d73233dbdf74c4a2d6d7e91b0bc2c5e18bd49bddb0dd32cb2218cb95dbf`
- Candidate `_core.abi3.so` SHA-256: `88b86723994eb2ac0f15cea0424215bc47e5d5a9f8fe201e677c339e49c1132d`
- Shared Git revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` (dirty tree in all runs)
- Focused Rust contract: `cargo test -p pillow-rs --lib uniform5x5_native_luma_tests -- --nocapture` — 1 passed, 595 filtered.

The selected receipts account for 2 or 3 image buffers, but runtime `host_allocation_count` and `host_allocated_bytes` report zero and do not expose the extra 2 KiB scratch separately. Treat allocation counters as incomplete; latency still includes the public call and its allocations/materialization. No SIMD, Parallel CPU, or GPU measurement was made. No genuine test defect was found. One initial uniform benchmark invocation requested parity reuse against a `successful_execution` correctness gate and selected zero parity cases; it was discarded, then explicit parity was run separately. This is a runner contract, not a product or test defect.

Raw benchmark reports, parity results, attestations, and samples remain in ignored local `build/migration-parity/filter-luma-vertical/`. The 16 subject rows are in [`../performance-optimization-local-runs.csv`](../performance-optimization-local-runs.csv). This evidence is local/dirty-tree and earns no full-operation completion credit.
