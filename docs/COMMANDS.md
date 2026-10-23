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
| Explicit GPU stream parity/contracts | `make gpu-batch-parity gpu-batch-contracts` after `make build-parity` | Requires a usable GPU; checks live Pillow pixels and bounded streaming ownership |
| Explicit GPU batch timing | `make gpu-batch-benchmark` | Prints completed-work queued/eager GPU throughput versus sequential Pillow without JSON artifacts |
| Build for comparison | `make build-parity` | Builds the replacement without installing over the Pillow oracle |
| Build for application use | `make build` | Installs the replacement into the selected environment |
| Build npm package | `make build-wasm-release` | Compiles the shared Node/browser WASM artifact |
| One parity case | `make migration-parity-case CASE_ID=<id>` | Runs the selected input against source and target |
| Same cases on CPU and SIMD | Set `MIGRATION_TARGET_BACKEND=cpu` or `simd` and run `make migration-parity-test` with the same `MIGRATION_PARITY_CASE_IDS` | The runner selects the backend; parity inputs and requirements are backend-neutral |
| Aggregate backend parity | `MIGRATION_PARITY_OUTPUTS='build/migration-parity/cpu.json build/migration-parity/simd.json' make migration-parity-aggregate` | Joins each parity receipt to its recorded target profile; repeated result paths are supported |
| Review repetitive cases | `make migration-parity-reduction MIGRATION_REDUCTION_ARGS='--candidates <pairs.json> --output-dir <directory>'` | Measures removal batches and binary restoration against separate CPU/SIMD/GPU baselines; see [coverage](COVERAGE.md#reduce-repetitive-parity-inputs) |
| Complete runtime campaign | `make test` | Runs backend, Node/browser, and reverse Pillow coverage lanes |
| Format check / fix | `make fmt` / `make fmt-fix` | Checks / changes Rust formatting |
| Consolidated quality gate | `make quality` | Runs rustfmt, Clippy, Rust file/function size ratchets, API-boundary and architecture checks, repository-map and documentation checks, and source/binding/spec policies |
| Compatibility lint command | `make lint` | Alias for `make quality` |
| Rust checkstyle | `make checkstyle` | Reports files from 800 lines and functions over 60 lines; fails on new code over 1,000/100 lines or growth beyond a legacy baseline |
| Checkstyle baseline maintenance | `make checkstyle-update-baseline` | Re-records current over-limit files/functions after reviewed reductions; inspect the baseline diff |
| Rust size policy and legacy inventory | [Rust size policy](RUST_SIZE_POLICY.md) | Defines counting rules, exclusions, baseline behavior, owners, and reducer issues |
| Rust lint | `make clippy` | Runs workspace Clippy, reports functions over 60 lines, and fails on new functions over 100 lines or growth beyond a legacy baseline |
| Source map | `make repo-map-update` / `make repo-map-check` | Regenerates / validates the tracked source inventory |
| Operation source crosswalk | `make operation-source-crosswalk` / `make operation-source-crosswalk-test` | Generates the ignored manifest→Python→PyO3→backend crosswalk, maps declared parity cases and benchmark workloads to public operations, includes the Rust module inventory, and checks focused contracts; input declarations never count as execution |
| Implementation structure | `make implementation-structure-inventory` / `make implementation-structure-audit` / `make implementation-structure-audit-test` | Inventories every maintained path, audits additions and moves against the same guide, and checks path-classification and inventory-scope contracts |
| Full operation and pipeline benchmark | `MIGRATION_BENCHMARK_PROFILE=standard make migration-parity-benchmark` | Benchmarks every declared operation and pipeline workload against Pillow, CPU, SIMD, and GPU; see the protocol |
| Selected Pillow-SIMD workloads | `PILLOW_ORACLE_PYTHON=<pillow-python> PILLOW_SIMD_PYTHON=<pillow-simd-python> .venv/bin/python scripts/run_pillow_simd_benchmark.py --workload-id <id>` | Runs only the named full-size parity-backed x86 workload, with separate result and parity artifacts; repeat `--workload-id` for additional cases |
| Parallel CPU comparison | `MIGRATION_BENCHMARK_PROFILE=standard make migration-parity-benchmark-parallel-cpu` | Builds the opt-in Rayon feature and compares Parallel CPU with ordinary Pillow across the same full workload set |
| Benchmark smoke test | `make bench MIGRATION_BENCHMARK_PROFILE=quick` | Runs the maintained smoke cohort; see the protocol |
| Documentation setup | `make docs-setup` | Installs the hash-locked documentation tools into `.venv-docs` |
| Documentation checks | `make docs-test docs-lint` | Tests validation guards and checks public sources |
| Site build / preview | `make docs-build` / `make docs-serve` | Builds static HTML / serves it at localhost:8000 |
| Version synchronization | `make release-lock-update` / `make release-version-check` | Refreshes Cargo/npm workspace versions / checks declarations with Python 3.12 |
| Release preparation | `make release-check` | Builds and inspects registry artifacts; requires a clean checkout |
| Cache cleanup | `make clean` | Removes Python bytecode and the temporary report |
| Build cleanup | `make clean-all` | Also removes Cargo build outputs |

ImageBatch prototype-specific targets and scripts have been removed. Use the
separate [explicit GPU executor](IMAGE_BATCHING.md); its focused
[validation lanes](GPU_BATCH_VALIDATION.md) print evidence without JSON files.

Setup installs dependencies; ordinary help and documentation builds do not.
Documentation builds require `make docs-setup` once. They do not execute
benchmarks. `make docs-serve DOCS_PORT=8001` changes the preview port; stop with
Ctrl-C. CPU/GPU parity and coverage require the dependencies described in
[Contributing](../CONTRIBUTING.md).

Each specialized target accepts its documented selectors and output paths.
Do not run two campaigns against the same mutable Python installation or output
directory. Parallel Make is appropriate only for independent targets.

Parity case documents do not declare backend applicability. Run the same case
IDs with `MIGRATION_TARGET_BACKEND=cpu`, `simd`, or another supported backend;
the selected runner configuration is recorded in the result identity. The
aggregator accepts repeated `--parity` artifacts through
`MIGRATION_PARITY_OUTPUTS` and reports each result against the same case set.

The root fontdone targets own a pinned checkout under `build/`. To contribute
to fontdone itself, clone that repository separately and use its own Makefile.

`make migration-parity-font-native-coverage` builds the test-only Rust font
driver, checks its Python adapter, and records native font coverage observations
in `build/migration-parity/font-native-observations.json`. Returned values and
API exceptions are reported separately; this command does not assert parity.
Use `make migration-parity-font-native-test` for the focused harness checks.
