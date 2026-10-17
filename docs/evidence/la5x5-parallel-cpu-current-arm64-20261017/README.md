# LA 5×5 Parallel CPU result — current ARM64

This folder records one full public-call measurement for the native LA 5×5
binomial filter on the local MacBook Pro M3 Pro, macOS 15.7.7, CPython 3.12.13.
The target build was dirty revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`
with `pillow-rs/parallel` and `pillow-rs-py/parallel` enabled. The captured
target process loaded `_core.abi3.so` with SHA-256
`1b61edfd18cae4cdbdcc7a8a2d1ac3a30803a75835be937289cfec8cad05600a`.

## Measurement

Workload `pipeline-op.filter.material-la-noise-5x5-1024x768`, mode LA, uses
the frozen warm-cache contract: 5 warmups, 20 iterations × 5 samples,
concurrency one, and observed `call` plus `materialize` steps.

| Subject | Median latency | p95 latency | Samples |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 7,200.2505 µs | 7,687.541 µs | 100 |
| Parallel CPU | 1,232.3335 µs | 1,674.708 µs | 100 |

The same-host median ratio is 5.8428× in favor of Parallel CPU. Pillow was
timed separately with the same interpreter, host, workload, and timing policy;
its raw timings are retained in `pillow-source.json`. The target-only benchmark
report itself has no Pillow subject, so its ratio is not a same-report
comparison.

The target execution receipt completed one `Filter5x5` operation on actual
backend `cpu`, with no fallback. Together with the exact LA mode, binomial
kernel, scale and offset in the parity input, the parallel feature identity
selects the LA row-band helper. The helper invokes Rayon through
`par_row_blocks_collect!`; current operation telemetry does not expose a
Rayon-specific counter.

## Correctness and limits

The reused, report-bound parity evidence contains two exact passing cases:

- `PIL.Image.Image.filter.nuanced.la-mode-kernel-5x5-noise-1024x768`
- `PIL.Image.Image.filter.nuanced.la-mode-kernel-5x5-odd-width-7x5`

The focused Rust guard also passed 1/1 for integer/reference equality, copied
borders, small dimensions and 64-row band seams. The benchmark result schema
validator passed. `pillow-perfdb check-attestation` reports complete runtime
receipts and `performance_admission: not_proven`; the Parallel-only report has
no same-report Pillow row, and runtime receipts alone do not prove producer
and source/build provenance for database admission.

This is ARM64 Parallel CPU evidence for one LA workload. Serial CPU, SIMD, GPU,
native x86_64 and Pillow-SIMD remain open. An SSH availability probe to the
provided x86 runner returned `Connection refused`; it received no files and
nothing was removed remotely. No full parity, benchmark campaign or coverage
run was performed.

## Artifacts

- `benchmark.json`: selected target workload and result.
- `pillow-source.json`: same-host Pillow timing and raw samples.
- `parity.json`: the selected cases from the passing Parallel CPU parity run.
- `execution.attestation.json` and its `.children/`: benchmark runtime receipts.
- `producer.attestation.json` and its bundle/children: benchmark runner identity.
- `raw-samples.json`: ordered Parallel CPU timing and phase samples.
