# RGB AutoContrast identity-LUT fast path (ARM64)

This trial adds a guarded fast path for native RGB images when all three AutoContrast lookup tables are identity maps. It returns a deep `DynamicImage` clone, preserving Pillow copy behavior and native RGB storage while skipping the per-byte map and reconstruction pass.

## Frozen scope and parity

- Workload: `pipeline-chain.autocontrast-materialized.rgb-identity-lut-1024x768`.
- Parity case: `PIL.ImageOps.autocontrast.nuanced.performance-rgb-identity-lut-1024x768` (RGB black, 1024×768; `ImageOps.autocontrast` followed by `tobytes`).
- Whole-workflow boundary; warm cache; one warmup; three iterations × two samples; concurrency 1; exact `parity_pass` gate.
- Existing guards: `uniform-rgb-no-range`, `nonzero-rgb-range`, the three per-channel fallback cases, `binary64-rgb-23-113-full`, and the RGB mask cases `PIL.ImageOps.autocontrast.parameter.mask` and `PIL.ImageOps.autocontrast.nuanced.valid-l-mask`.
- Current-source serial CPU parity: 7/7 passed before the change; 7/7 passed after it, plus 2/2 exact RGB mask guards.
- Parallel CPU parity preflight: 1/1 passed for the full-size case, plus 2/2 exact RGB mask guards. The serial profile also passed those 2 mask guards.

## Serial CPU result

| Run | Pillow median | Serial CPU median | CPU samples | Receipt |
| --- | ---: | ---: | ---: | --- |
| baseline-A1 (`migration-benchmark-0a7659380e124eada4fd89cfdb5f11da`) | 1.992667 ms | 0.997792 ms | 6 | actual `cpu`, terminal complete `True`, fallbacks `{}` |
| baseline-A2 (`migration-benchmark-35ad4bb732a04ea6925f6502fd7b979b`) | 2.156770 ms | 0.931417 ms | 6 | actual `cpu`, terminal complete `True`, fallbacks `{}` |
| candidate-B1 (`migration-benchmark-2ef3b944283d4324a761bde75e018572`) | 3.183417 ms | 0.456937 ms | 6 | actual `cpu`, terminal complete `True`, fallbacks `{}` |
| candidate-B2 (`migration-benchmark-cd9b8eda075a47f5bd864da2f6dd88cd`) | 2.956771 ms | 0.435917 ms | 6 | actual `cpu`, terminal complete `True`, fallbacks `{}` |

The two baseline CPU medians were 0.931417–0.997792 ms. The two candidate medians were 0.435917–0.456937 ms, about 2.16× lower at the same end-to-end boundary. The per-run Pillow medians drifted materially between cohorts, so the higher candidate/Pillow ratios are not treated as a stable speedup claim.

## Parallel CPU result

- Run `migration-benchmark-c09b37f2f0d7457990569a3649540c2c` used release `parallel` features (`pillow-rs-py/parallel`, `pillow-rs/parallel`) and recorded target profile `python-parallel-cpu`.
- Median 0.429916 ms across 6 samples; actual backend `cpu`, terminal complete `True`, fallbacks `{}`.
- The default serial CPU runs measured Pillow at 1.992667 and 2.156770 ms for the same host, workload, and input. Keep this profile separate from serial CPU; the Parallel CPU benchmark intentionally records only its target cohort.

## Limits and retained evidence

All measurements ran on macOS 15.7.7 ARM64 with the local dirty source tree. A native x86 runner check to `appunni@192.168.0.221` returned `Connection refused`; no files were copied to or removed from that machine. These results do not replace the historical x86 row. SIMD and GPU were not executed because the code change is confined to the CPU operator.

Builds: `make build-parity` and `make build-parity-parallel-cpu` succeeded. No coverage or full parity/benchmark campaign ran. The reports, raw samples, producer/runtime attestations, and child receipts are in `reports.tar.gz`. It includes the passing serial and Parallel CPU mask-guard reports; the first parallel guard invocation, which omitted `MIGRATION_TARGET_PROFILE=parallel-cpu`, is excluded.

- Archive SHA-256: `7de1b240d6154fd7f20df0838b52f703c504033ce926937ed4eaa99b166db1fb`
- Serial candidate B1 run: `migration-benchmark-2ef3b944283d4324a761bde75e018572`
- Serial candidate B2 run: `migration-benchmark-cd9b8eda075a47f5bd864da2f6dd88cd`
- Operation remains open for x86, other RGB inputs, other modes, and SIMD/GPU.
