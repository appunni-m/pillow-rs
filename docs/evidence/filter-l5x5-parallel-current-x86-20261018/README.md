# Filter5x5 native-L Parallel CPU rebaseline — x86_64 — 2026-10-18

This records the 2026-10-18 recheck of the historical x86 Parallel CPU miss on
clean source at `1c9203b11e789059b139bbd4772bc3184cf37511`. It is now a
historical rebaseline; the latest clean-main x86 result is the
[2026-10-21 rebaseline](../filter-l5x5-parallel-current-x86-20261021/README.md).
The old row reported 22.734 ms Parallel CPU vs
8.864 ms Pillow (0.3899×), but did not attest the loaded extension or the
Parallel feature. The October 18 measurement is a focused rebaseline, not an
A–B–B–A comparison with that historical build.

## Frozen scope and identity

- Workload: `pipeline-op.filter.material-l-noise-5x5-1024x768`.
- Primary parity case: `PIL.Image.Image.filter.nuanced.l-mode-kernel-5x5-noise-1024x768`.
- Flat-kernel admission guard: `PIL.Image.Image.filter.nuanced.l-mode-flat-kernel-5x5-noise-1024x768`.
- Mode and size: L, 1024×768; exact 5×5 binomial kernel, scale 256, offset 0.
- Timed boundary: public `call` plus `materialize`, warm cache, 5 warmups,
  20 iterations × 5 samples, concurrency one.
- Host: native x86_64 runner at `192.168.0.221`, Linux
  `7.0.0-38-generic`, glibc 2.39; CPython 3.12.11, Pillow 12.2.0.
- Source: clean revision `1c9203b11e789059b139bbd4772bc3184cf37511`.
- Loaded `_core.abi3.so` SHA-256:
  `e23eb26acd1a5e59c3c28b43a965b731ceb0fef155717ee846865dc38e9fc028`.
- `filter.rs` SHA-256:
  `aeacce762f0bcaa30c853e7fb70d1d649b30ee28f9bb2a8a30b3fd43db846175`.
- Input pixel asset SHA-256:
  `e043a2e84c2350fc3326390eaa6ee137ff397e88c5e54240d3eb96e35dfdad47`.
- Benchmark run: `migration-benchmark-e810c8ed38084d15ade78ad0510efc26`.
- Primary parity gate: `migration-parity-benchmark-gate-e66b36dbab5a42f2aee43308595a3ec8`.
- Flat-kernel guard parity: `migration-parity-ba4bf6d512b744a0ab4c850c5ae6c83b`.

The target was built with both `pillow-rs-py/parallel` and
`pillow-rs/parallel`; the report identifies `python-parallel-cpu` and a clean
source tree. The two selected parity cases passed 1/1 each. Benchmark schema
validation passed.

## Results

| Subject | Median | P95 | Samples |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 9.422411 ms | 9.699967 ms | 100 |
| Parallel CPU | 0.304787 ms | 0.396981 ms | 100 |

The same-host, same-interpreter, same-workload median ratio is 30.9147× in
favor of Parallel CPU. Pillow was measured by a separate source adapter with
the same boundary and sampling policy; the target-only Parallel CPU report
does not contain a Pillow subject. Treat this paired ratio as local evidence,
not as a same-report database ranking row. The old 22.734 ms target median is
about 74.6× higher than this current result, but its missing extension and
feature identities prevent a causal A/B claim.

The target receipt records one completed `Filter5x5`, actual backend `cpu`,
terminal completion, and no fallback. The code route was verified against the
built feature and exact case: `execute_filter5x5` selects
`filter_5x5_binomial_luma_parallel_rows` for this native-L kernel, and that
helper uses `par_row_blocks_collect!`, whose indexed iterator calls Rayon
`into_par_iter()`. The receipt does not expose a Rayon-specific runtime
counter, so this source-path check is the evidence for the parallel strategy;
the receipt itself proves CPU execution. The 3,280.985 ops/s median is the
single-concurrency reciprocal-throughput metric, not a sustained-throughput
measurement. The receipt reports two host buffers totaling 1,572,864 bytes;
its host-allocation counter is zero, which does not account for every Rust
scratch allocation and is not proof that the call allocates nothing.

This result covers Parallel CPU only. Serial CPU, SIMD, GPU latency and GPU
sustained throughput remain unmeasured here; this row does not close
Filter5x5's mode, size, or backend matrix.

## Artifacts

- `l5x5-parallel-current-x86-py31211-20261018.json`: target benchmark report.
- `l5x5-parallel-current-x86-py31211-raw-20261018.json`: ordered target
  samples, including warmups.
- `l5x5-pillow-source-x86-py31211-20261018.json`: matched Pillow source
  samples and summary.
- `l5x5-parallel-current-x86-py31211-parity-20261018.json.gz`: primary exact
  parity report.
- `l5x5-parallel-flatguard-parity-x86-py31211-20261018.json.gz`: flat-kernel
  guard report.
- Benchmark, parity, producer attestations and their child receipts.
- `l5x5-current-x86-evidence-20261018.tar.gz`: original transfer bundle.
- `SHA256SUMS.txt`: hashes of the retained files.
