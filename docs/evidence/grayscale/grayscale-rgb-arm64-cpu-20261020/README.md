# RGB Grayscale strict CPU baseline (2026-10-20)

This evidence covers one warm, materialized 1024×768 RGB public-call workload on
the ARM64 macOS runner `lazys-macbook-pro-arm64`. The source checkout was dirty
at revision `418d5f0cde51ca64a97cbd007ea731da72d68abf`; no image-kernel source
change was made for this baseline.

- Workload: `pipeline-op.grayscale.material-rgb-noise-1024x768`
- Parity case: `PIL.ImageOps.grayscale.nuanced.performance-large-rgb-noise-1024x768`
- Oracle: Pillow 12.2.0, CPython 3.12.13
- Target: `python-cpu`, exact selected parity 1/1
- Boundary: `call;observe-result`, 5 warmups, 20 iterations × 5 samples,
  concurrency 1
- Loaded `_core.abi3.so`: SHA-256
  `b884c995c1496014d49321efbb5c228ab5b08e794ad8021a1fd8ec29ac754c24`
- CPU receipt: actual backend CPU, terminal complete, one Grayscale operation,
  no fallback, one mode conversion, one 786,432-byte output allocation

## Baseline and strict candidate check

The seeded run measured Pillow at 161.875 µs median / 212.125 µs p95 and CPU at
98.750 µs / 125.917 µs (1.639× Pillow). PerfDB admitted it as the strict
one-workload channel `grayscale-rgb-arm64-cpu-20261020`.

A fresh candidate run with the same source and workload measured Pillow at
222.542 µs / 263.375 µs and CPU at 94.792 µs / 101.084 µs (2.348× Pillow).
The generated strict check passed: CPU median −4.008%, p95 −19.722%, and
throughput +4.175% against the promoted baseline. The CPU target stayed near
its baseline while Pillow timing moved substantially, so the ratio difference
is not evidence of a kernel speedup. The candidate's within-report ratio clears
2×; this workload remains below 2× in the baseline report. SIMD, GPU, other
modes, sizes, architectures, and Pillow-SIMD were not measured here.

The selected run packages under `perfdb-selected-python-module/` and
`candidate-fresh/perfdb-selected/` carry the validated report, parity,
execution, producer, manifest, input, and materialized-build links. The
generated DuckDB remains at
`/Users/lazytrot/work/pillow-perfdb/performance-results.duckdb`; the strict
channel is reproducible from those selected-run packages.

## Proof workflow notes

On this macOS host, the direct Cargo build witness needs
`RUSTFLAGS="-C link-arg=-undefined -C link-arg=dynamic_lookup"` to link the
PyO3 extension. PerfDB accepts the locked Maturin command through the selected
Python (`.venv/bin/python -m maturin develop ...`); a standalone Maturin
executable receipt is rejected by the current source/build validator.

Re-materializing the same extension bytes replaces the file inode. The runtime
fingerprint therefore changes even though the path, size, and SHA-256 stay the
same, and the harness correctly rejects reusing parity from before that
materialization. This candidate was measured with fresh parity after the final
materialization. The `perfdb-selected/` and `candidate/` directories retain
rejected proof attempts; they are diagnostics, not admitted evidence.

No code optimization is claimed. Keep this strict baseline scoped to the one
RGB workload and leave the Grayscale operation open for remaining modes,
profiles, architectures, and SIMD/GPU goals.
