# L 5×5 Parallel CPU rebaseline — current ARM64

This folder records the current-source native-L 5×5 binomial filter path on
the local MacBook Pro M3 Pro, macOS 15.7.7, CPython 3.12.13. The target was
dirty revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, built with
`pillow-rs/parallel` and `pillow-rs-py/parallel`. The captured target process
loaded `_core.abi3.so` with SHA-256
`1b61edfd18cae4cdbdcc7a8a2d1ac3a30803a75835be937289cfec8cad05600a`.

## Measurement

Workload `pipeline-op.filter.material-l-noise-5x5-1024x768`, mode L, uses the
unchanged warm-cache policy: 5 warmups, 20 iterations × 5 samples, concurrency
one, and the public `call` plus `materialize` boundary.

| Subject | Median latency | p95 latency | Samples |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 3,990.979 µs | 4,479.875 µs | 100 |
| Parallel CPU | 244.146 µs | 341.625 µs | 100 |

The same-host median ratio is 16.3467× in favor of Parallel CPU. Pillow was
timed separately with the same interpreter, host, workload and policy; raw
samples are in `pillow-source.json`. The target-only benchmark report has no
Pillow subject, so the ratio is not a same-report comparison.

The target execution receipt completed one `Filter5x5` operation on actual
backend `cpu`, with no fallback. The exact L mode, binomial kernel, scale and
offset in the passing parity case select the feature-gated L row-band helper.
It calls `par_row_blocks_collect!`, which uses Rayon `into_par_iter()`;
operation telemetry records CPU execution but has no Rayon-specific counter.

## Correctness and limits

Fresh focused parity passed for
`PIL.Image.Image.filter.nuanced.l-mode-kernel-5x5-noise-1024x768` (1 selected,
1 executed, 1 passed). The benchmark result schema validator passed.

`pillow-perfdb check-attestation` reports complete runtime receipts and
`performance_admission: not_proven`: this Parallel-only report has no
same-report Pillow subject, and runtime receipts alone do not bind parity and
source/build provenance for database admission. Keep the separate source
comparison explicit and do not describe its ratio as a DB-verified rank row.

This is current-source ARM64 evidence. It does not revalidate the historical
Ubuntu x86_64 0.3899× row: the native x86 runner SSH probe still returns
`Connection refused`. The separate scout issue `pillow-rs#kpfj` keeps the
native x86 follow-up open. Pillow-SIMD, serial CPU, SIMD and GPU are also open.
No full parity, benchmark campaign or coverage run was performed.

## Artifacts

- `benchmark.json`: selected target workload and result.
- `pillow-source.json`: same-host Pillow timing and raw samples.
- `parity.json`: fresh selected Parallel CPU parity case.
- `execution.attestation.json` and its `.children/`: benchmark runtime receipts.
- `producer.attestation.json` and its bundle/children: benchmark runner identity.
- `raw-samples.json`: ordered Parallel CPU timing and phase samples.
