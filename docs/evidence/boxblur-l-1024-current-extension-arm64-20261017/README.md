# BoxBlur L current loaded-extension rebaseline — 2026-10-17

This current-source ARM64 measurement is a rebaseline, not a kernel change or completion claim. It replaces no earlier cohort; the extension SHA is recorded so this result stays bound to the binary that executed.

## Direct full-call latency

- Workload: pipeline-op.boxblur.material-l-noise-1024x768-radius-1
- Material parity case: PIL.ImageFilter.BoxBlur.nuanced.performance-material-l-noise-1024x768-radius-1
- Edge guard: PIL.ImageFilter.BoxBlur.nuanced.backend-noise-l-65x47-radius-1
- Direct boundary: apply-filter plus observe-filter-result, with warm cache, 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Material parity gate passed all three target profiles. The edge guard separately passed CPU, SIMD, and GPU (1/1 each).
- Host: Lazys-MacBook-Pro.local, macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0.
- Source revision 9230f5c55ddfe1eab865c1aa5e05ab7b87241c31 (dirty); loaded _core.abi3.so SHA-256 3896b49bbf7375c727f1b3d65af7b527ba4f08d6ba7c5e8e078913fbd3e619ff.

| Profile | Median | P95 | Speedup vs Pillow | Execution proof |
| --- | ---: | ---: | ---: | --- |
| Pillow | 1.868125 ms | 4.162750 ms | 1.000× | Pillow 12.2.0 |
| Serial CPU | 0.651480 ms | 1.388917 ms | 2.867× | Actual CPU, terminal complete, no fallback |
| Single-thread SIMD | 0.620209 ms | 0.829500 ms | 3.012× | Actual SIMD vector path, 98,304 blocks, zero tail, no fallback |
| GPU | 0.902167 ms | 1.031750 ms | 2.071× | Actual GPU, one fused dispatch, terminal complete, no fallback |

GPU latency is 0.687× SIMD latency (GPU is 1.455× slower). Its receipt records one 786,432-byte upload and readback, 256 parameter bytes, and one full-frame copy. The public result must be a CPU-owned image; mapped GPU bytes are copied into a newly allocated L buffer before byte export. The current path already fuses both blur axes in one dispatch and uses the mapped native input path. The measured call therefore includes the required GPU completion, mapped readback, materialization, and output observation.

A prior direct run on 2026-10-16 used a different extension SHA and measured a 0.512 ms GPU median versus 0.678 ms SIMD. The relevant BoxBlur GPU source path is unchanged in the current diff. The runs are still not a matched extension/environment bracket, so no source attribution is warranted.

## Completed-window throughput

- Workload: pipeline-batch.boxblur.l-1024x768.
- Boundary includes construction of images and graphs, submit, execution, materialization, and byte export.
- 32 images per window, 2 warmup windows, 120 measured windows, max_jobs=16, max_in_flight=4.
- All 32 outputs per subject compared byte-for-byte across subjects and measured windows.
- Every target subject used the same loaded extension SHA listed above; GPU results were actual GPU, terminal complete, and had no fallback.

| Subject | Completed images/s | Ratio vs Pillow |
| --- | ---: | ---: |
| Pillow | 532.866 | 1.000× |
| Serial CPU | 1,519.689 | 2.852× |
| Single-thread SIMD | 1,574.585 | 2.955× |
| GPU eager | 456.631 | 0.857× |
| GPU queued | 1,970.039 | 3.697× |

Queued GPU completed-window throughput is 1.251× SIMD. It submitted and dispatched 3,904 jobs in 976 submissions, largest group four, uploaded and read back 3,070,230,528 bytes, and finished with zero live jobs or bytes. This is about 1.95 seconds of measured windows, not long-duration sustained-throughput evidence and not proof of overlapping device kernels. Eager GPU throughput is below SIMD.

## Disposition

Keep the operation open. CPU is faster than Pillow, while SIMD remains below the final 5× Pillow target. The direct GPU latency target fails despite queued throughput exceeding SIMD. The current source already uses one fused dispatch and mapped upload; the report does not split device execution from map wait, decode/copy, image construction, and bytes export. Do not spend another shader variant on this case without phase profiling that proves device execution is the dominant gap. Kata issue pillow-rs#7gmy records the source/build-matched audit and next profiling question.

The local results ledger contains four direct rows and five batch rows. No kernel changed, and no coverage, commit, or push ran.

## Retained artifacts

SHA256SUMS covers this directory's reports, parity outputs, and attestations. The direct report retains raw samples and child attestations; each edge-parity report retains its child attestations. The batch report retains all 120 windows and exact output digests.
