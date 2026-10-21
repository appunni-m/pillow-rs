# Current-source YCbCr Grayscale x86 refresh — 2026-10-20

This selected run refreshes the 1024×768 materialized YCbCr
`ImageOps.grayscale` workload on the native x86_64 runner. It confirms a
current same-run CPU and SIMD miss against Pillow. It is diagnostic evidence,
not a strict baseline or an optimization claim.

## Frozen scope and identity

- Workload: `pipeline-op.grayscale.material-ycbcr-noise-1024x768`.
- Parity case: `PIL.ImageOps.grayscale.nuanced.performance-material-ycbcr-noise-1024x768`,
  selected for both `python-cpu` and `python-simd`.
- Boundary: public call plus result observation (`call`, `observe-result`),
  warm release execution, 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Host: native x86_64 Linux 7.0.0-38, AMD Ryzen 5 2600X; CPython 3.12.3;
  Pillow 12.2.0. Runner ID: `native-x86-192.168.0.221`.
- Benchmark: `migration-benchmark-cb2821daf90c4b318390292e310ee3ac`.
- Parity gate: `migration-parity-benchmark-gate-32a62b788de14c7a94957e6110d10f14`;
  2/2 selected comparisons passed with no diffs or infrastructure errors.
- Target revision: dirty worktree at
  `418d5f0cde51ca64a97cbd007ea731da72d68abf`.
- Loaded `_core.abi3.so` SHA-256:
  `7b7433a32cd1a4c49dd4019d328b673f5cf960c16a6cef2133a6123ce118a066`.
- The selected workload object hash matched the local fixture
  (`5a3b2bb1b3b812a364f249959e9a9bed55d9324b01d7a6d80bf990a9aac6a821`),
  and the parity case asset hash was
  `8d09ced0ef27917e84162a9bc924b41913252d85adfb87a2464a307c2a8cd110`.
  Relevant source-file hashes matched between the runner and this worktree:

| Source file | SHA-256 |
| --- | --- |
| `pillow-rs/src/compute/pool_cpu/ops/color.rs` | `1e0536eeccdde70eac33495bbdc1553749021f4f63a81a5e1d9acf6daa7dc732` |
| `pillow-rs/src/compute/pool_cpu/ops/grayscale.rs` | `9ef2e0694fa43c8cf33a824e06b9c5205378ff8aee2385c439f6a5a6a603f48d` |
| `pillow-rs/src/compute/pool_simd/ops/adapters/grayscale.rs` | `93674c82b7bee71fd647eb5e75ef0f085ddf468f6d2c1b072abb0aea5a8b6660` |
| `pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/mod.rs` | `deb54f7aa6ffa269424a2fd116c6ff0235d403934d442ca651f08ab69e419df3` |
| `pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs` | `cfc040f63fb19db6780189618257c22bdbe7c442c6a6c1d144019c961d2c751c` |
| `pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr_ssse3.rs` | `a4ebfb9a978e260ac328ea7c5d9afaf1094bffbff1fa2184e8a15379ceb65712` |
| `pillow-rs/src/image.rs` | `2cd8961958c97a2832ead1c823d1855b51abeee7a79be8e2fdade40d6ccb03d5` |

The full manifest and benchmark/parity input-file hashes differ from the local
worktree; the exact report-referenced files are retained here as
`manifest.yaml`, `benchmark-input.json.gz`, and `parity-input.json.gz`. Treat
this as a selected-object matched diagnostic, not a strict-source baseline
eligible for promotion. The loaded extension digest is recorded, but there is
no complete clean-tree build witness for this run.

## Results

Each ratio uses the Pillow median from the same report. The timing includes the
public call and materialization boundary for all profiles.

| Profile | Pillow median / p95 | Target median / p95 | Pillow / target | Backend receipt |
| --- | ---: | ---: | ---: | --- |
| CPU | 270.190 / 347.460 µs | 834.296 / 935.011 µs | 0.324× | actual CPU; terminal complete; no fallback; 1 Grayscale operation |
| SIMD | 270.190 / 347.460 µs | 958.181 / 1062.791 µs | 0.282× | actual SIMD; `ssse3-deinterleave`; 49,152 vector blocks; zero tail; no fallback |

Both targets allocated the 786,432-byte output and recorded one mode
conversion. Across the 20 iteration medians, CPU ranged from 814.661 to
855.101 µs, SIMD from 940.261 to 990.881 µs, and Pillow from 259.250 to
307.611 µs; the miss was not a single-sample spike.

The separate phase instrumentation places median terminal materialization at
812.891 µs on CPU, 937.781 µs on SIMD, and 159.845 µs for Pillow. Use this only
to locate the cost; the full-call medians above are the performance result.

## Interpretation and limits

The prior x86 reports on 2026-10-18 recorded Pillow at 1.160800 ms and CPU at
0.851483 ms (1.363×), and Pillow at 1.188005 ms and SIMD at 0.855338 ms
(1.389×). The target medians are similar, but the Pillow denominator differs
by more than 4×. The runs have different report/manifest identities and the
older report did not record the CPU model. Do not compare their ratios across
reports or infer a kernel regression; the new within-report comparisons show
that both current targets are slower than Pillow in this cohort.

The report's `identity.command.argv` says `make migration-parity-benchmark`,
although this was invoked directly with a workload selector. This is a known
producer-provenance defect. Scope is evidenced by the single selected workload
and the two selected parity comparisons in the report. No Pillow-SIMD oracle
was measured in this run.

The source path remains open for CPU and SIMD optimization. Earlier scalar,
unrolled, AVX2, and direct-store trials are recorded under Kata
`pillow-rs#xjn8`; do not repeat them without a new bottleneck hypothesis. No
candidate was added, no strict baseline was promoted, and no full benchmark,
parity campaign, or coverage run was performed for this refresh.

## Retained artifacts

This directory contains the benchmark report, raw samples, execution and
producer attestations with child receipts, the compressed exact-parity report
with its producer and attestation, and the exact input and manifest files
referenced by the report. `SHA256SUMS` covers all retained artifacts and this
README. The corresponding Pillow, CPU, and SIMD rows are in
`docs/evidence/performance-optimization-local-runs.csv`.
