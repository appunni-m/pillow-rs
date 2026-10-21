# GetData HSV band 2: current-source native x86 rebaseline

This is a one-workload diagnostic for the dirty current source tree. It does
not promote the clean published matrix or claim a source optimization.

## Frozen scope and result

- Workload: `pipeline-chain.terminal-read.getdata.hsv-band2-1024x768`
- Exact parity case: `PIL.Image.Image.getdata.performance.material-hsv-band2-1024x768`
- Boundary: observed `band-data`, including returned `ImagingCore` sequence
  materialization; warm release build, 5 warmups, 20 iterations × 5 samples,
  concurrency 1.
- Parity: exact pass for both selected profiles (2/2); benchmark and parity
  result schemas validate.
- Host: AMD Ryzen 5 2600X, x86_64 Linux, CPython 3.12.3; Pillow 12.2.0.
- Source: dirty `418d5f0cde51ca64a97cbd007ea731da72d68abf`; loaded
  `lib_core.so` SHA-256 `dee09f63e602025d70551cb8e960bb8cc0c9cc0a636e1e00b323d4de223c5d19`.

| Profile | Median latency | P95 | Speedup vs Pillow | Execution proof |
| --- | ---: | ---: | ---: | --- |
| Pillow | 10.940142 ms | 11.075442 ms | 1.000× | Pillow 12.2.0 |
| Serial CPU | 5.636147 ms | 5.673252 ms | 1.941× | actual CPU; one GetData; terminal complete; no fallback |
| SIMD | 5.409436 ms | 5.453771 ms | 2.022× | actual `ssse3-gather`; 49,152 vector blocks; zero scalar tail; no fallback |

Serial CPU clears the no-slower-than-Pillow requirement, but misses the 2×
sequencing checkpoint narrowly. SIMD is 1.042× faster than serial CPU and just
clears the 2× Pillow checkpoint; it remains below the repository's 5× SIMD
completion target. SIMD telemetry reports two host buffers totaling 3,145,728
bytes; CPU reports one 786,432-byte output buffer. Neither path reports a mode
conversion or fallback.

The timer includes the public GetData call and iteration of its bytes-backed
`ImagingCore` result. The wrapper's `ImagingCore.__iter__` returns the bytes
iterator, so the measured work includes the final output and its consumer. The
representative untimed backend receipt reports 276,231 ns for CPU and 71,030 ns
for SIMD; these are single receipts, not latency distributions. They do not by
themselves identify enough removable work for a kernel change. The earlier
mode-cache, scalar-unroll, and interleaved-scan experiments remain rejected;
profile the public call before proposing another GetData variant.

## Identity and retained artifacts

Benchmark run `migration-benchmark-a804529761dc4924a8ede604bf805a7e` passed
the focused result validator. Parity gate:
`migration-parity-benchmark-gate-25286402da994d24a8203c6954f58ed3`.
Benchmark report SHA-256:
`acea0829a586184a2f08b979a428556a612393a733c9e02fcf717cf882f64518`.
Parity report SHA-256:
`e339da31b684e593d29e1805cc76360a0ea380567706220fb3322149664c741a`.
Raw samples SHA-256:
`76a1d824ad7e6c2f8725c166f0380b9ad9d43d149e65a5d84a8a4655740a1a48`.

The selected workload and case objects match the local tree by SHA-256
(`53401efb…67d3d3` and `14f28d77…73ffaf`). The local and x86 manifests differ
because the x86 checkout has unrelated dirty fixture-index changes; this run is
therefore diagnostic and is not a clean matrix row. The measured source,
binding, Python wrapper, SIMD adapter, benchmark runner, and parity runner files
listed below match the local tree:

- `pillow-rs/src/image.rs`: `2cd89619…ccb03d5`
- `pillow-rs-py/src/lib.rs`: `cff227d1…511ef2c`
- `pillow-rs-py/python/pillow_rs/image.py`: `da25efb0…22e1e8`
- `pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs`:
  `e5f5f930…d6bad3ecdde`
- `scripts/run_migration_benchmark.py`: `67354418…eab536cb`
- `scripts/run_migration_parity.py`: `2203d553…c0ae7d63`

Only this selected workload's report, parity evidence, samples, and attestation
sidecars were copied back from the existing x86 checkout. No checkout or
`target/` copy was created there, and no remote files were deleted.

## Existing host build setup

The normal `make migration-parity-benchmark` build prerequisite could not use
the host's `.venv`: it has no `maturin`, `pip`, or `ensurepip`; the checkout also
configures absent `sccache`. The existing `~/.cargo/bin/maturin` and pinned Rust
toolchain were present. An in-place `cargo build --release --locked` with
`CARGO_BUILD_RUSTC_WRAPPER=` updated the already-existing `lib_core.so` target
behind the Python package symlink. This enabled the selected benchmark without
installing packages, creating another checkout, or copying a wheel. Future x86
runs can reuse that existing route until the runner environment is repaired.
