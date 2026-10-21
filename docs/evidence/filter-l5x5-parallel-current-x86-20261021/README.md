# Filter5x5 native-L Parallel CPU rebaseline — x86_64 — 2026-10-21

This rechecks the native-L 5×5 binomial filter on clean `main` at
`fce9a28b9062c129cf8159a52be231799801359f`. It supersedes the 2026-10-18
rebaseline as the latest x86 source measurement; the earlier run remains
available at
[`filter-l5x5-parallel-current-x86-20261018`](../filter-l5x5-parallel-current-x86-20261018/README.md).

## Frozen scope and result

- Workload: `pipeline-op.filter.material-l-noise-5x5-1024x768` (L, 1024×768).
- Primary parity case: `PIL.Image.Image.filter.nuanced.l-mode-kernel-5x5-noise-1024x768`.
- Admission guard: `PIL.Image.Image.filter.nuanced.l-mode-flat-kernel-5x5-noise-1024x768`.
- Kernel: exact 5×5 binomial coefficients, scale 256, offset 0.
- Timing: warm public `call;materialize`, 5 warmups, 20 iterations × 5 samples,
  concurrency one.
- Host: AMD Ryzen 5 2600X, Linux `7.0.0-38-generic` with glibc 2.39,
  CPython 3.12.3, Pillow 12.2.0.

| Subject | Median | P95 | Samples |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 9.919031 ms | 12.140210 ms | 100 |
| Parallel CPU | 0.302127 ms | 0.408422 ms | 100 |

The separate same-host Pillow source adapter and target-only Parallel CPU
report use the same workload, interpreter, boundary and sample policy. Their
median ratio is 32.8307× in favor of Parallel CPU. Treat this as a local paired
comparison, not a same-report PerfDB ranking. The run has no configured
performance budgets, so it claims no budget pass.

## Identity and correctness

- Benchmark run: `migration-benchmark-0eb0a9db4ed04949956ac3b394ea1666`.
- Parity run: `migration-parity-acc8be23dee4420899defd2f3cb16cc2` (2 selected,
  2 passed; each selected case passed 1/1).
- Both `pillow-rs/parallel` and `pillow-rs-py/parallel` were enabled. The
  loaded `_core.abi3.so` SHA-256 is
  `a8737411d3c70b4a3f6c3d0b20c27dc0c32765aed9aa9ff50ac1dff4448eb668`.
- The target receipt reports actual backend `cpu`, one completed `Filter5x5`,
  terminal completion and no fallback. The receipt has no Rayon-specific
  counter; the exact dispatch and source path establish the parallel strategy.
- `filter.rs` SHA-256:
  `96aab7ec3af882be049c963c199afe0553f28c9281f6d773b4838669a9c3ff51`.
  `par.rs` SHA-256:
  `12c89ca10f6373eb3d0aea69b883bd5dc5481ebf4512dca6a717f78208fafcdf`.
- Manifest SHA-256: `60aa7baabfb11d7d43cd8ade8978a39d9642a3a31b22a6a1307478ef09298972`.
  Selected benchmark input SHA-256:
  `9f0a8fdbef7c4eb921ac9897a7ddd1810c213df4fe962fb8e7d9870f541202a8`.
  Selected parity input SHA-256:
  `5fab0fc0464ee54d183330c36a4d1207b6e52e0f0638582ec2bfdd9452a76a93`.
  Generated pixel asset SHA-256:
  `e043a2e84c2350fc3326390eaa6ee137ff397e88c5e54240d3eb96e35dfdad47`.

## Limits and artifacts

This covers only the selected Parallel CPU L workload and its two parity cases.
Serial CPU, SIMD, GPU, other modes and sizes remain unmeasured. The 3,309.876
ops/s median at concurrency one is reciprocal latency, not sustained
throughput. The x86 run does not establish an A–B–B–A change or a
Rayon-specific runtime count.

Benchmark, raw sample, separate Pillow source, parity, execution-attestation
and producer files are retained in this directory. The benchmark and parity
result schema validators passed. `SHA256SUMS.txt` records artifact hashes.
