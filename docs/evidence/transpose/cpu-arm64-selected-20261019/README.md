# RGBA Transpose selected CPU rebaseline — ARM64, 2026-10-19

## Result

The current local ARM64 CPU implementation passes this selected materialized
Transpose workload at **2.142× Pillow** (Pillow/CPU median latency).
This clears the serial CPU ≤ Pillow target for this case on ARM64. It does not
resolve the separate x86_64 CPU miss or the SIMD comparison; neither backend
was run here.

- Workload: pipeline-matrix.transpose-tiled.rgba-772x768-transpose-mirror.operation-materialized
- Operation: PIL.Image.Image.transpose, RGBA 772×768.
- Boundary: transpose-0;transpose-1;materialize; full observed public call,
  including output allocation and tobytes materialization.
- Profile: python-cpu; requested and actual backend cpu; no fallback;
  terminal complete; two operations.
- Host: macOS-15.7.7-arm64-arm-64bit; CPython 3.12.13; Pillow 12.2.0.
- Release profile, warm cache, concurrency 1; 5 warmups, 20×5 samples
  (100 observations per subject).

| Subject | Median latency (ms) | P95 latency (ms) | Median throughput (calls/s) |
| --- | ---: | ---: | ---: |
| Pillow | 1.012334 | 1.390542 | 987.816 |
| pillow-rs CPU | 0.472645 | 0.617333 | 2115.751 |

Two exact parity cases passed: the aligned 772×768 main case and the odd
1025×1023 guard. The benchmark reused that same-build proof; it did not repeat
either parity case.

## Identity and retained evidence

- Benchmark run: migration-benchmark-3e505d3512bd421ca9ffd19c60e7d0b7; report SHA-256
  839c64948c6c79534bf330e67d53469f9f8376c45a228f95778f829cbdbb4580.
- Selected parity run: migration-parity-0a829ca376b541c5b111982e7a9cabbd; compact outcomes and
  full report SHA-256 are in parity-summary.json. The 33 MB full parity report
  remains under /tmp rather than being copied into repository evidence.
- Target runtime identity SHA-256: ae3789c214dd026662872908b510f946205a902768dbce14780076e6abb13397.
- Loaded _core.abi3.so SHA-256: 84a42503697d72038ac9fb0c3d260a025f476b0aba7e62fe7fb888f442a09bcc.
- Current source hashes: geometry.rs 83e261db28927037b9cf461be6f1b911932bec9648b9567de5cabe2e07f38017, image.rs 423f9d54b312afb31fdf10b294309fafcf12e5a375ece9887d2b2e238db547e3,
  registry.rs c85deb6f6ba7467003c87a01238b497ec9d6bf5676a930460ac00f26ce780dd1, and PyO3 lib.rs 7022046e9b51a4a19aeca71369a4c2da6a5a6576776016e12699b90aa6fec1fc.
- Revision: 418d5f0cde51ca64a97cbd007ea731da72d68abf (dirty worktree).
- The benchmark report normalizes identity.command.argv to
  make migration-parity-benchmark. This run was invoked directly with
  .venv/bin/python scripts/run_migration_benchmark.py, one explicit workload,
  CPU-only backend, and report-bound parity reuse. The normalized field omits
  runtime selection flags; this README records the actual invocation.

No full benchmark, full parity, or coverage campaign ran.
