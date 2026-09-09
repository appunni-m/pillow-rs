# Contributing to pillow-rs

The project has one maintained Rust workspace, one manifest-driven parity
contract, and thin Python/WASM boundaries. Read
[`docs/REPO_MAP.md`](docs/REPO_MAP.md) and the
[documentation checklist](docs/DOCUMENTATION_CHECKLIST.md) before changing a
source, fixture, benchmark, or public page.

## Toolchain

| Tool | Version or source | Reason |
| --- | --- | --- |
| Rust | `1.96.1` from `rust-toolchain.toml` | workspace build, test, fmt, and clippy |
| Python | 3.10 or 3.12 for parity | Pillow 12.2.0 oracle support |
| Python package floor | `>=3.8`, `abi3-py38` | published wheel compatibility |
| Node.js | 22.14.0 in CI and release | WASM and browser adapter |
| maturin | 1.14.1 | PyO3 build |
| wasm-pack | 0.15.0 | WASM build |
| Pillow | 12.2.0 | live oracle and reverse coverage |
| NumPy / PyYAML / coverage | 2.2.6 / 6.0.3 / 7.10.7 | fixed parity toolchain |

The exact Python tools are in [`requirements-ci.txt`](requirements-ci.txt), the
exact npm graph is in `pillow-rs-js/package-lock.json`, and Cargo uses the
committed `Cargo.lock`. Normal build commands use the root Makefile.

## Set up a checkout

```sh
make setup-venv PYTHON=python3.12
make build
python -c "from RSPIL import Image; print(Image.new('RGB', (10, 10)))"
```

The setup target creates an isolated environment and installs the fixed parity
tools. Use Python 3.10 or newer for the full oracle suite; a wheel may still be
consumed by Python 3.8 through its ABI3 boundary.

Build and inspect the WASM package with:

```sh
make build-wasm-release
cd pillow-rs-js && npm run test:package
```

## Architecture rules

- `pillow-rs/` owns image data, formats, operations, pipelines, and backend
  routing. Runtime code is safe Rust and has no Python, JavaScript, filesystem,
  or network dependency.
- `pillow-rs-py/` and `pillow-rs-js/` are conversion and delegation layers.
  Keep algorithms, loops, and business decisions in the Rust core.
- Drawing writes the image's native pixel format. Do not convert to RGBA as a
  temporary representation.
- CPU, SIMD, and GPU paths must preserve the same observable result. A fallback
  must be explicit in execution evidence.
- Do not add runtime FFI or native FreeType calls. The pinned `fontdone` source
  is an oracle and pure-Rust implementation boundary, not a runtime shortcut.

## Manifest-driven changes

The active public contract is
[`pillow-rs/tests/fixtures/manifest.yaml`](pillow-rs/tests/fixtures/manifest.yaml).
For a new or changed operation:

1. update the manifest's source, target, requirements, and applicable profiles;
2. regenerate input-only parity, coverage, and benchmark documents;
3. add cases that exercise both the success path and meaningful edge behavior;
4. implement the Rust core and keep bindings as delegation;
5. run the narrow lane and inspect the first divergence; and
6. run the all-backend lane, coverage, and the repository checks.

Use the generators and validators through Make:

```sh
make migration-parity-inputs
make migration-parity-inputs-check
make migration-parity-fixtures-check
```

Do not put expected values, output hashes, or run status in active inputs. Those
belong to result artifacts under `build/migration-parity/`.

## Testing

Run the smallest relevant target first:

```sh
make migration-parity-case MIGRATION_PARITY_CASE=<case-id>
make migration-parity-test MIGRATION_PARITY_CASE_IDS=<case-id>
make migration-parity-coverage MIGRATION_COVERAGE_CASE_IDS=<case-id>
```

Then run the shared public corpus:

```sh
make migration-parity-test
make migration-parity-test-all-backends
make test
```

`make test` runs target parity, Node/browser WASM parity, reverse Pillow source
coverage, and the ordered missing-feature manifest. It can take several minutes
and may require a GPU/browser-capable runner. Incomplete backends remain
classified in the JSON result.

Validate evidence and source identity with:

```sh
make migration-parity-evidence-check
make migration-parity-receipt-test migration-parity-coverage-receipt-test
make migration-parity-inputs-check
make repo-map-check
```

Coverage is trusted only when an input-driven parity case reaches the target.
Changed-line reports must use a source-bound LCOV receipt. WGSL source lines
remain unmeasured until shader instrumentation exists.

## Formatting and linting

```sh
make fmt
make clippy
make lint
```

The workspace denies unsafe code, unused items, `unwrap`/`expect` in production,
and selected correctness/performance lints. The documented PyO3 deprecation
exception is temporary and must not be broadened to silence new warnings.

## Benchmarking

Read [`docs/BENCHMARKING.md`](docs/BENCHMARKING.md) before interpreting a timing
result. The benchmark runner uses a correctness gate, fixed input identities,
warmup and repeated samples, and a five-percent budget policy.

```sh
MIGRATION_BENCHMARK_PROFILE=quick make migration-parity-benchmark
MIGRATION_BENCHMARK_PROFILE=standard make migration-parity-benchmark
make migration-parity-pipeline-report
```

Do not compare runs with different modes, dimensions, cache states, build
profiles, requested backends, or manifest/input hashes. Keep timing noise and
budget violations visible.

## Documentation and repository hygiene

Use `docs/DOCUMENTATION_CHECKLIST.md` for maintained pages and claim labels.
`docs/generated/` is generated; do not hand-edit it. Dated reports and
`docs/superpowers/` are historical. The `deprecated/` trees are read-only
provenance with an explicit migration map. Review references before deleting a
file and run `make repo-map-update` after a source-tree move.

## Release checks

Version, packaging, and publication are separate steps:

```sh
make release-check
```

This builds and inspects package artifacts without publishing. Publishing
targets require `RELEASE_CONFIRM=1` and protected credentials; follow
[`docs/CI_CD_RELEASE_PLAN.md`](docs/CI_CD_RELEASE_PLAN.md) for the registry
order, tag, provenance, and future GitHub Pages workflows.

## Commit checklist

- [ ] Source and fixture changes follow the manifest and input-only contract.
- [ ] The first divergence and the corresponding reference behavior are
      documented in code or a current project note.
- [ ] `make migration-parity-inputs-check` passes.
- [ ] Relevant parity, coverage, benchmark, and package checks pass.
- [ ] `make fmt`, `make clippy`, and `make repo-map-check` pass.
- [ ] Documentation uses measured/declared/planned/not-proven labels.
- [ ] No generated output, cache, expected result, or local metadata is staged.
