# pillow-rs

pillow-rs is a Rust image-processing library with Python and WebAssembly
bindings. It follows Pillow's public behavior through a manifest-driven,
input-only parity suite. The project is still version `0.1.0`; compatibility is
measured per operation and backend rather than advertised as a complete Pillow
replacement.

## Current status

The active contract is defined by
[`pillow-rs/tests/fixtures/manifest.yaml`](pillow-rs/tests/fixtures/manifest.yaml)
(`migration-parity/manifest@2`). The latest integrated campaign selected 11,345
public input workflows and recorded exact terminal output comparisons across
the available CPU, SIMD, GPU, Node WASM, and browser WASM lanes. The GPU lane
recorded 7,090 native dispatches and 137 explicitly classified host controls;
those controls remain visible as backend coverage gaps. The current acceptance
blocker is the benchmark budget comparison described in
[`docs/benchmark-backend-pending-2026-09-03.md`](docs/benchmark-backend-pending-2026-09-03.md).

These figures are measured evidence from named runs, not a promise that every
Pillow API, platform, or GPU adapter is supported.

## Packages

| Target | Source | Import/use |
| --- | --- | --- |
| Rust core | [`pillow-rs/`](pillow-rs/) | `pillow_rs` |
| Python extension | [`pillow-rs-py/`](pillow-rs-py/) | `from RSPIL import Image` |
| WebAssembly package | [`pillow-rs-js/README.md`](pillow-rs-js/README.md) | `import { Image } from "pillow-rs"` |

The Python wheel uses the `abi3-py38` boundary and declares `requires-python >=3.8`.
The parity oracle is Pillow 12.2.0, which requires Python 3.10 or newer, so CI
parity runs use Python 3.10 and 3.12. The WASM CI lane uses Node 22.14.0 with
npm 11.5.1.

## Quick start from a checkout

Use an isolated Python 3.10+ environment for the parity tools:

```sh
make setup-venv PYTHON=python3.12
make build
python -c "from RSPIL import Image; print(Image.new('RGB', (10, 10)))"
```

Build the WASM package and run its package check with:

```sh
make build-wasm-release
cd pillow-rs-js && npm run test:package
```

Published registry artifacts are not implied by this source checkout. Use the
release checklist before installing from or publishing to PyPI, npm, or
crates.io.

## API and parity

The manifest currently declares 24 public surfaces, 209 operations, 1,801
requirements, 11,345 parity inputs, 24 coverage plans, and 744 standard
benchmark workloads. These are declared/indexed counts; live results are kept
separately in `build/migration-parity/`.

Every active parity case contains only public stimulus. Pillow produces the
oracle result at run time, and the Rust target is compared with the same input.
Coverage selectors and benchmark workloads reference those cases without
embedding expected output bytes or hashes.

Regenerate and validate the input contract with:

```sh
make migration-parity-inputs
make migration-parity-inputs-check
make migration-parity-fixtures-check
```

## Test and verification commands

The root `Makefile` is the maintained command interface:

```sh
make fmt
make clippy
make migration-parity-test
make migration-parity-test-all-backends
make migration-parity-coverage
make test
```

`make test` runs the shared target lanes, JS/WASM lanes, reverse Pillow source
coverage, and the ordered missing-feature report. GPU and browser capability
limitations are recorded in the result artifact. They are not converted into
passing output.

For one case, use `make migration-parity-case MIGRATION_PARITY_CASE=<case-id>`.
Use the same case ID with the coverage and all-backend variables when
debugging a first divergence.

## Benchmarking

The benchmark inputs and repeat policy are documented in
[`docs/BENCHMARKING.md`](docs/BENCHMARKING.md). Run the quick smoke cohort with:

```sh
MIGRATION_BENCHMARK_PROFILE=quick make migration-parity-benchmark
make migration-parity-pipeline-report
```

Run the complete standard workload set with
`MIGRATION_BENCHMARK_PROFILE=standard make migration-parity-benchmark`. A timing
number is publishable only when the workload passed its correctness gate and
the manifest, input, runtime, and backend identities match the comparison.
Use `MIGRATION_BENCHMARK_PROFILE=release make migration-parity-benchmark` for
the fixed 11-workload release acceptance cohort.
`BENCHMARKS.md` is a landing page; the JSON result is the evidence record.

## Architecture

```text
pillow-rs/       pure Rust image model, operations, pipeline, and backends
pillow-rs-py/    thin PyO3 boundary and Python compatibility modules
pillow-rs-js/    thin wasm-bindgen boundary and Node/browser adapters
scripts/         manifest generators, parity runners, validators, and reports
```

Runtime image logic lives in the Rust core. Bindings convert host values and
delegate to it. The compute registry can route a pipeline through CPU, SIMD,
or GPU; unsupported or unproven GPU work keeps an explicit host-control record
and follows the configured fallback policy.

## Documentation map

| Page | Purpose |
| --- | --- |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Setup, development, tests, coverage, and release checks |
| [`docs/DOCUMENTATION_CHECKLIST.md`](docs/DOCUMENTATION_CHECKLIST.md) | Active documentation rules and claim labels |
| [`docs/REPOSITORY_FILE_AUDIT.md`](docs/REPOSITORY_FILE_AUDIT.md) | Tracked-file classification and archive/deletion policy |
| [`docs/CI_CD_RELEASE_PLAN.md`](docs/CI_CD_RELEASE_PLAN.md) | Fixed inputs, CI stages, release sequence, and Pages plan |
| [`docs/LOCAL_FIRST_RELEASE.md`](docs/LOCAL_FIRST_RELEASE.md) | Exact local bootstrap tags, package order, evidence, and public prerequisites |
| [`docs/REGISTRY_RELEASE_MATRIX.md`](docs/REGISTRY_RELEASE_MATRIX.md) | Local registry setup, dependency order, and guarded publication runbook |
| [`RELEASING.md`](RELEASING.md) | Root artifact order, bootstrap rehearsal, and tag-driven releases |
| [`SECURITY.md`](SECURITY.md) | Private vulnerability reporting and disclosure scope |
| [`SUPPORT.md`](SUPPORT.md) | Reproducible bug reports and parity support |
| [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) | Contribution and project-space conduct |
| [`docs/REPO_MAP.md`](docs/REPO_MAP.md) | Maintained ownership map and generated source tree |
| [`docs/COVERAGE.md`](docs/COVERAGE.md) | Current coverage evidence landing page |
| [`BENCHMARKS.md`](BENCHMARKS.md) | Current benchmark evidence landing page |
| [`docs/generated/`](docs/generated/) | Generated contract and evidence views |
| [`docs/benchmark-backend-pending-2026-09-03.md`](docs/benchmark-backend-pending-2026-09-03.md) | Integrated GPU parity and benchmark acceptance record |

Older dated reports, `CODEBASE_AUDIT.md`, `SYSTEMIC_FIXES.md`, and
`docs/superpowers/` are historical references. The retired fixture and oracle
trees are under [`deprecated/`](deprecated/) with an explicit migration map.

## Contributing

Before opening a change, read [`CONTRIBUTING.md`](CONTRIBUTING.md) and the
documentation checklist. New operations require a manifest entry, input-only
parity and coverage cases, and measured coverage. Fixes must preserve failing
cases and thresholds until the implementation is correct.

## License and attribution

pillow-rs is licensed under the MIT-CMU License. It targets the public behavior
of [Pillow](https://python-pillow.org/), whose license and attribution remain
in [`LICENSE`](LICENSE). The pinned `fontdone` checkout and `image-slash-star`
dependency retain their own licenses and notices.
