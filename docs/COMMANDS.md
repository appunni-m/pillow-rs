# Command reference

Run commands from the repository root with GNU Make 3.81 or newer. On Windows,
use a shell with GNU Make and the required toolchains, as CI does. Start with
`make help`; `make help-all` lists the specialized parity and coverage lanes.
Push and pull-request CI runs parity without collecting coverage. To collect
Python migration-parity coverage, manually dispatch CI with `run_coverage`
enabled.

| Task | Command | Effect |
| --- | --- | --- |
| Python development environment | `make setup-venv PYTHON=python3.12` | Creates this checkout's virtual environment and installs pinned tools |
| Build for comparison | `make build-parity` | Builds the replacement without installing over the Pillow oracle |
| Build for application use | `make build` | Installs the replacement into the selected environment |
| Build npm package | `make build-wasm-release` | Compiles the shared Node/browser WASM artifact |
| One parity case | `make migration-parity-case CASE_ID=<id>` | Runs the selected input against source and target |
| Explicit image-batch parity | `make build-parity && .venv/bin/python scripts/test_imagebatch_parity.py` | Compares `PIL.ImageBatch` with isolated Pillow; verifies one GPU shader dispatch per compatible group and a separate dispatch per fallback job |
| Explicit image-batch throughput | `.venv/bin/python scripts/benchmark_imagebatch.py --backend gpu --queue --operation color3dlut --mode RGBA --width 256 --height 256 --images 16 --samples 12 --warmups 3` | Run after `make build-parity`; measures full-call queued `ImageBatch.Color3DLUT`. `--operation` also accepts `median-filter`, `extract-band`, `multiply`, and `paste`; compare with `--backend pillow`, `cpu`, or `simd` without `--queue` |
| Explicit masked-Paste batch throughput | `.venv/bin/python scripts/benchmark_imagebatch.py --backend gpu --queue --operation paste --mode RGBA --width 256 --height 256 --images 16 --samples 12 --warmups 3` | Run after `make build-parity`; compare Pillow, CPU, SIMD, eager GPU, and queued GPU full-call timings. The operation is limited to compatible full-frame native-mode pastes with same-size L masks |
| Explicit masked-Paste Parallel CPU comparison | `make build-parity-parallel-cpu && .venv/bin/python scripts/benchmark_imagebatch.py --backend cpu --operation paste --mode RGBA --width 1024 --height 768 --images 4 --samples 12 --warmups 3` | Builds the opt-in Rayon profile; repeat with `--backend pillow` for the ordinary sequential Pillow baseline. Each image exceeds masked Paste's 512×512 row-parallel threshold |
| Review repetitive cases | `make migration-parity-reduction MIGRATION_REDUCTION_ARGS='--candidates <pairs.json> --output-dir <directory>'` | Measures removal batches and binary restoration against separate CPU/SIMD/GPU baselines; see [coverage](COVERAGE.md#reduce-repetitive-parity-inputs) |
| Complete runtime campaign | `make test` | Runs backend, Node/browser, and reverse Pillow coverage lanes |
| Format check / fix | `make fmt` / `make fmt-fix` | Checks / changes Rust formatting |
| Rust lint | `make clippy` | Runs workspace Clippy checks |
| Full lint | `make lint` | Runs Rust, binding, dependency, and input checks |
| Source map | `make repo-map-update` / `make repo-map-check` | Regenerates / validates the tracked source inventory |
| Full operation and pipeline benchmark | `MIGRATION_BENCHMARK_PROFILE=standard make migration-parity-benchmark` | Benchmarks every declared operation and pipeline workload against Pillow, CPU, SIMD, and GPU; see the protocol |
| Parallel CPU comparison | `MIGRATION_BENCHMARK_PROFILE=standard make migration-parity-benchmark-parallel-cpu` | Builds the opt-in Rayon feature and compares Parallel CPU with ordinary Pillow across the same full workload set |
| Benchmark smoke test | `make bench MIGRATION_BENCHMARK_PROFILE=quick` | Runs the maintained smoke cohort; see the protocol |
| Documentation setup | `make docs-setup` | Installs the hash-locked documentation tools into `.venv-docs` |
| Documentation checks | `make docs-test docs-lint` | Tests validation guards and checks public sources |
| Site build / preview | `make docs-build` / `make docs-serve` | Builds static HTML / serves it at localhost:8000 |
| Version synchronization | `make release-lock-update` / `make release-version-check` | Refreshes Cargo/npm workspace versions / checks declarations with Python 3.12 |
| Release preparation | `make release-check` | Builds and inspects registry artifacts; requires a clean checkout |
| Cache cleanup | `make clean` | Removes Python bytecode and the temporary report |
| Build cleanup | `make clean-all` | Also removes Cargo build outputs |

For the 64×64 × 64 Multiply comparison, run the listed command with each of
`--backend pillow`, `cpu`, and `simd`; use `--backend gpu` without `--queue`
to measure independent GPU calls. To reproduce the 256×256 × 16 cohort, change
the dimensions and image count to `--width 256 --height 256 --images 16` and
run the Pillow, SIMD, and queued-GPU profiles. Both profiles default to 12
samples and 3 warmups.

The Color3DLUT batch workload uses one shared non-identity 17³ RGBA table.
Run the same command with `--operation color3dlut --mode RGBA` at
`--width 64 --height 64 --images 64`, `--width 256 --height 256 --images 16`,
and `--width 1024 --height 768 --images 4`. Use `--backend pillow`, `cpu`, and
`simd` without `--queue`, plus `--backend gpu` both with and without `--queue`.
`scripts/test_imagebatch_parity.py` compares every output against the isolated
Pillow oracle and checks that compatible queued groups execute one actual
`color_3dlut.wgsl` dispatch with no mode conversion or fallback.

Setup installs dependencies; ordinary help and documentation builds do not.
Documentation builds require `make docs-setup` once. They do not execute
benchmarks. `make docs-serve DOCS_PORT=8001` changes the preview port; stop with
Ctrl-C. CPU/GPU parity and coverage require the dependencies described in
[Contributing](../CONTRIBUTING.md).

Each specialized target accepts its documented selectors and output paths.
Do not run two campaigns against the same mutable Python installation or output
directory. Parallel Make is appropriate only for independent targets.

The root fontdone targets own a pinned checkout under `build/`. To contribute
to fontdone itself, clone that repository separately and use its own Makefile.

`make migration-parity-font-native-coverage` builds the test-only Rust font
driver, checks its Python adapter, and records native font coverage observations
in `build/migration-parity/font-native-observations.json`. Returned values and
API exceptions are reported separately; this command does not assert parity.
Use `make migration-parity-font-native-test` for the focused harness checks.
