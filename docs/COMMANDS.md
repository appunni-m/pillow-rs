# Command reference

Run commands from the repository root with GNU Make 3.81 or newer. On Windows,
use a shell with GNU Make and the required toolchains, as CI does. Start with
`make help`; `make help-all` lists the specialized parity and coverage lanes.

| Task | Command | Effect |
| --- | --- | --- |
| Python development environment | `make setup-venv PYTHON=python3.12` | Creates this checkout's virtual environment and installs pinned tools |
| Build for comparison | `make build-parity` | Builds the replacement without installing over the Pillow oracle |
| Build for application use | `make build` | Installs the replacement into the selected environment |
| Build npm package | `make build-wasm-release` | Compiles the shared Node/browser WASM artifact |
| One parity case | `make migration-parity-case CASE_ID=<id>` | Runs the selected input against source and target |
| Review repetitive cases | `make migration-parity-reduction MIGRATION_REDUCTION_ARGS='--candidates <pairs.json> --output-dir <directory>'` | Measures removal batches and binary restoration against separate CPU/SIMD/GPU baselines; see [coverage](COVERAGE.md#reduce-repetitive-parity-inputs) |
| Complete runtime campaign | `make test` | Runs backend, Node/browser, and reverse Pillow coverage lanes |
| Format check / fix | `make fmt` / `make fmt-fix` | Checks / changes Rust formatting |
| Rust lint | `make clippy` | Runs workspace Clippy checks |
| Full lint | `make lint` | Runs Rust, binding, dependency, and input checks |
| Source map | `make repo-map-update` / `make repo-map-check` | Regenerates / validates the tracked source inventory |
| Benchmark | `make bench MIGRATION_BENCHMARK_PROFILE=quick` | Runs the maintained smoke cohort; see the protocol |
| Documentation setup | `make docs-setup` | Installs the hash-locked documentation tools into `.venv-docs` |
| Documentation checks | `make docs-test docs-lint` | Tests validation guards and checks public sources |
| Site build / preview | `make docs-build` / `make docs-serve` | Builds static HTML / serves it at localhost:8000 |
| Version synchronization | `make release-lock-update` / `make release-version-check` | Refreshes Cargo/npm workspace versions / checks declarations with Python 3.12 |
| Release preparation | `make release-check` | Builds and inspects registry artifacts; requires a clean checkout |
| Cache cleanup | `make clean` | Removes Python bytecode and the temporary report |
| Build cleanup | `make clean-all` | Also removes Cargo build outputs |

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
