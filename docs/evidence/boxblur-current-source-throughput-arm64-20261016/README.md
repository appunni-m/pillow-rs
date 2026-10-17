# BoxBlur L current-source latency and queued-window diagnostic — 2026-10-16

This is a dirty-tree rebaseline and one batch-window measurement, not a kernel change or operation-completion claim. The direct benchmark and the batch harness measure different call boundaries; keep their numbers separate.

## Frozen direct latency cohort

- Workload: `pipeline-op.boxblur.material-l-noise-1024x768-radius-1`
- Parity case: `PIL.ImageFilter.BoxBlur.nuanced.performance-material-l-noise-1024x768-radius-1`
- Edge guard: `PIL.ImageFilter.BoxBlur.nuanced.backend-noise-l-65x47-radius-1`
- Direct timing boundary: `observed_steps`, `apply-filter` + `observe-filter-result`; Pillow, serial CPU, SIMD, and GPU share the same fixture input and boundary.
- Policy: warm cache, 5 warmups, 20 iterations × 5 samples (100 timed samples), concurrency 1.
- Direct benchmark parity: 3/3 target profiles passed. The material and edge cases separately passed strict CPU, SIMD, and GPU parity: 6/6 backend/case combinations.
- Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7 ARM64, Pillow 12.2.0, CPython 3.12.13.
- Target source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty; loaded `_core.abi3.so` SHA-256 `d9cb7176aadd058e07f3a3cb68b24a2e29360e08d782a9faa3aa81eff1c6b6b3`.

| Profile | Median | P95 | Pillow / target | Execution proof |
| --- | ---: | ---: | ---: | --- |
| Pillow | 1.776146 ms | 1.910584 ms | — | Pillow 12.2.0 |
| Serial CPU | 0.636104 ms | 0.715583 ms | 2.792× | Actual CPU, terminal complete, no fallback |
| Single-thread SIMD | 0.678396 ms | 0.755584 ms | 2.618× | Actual SIMD `path=vector`, 98,304 vector blocks, no fallback |
| GPU | 0.512042 ms | 1.345667 ms | 3.469× | Actual GPU, one dispatch, terminal complete, no fallback |

GPU median latency is 1.325× faster than SIMD. Its P95 is 1.78× slower than SIMD, so this does not establish matched tail latency. The GPU receipt records one 786,432-byte upload, one 786,432-byte readback, and a 256-byte parameter block. Allocation, transfer, synchronization, and materialization remain inside the measured operation path.

The earlier local BoxBlur receipt in `boxblur-l-gpu-current-arm64-20261016/` used a different extension hash (`dedbed…`) and lower speedups. This run uses a rebuilt current extension; without an identity-matched bracket, the movement is not attributed to a source change.

## Explicit batch-window throughput

- Selected row: `pipeline-batch.boxblur.l-1024x768`
- Boundary: completed window from graph/image construction through submission, execution, result materialization, and byte export.
- Policy: 2 warmup windows, 20 measured windows, 32 distinct deterministic inputs per window, `max_jobs=16`, `max_in_flight=4`.
- Exact byte comparison passed for all 32 outputs per subject; the report verifies repeated windows remain stable. GPU queued submitted 704 jobs, dispatched once per job, grouped them into 176 queue submissions (largest group 4), uploaded and read back 553,648,128 bytes, and finished with zero live jobs/bytes.

| Subject | Completed images/s | Ratio vs Pillow |
| --- | ---: | ---: |
| Pillow | 536.5 | 1.000× |
| Serial CPU | 1,519.0 | 2.831× |
| Single-thread SIMD | 1,432.2 | 2.670× |
| GPU eager | 520.9 | 0.971× |
| GPU queued | 2,117.9 | 3.948× |

Queued GPU completed-window throughput was 1.479× SIMD. This is a measured batch-window result, not long-duration GPU throughput. The executor's `max_in_flight`, submission grouping, dispatch count, and completion counters prove queued work and GPU dispatches; they do not prove overlapping device kernels. The harness marks this distinction explicitly. Do not credit the full sustained-throughput goal without a longer stable workload and device execution timestamps/profiler evidence if the claim requires kernel overlap.

A separate one-image `benchmark_gpu_batch.py --subject ...` diagnostic used the same input digest across Pillow/CPU/SIMD/GPU-eager. Its full construction/filter/export medians were 1.916/0.604/0.727/2.029 ms. That is the explicit executor boundary; GPU eager is slower than SIMD there. Keep it separate from the direct `Image.filter` path above. It does not negate the queued 32-image throughput result.

## Harness selector defect

The multi-subject invocation supplied singular `--mode L --size 1024` but omitted `--modes L --sizes 1024`. In cohort mode, `main()` ignores the singular selectors and used defaults, emitting four workloads: L and RGB at widths 256 and 1024 (height 768). Only the exact L 1024×768 row above is selected; the other three rows remain in the raw report and receive no campaign credit. For a future exact run, pass `--modes L --sizes 1024` and verify the emitted workload IDs before interpreting results. Kata issue `pillow-rs#6yv9` tracks a focused selector-contract proposal; the script was not changed and the checks were not weakened.

The all-subject one-image mode also reaches the queued lane's `largest_submission > 1` assertion. Preserve that assertion for throughput; single-image latency requires per-subject invocations or a distinct harness contract.

## Disposition

This one L-mode size passes CPU ≤ Pillow, SIMD ≥ 2× Pillow (interim checkpoint; still below the repository's final 5× SIMD target), and GPU median latency ≤ SIMD. It does not prove GPU P95 parity, long-duration sustained throughput, other modes/sizes/radii, Pillow-SIMD, Parallel CPU, or the full operation matrix. BoxBlur remains open. The local-run ledger gained four direct-profile rows and five selected batch-window rows (7,997 data rows total); the 33-item commit batch remains 0/33. No coverage, commit, or push ran.

## Retained artifacts

`SHA256SUMS` covers all compressed reports, samples, parity outputs, identities, and the exact-row derived view. `boxblur-batch-full-report.json.gz` preserves the original four-workload output; `selected-boxblur-l-1024x768-view.json.gz` contains only the unchanged selected row.
