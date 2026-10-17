# L-mode Posterize GPU/SIMD diagnostic — 2026-10-16

## Scope

One current-source workload only: `PIL.ImageOps.posterize(image, bits=4)`, L mode, 1024×768. The direct benchmark selected `pipeline-op.posterize.material-l-noise-1024x768` and its exact parity case. A separate completed-window run compared serial SIMD with the queued GPU batch API over 32 inputs, two warm-up windows, and 20 measured windows. No Rust code changed.

## Identity

- Worktree revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`; dirty: `true`.
- Loaded extension: SHA-256 `315a2f3278395a2a2f9ae65221ad2fcbdb2cc463558931c0acd3ca35334ce198`; target package version 12.2.1; Rust `parallel` feature disabled.
- Reference: Pillow 12.2.0 from the isolated `.venv`; target Python 3.12.13.
- Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7, ARM64.
- Manifest input asset: SHA-256 `d9cc88816a9cab2ca448b29afb517357ea1639fffeee3ad11ed4802d4102e608`; exact parity case `PIL.ImageOps.posterize.nuanced.performance-large-l-noise-1024x768`.
- Direct benchmark run: `migration-benchmark-8580da9893db4238b133e7119bac3dd0` (2026-10-16T12:08:10.936113Z–2026-10-16T12:08:35.806662Z UTC).
- Direct parity gate: `migration-parity-benchmark-gate-ea1bc127ed484210beb4a4c8ae424965`; exact bytes passed for CPU, SIMD, and GPU (3/3).
- GPU tail guard: `PIL.ImageOps.posterize.nuanced.gpu-native-byte-tail-l-3x1-bits-4`; exact parity passed for CPU, SIMD, and GPU (1/1 each). See the per-profile reports and execution attestations.

## Direct public-call latency

The manifest boundary includes `call;observe-result`, with 5 warm-ups, 20 measurement iterations, 5 samples per iteration, and 100 observed calls per subject. Median and p95 are in microseconds.

| Subject | Median | P95 | Median speedup vs Pillow | Executed backend |
| --- | ---: | ---: | ---: | --- |
| Pillow 12.2.0 | 382.958 | 426.208 | 1.000× | Pillow |
| Serial CPU | 84.146 | 107.709 | 4.551× | CPU, terminal complete, no fallback |
| SIMD | 80.104 | 98.084 | 4.781× | SIMD, terminal complete, no fallback |
| GPU | 328.145 | 606.042 | 1.167× | GPU, terminal complete, no fallback |

GPU/SIMD latency speedup is **0.244×**: the direct GPU call is 4.10× slower than SIMD. Its receipt records one actual GPU dispatch, 786,432 upload bytes, 786,432 readback bytes, one full-frame copy, and one host allocation of 786,432 bytes. SIMD executed `vector` with 49,152 vector blocks and no scalar tail.

## Queued completed-window throughput

Each window constructs 32 fresh L images and Posterize graphs, executes them, drains GPU work, materializes CPU outputs, calls `tobytes()`, and completes synchronization. Input generation and exact byte comparisons are outside the timed windows; the persistent executor is constructed before timing. Every timed output was compared byte-for-byte with Pillow 12.2.0 (704 checks per backend).

| Subject | Median completed images/s | P95 window | Exact checks | Backend |
| --- | ---: | ---: | ---: | --- |
| SIMD | 13638.059 | 3.372 ms | 704 | actual SIMD, 49,152 representative vector blocks |
| Queued GPU | 1978.578 | 17.622 ms | 704 | actual GPU, no fallback |

GPU/SIMD completed-window throughput is **0.145×**. The GPU submitted 704 jobs in 176 submissions (largest group 4), with 553,648,128 upload bytes and the same number of readback bytes. All jobs and resources were drained. This completed-window result is below SIMD; it is not a sustained-duration or kernel-overlap claim.

## Disposition

CPU is faster than Pillow. SIMD clears the user's 2× screening floor, though this dirty ARM64 measurement is narrowly below the repository's stricter 5× SIMD target. GPU is slower than SIMD on the same ARM64 call boundary, and queued GPU throughput is 6.89× below SIMD. Pillow-SIMD was not measured on this host, so no same-host GPU/Pillow-SIMD comparison is claimed. Do not spend another kernel experiment on this L Posterize GPU route; move to the next distinct GPU operation. The current source path has real SIMD vector work and the GPU path has a real shader dispatch, so neither timing is a fallback measurement.

This is a dirty-tree ARM64 diagnostic, not release-credit evidence. The direct benchmark JSON, compact byte-parity receipt, report-bound attestations, three edge-profile receipts, and batch output are stored beside this note. The original 12.6 MB parity payload and generated oracle bytes remain in `/tmp`; the compact parity receipt records the full report SHA-256 `31ab8dcfa3fbc3f73168e4ce959d49e1045f512c28245fd2552e926110d6147c`.
