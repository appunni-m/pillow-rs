# GaussianBlur RGBA horizontal row fusion: source-matched A–B–B–A

This run isolates the retained serial CPU RGBA horizontal row-fusion route in
`pillow-rs/src/compute/pool_cpu/ops/filter.rs`. The control and candidate differ
only in the `rgba_gaussian_row_fusion` dispatch condition. The original route
uses two full-frame horizontal intermediate buffers; the candidate processes
each row through the same three `blur_line` passes with two reusable row
buffers.

## Frozen workload and proof

- Workload: `pipeline-chain.blur-material.gaussian-rgba-1024x768`
- Exact parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-gaussian-1024x768-radius-7-5`
- Input: 1024×768 RGBA, constant `[83, 149, 17, 191]`, radius 7.5
- Boundary: `whole_workflow`, including setup, GaussianBlur, and `tobytes`
- Profile: serial `python-cpu`, release/default features, warm cache, concurrency 1
- Policy: one warmup, 3 iterations × 2 samples (six measured samples per subject)
- Host: macOS 15.7.7 ARM64, CPython 3.12.13, Pillow 12.2.0
- Source revision: dirty `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`
- Control `filter.rs` SHA-256: `1bcb15f2a51fcb5af6f02169a7442e5a446868bf9de268d4bbdf6ec836a19015`
- Candidate `filter.rs` SHA-256: `d1dc406edf80f0ac343897f58b12c473f3a26f6131fbe758bddb3b3d1332d779`

Each of A1, B1, B2, and A2 selected only this workload. Each parity gate passed
1/1. Every target timing has an actual CPU receipt, a complete terminal
materialization, and no fallback. A1/A2 loaded the same `_core.abi3.so`
SHA-256 `2754a14fd0d8661b59d1f0a6c7018493e3e11eec8b7374443ff7fa09277904ce`;
B1/B2 loaded the same candidate SHA-256
`d9cb7176aadd058e07f3a3cb68b24a2e29360e08d782a9faa3aa81eff1c6b6b3`.
Benchmark and parity input digests, manifest, runtime, profile, workload policy,
and host match across all four runs.

## Results

| Run | Route | Pillow median | CPU median | Pillow / CPU |
| --- | --- | ---: | ---: | ---: |
| A1 | Control | 5.891 ms | 10.915 ms | 0.540× |
| B1 | Row fusion | 6.034 ms | 7.729 ms | 0.781× |
| B2 | Row fusion | 5.820 ms | 7.333 ms | 0.793× |
| A2 | Control | 6.122 ms | 11.039 ms | 0.555× |

Mean target latency fell from 10.977 ms in the control legs to 7.531 ms in the
candidate legs, a 1.458× improvement over control. Both candidate legs beat
both controls. The candidate remains 1.26–1.28× slower than same-run Pillow,
so this is a retained partial improvement and does not clear the CPU/Pillow or
2× sequencing target. The workload has reciprocal single-request throughput
only; it provides no sustained-throughput result.

## Retained artifacts

Each run's benchmark report, attestation, producer records, raw samples, and
parity attestation are stored beside this README. Full parity reports are
gzip-compressed. `SHA256SUMS.txt` checks every stored artifact. The local run
ledger has eight appended Pillow/CPU rows with the A–B–B–A roles and execution
receipts.

No SIMD, Parallel CPU, GPU, Pillow-SIMD, or clean-published-runner claim follows
from these serial ARM64 measurements. Do not try another RGBA Gaussian kernel
variant in this visit; move to the next ranked operation.
