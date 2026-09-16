# Contributing to pillow-rs

Useful contributions include small reproductions, API cases, documentation
corrections, portability fixes, and implementation work. Start with
[maturity](docs/COMPATIBILITY.md) to see the promised scope and
[architecture](docs/ARCHITECTURE.md) to find the right component.

For a large API or dependency change, describe the intended behavior in a
GitHub issue first. A focused fix with a reproducible failing case can go
directly to a pull request. Follow the [code of conduct](CODE_OF_CONDUCT.md);
report vulnerabilities through [security](SECURITY.md).

## Prepare a checkout

Use the pinned Rust 1.96.1 toolchain, Python 3.10 or 3.12, and GNU Make.
Node 22.14.0 is the CI JavaScript runtime. Exact Python, npm, and Cargo
dependencies live in the committed requirements and lockfiles.

```sh
make help
make setup-venv PYTHON=python3.12
make build-parity
```

The isolated environment contains Pillow 12.2.0 as the oracle.
`make build-parity` builds this checkout's replacement without installing
its `PIL` directory over the oracle. For application use, install pillow-rs
into a different environment. The two distributions share the `PIL` namespace.

For documentation-only work, `make docs-setup` is sufficient. See the
[command reference](docs/COMMANDS.md) for setup effects and optional toolchains.

## Change behavior through the public contract

1. Find the operation in the [manifest](pillow-rs/tests/fixtures/manifest.yaml).
2. Add or extend generator-owned input cases for the affected behavior.
3. Implement the Rust core; keep bindings as conversion and delegation.
4. Run the narrow case and diagnose the first source/target divergence.
5. Run the relevant full lanes and collect source-bound changed-line coverage.

```sh
make migration-parity-inputs-check
make migration-parity-case CASE_ID=<case-id>
make migration-parity-test
```

Cases describe inputs. Do not store expected output, hashes, or pass/fail state
in active input files. Never weaken a threshold or remove an incomplete lane
to make a result pass. Keep newly discovered reference behavior in a short
implementation comment or a maintained public contributor guide.

## Validate the change

```sh
make fmt
make clippy
make docs-check
make workflows-check
```

For runtime changes, run `make test` and the relevant coverage/benchmark gates.
It includes multiple backends, Node/browser execution, and reverse Pillow
coverage; it needs the matching host capabilities. Failed and unmeasured
backends remain explicit.

[Coverage](docs/COVERAGE.md) explains collection receipts and changed-line
claims. [Benchmarking](docs/BENCHMARKING.md) explains controlled comparisons.

`make workflows-check` validates Actions YAML, expressions, action inputs, and
job dependencies using checksum-pinned actionlint 1.7.12. The first invocation
downloads the tool into `target/`; later invocations verify the cached archive.
Shell and Python lint remain separate checks. CI runs the workflow check on
every commit.
Do not attach an old coverage context to a new report.

## Send a pull request

Describe the observable problem, the resulting behavior, and the verification
commands and results. Include a minimal case ID and any remaining platform or
backend limitation. Keep unrelated cleanup separate.

Before committing, inspect the diff for generated output, credentials, private
inputs, and local paths. Regenerate the source map with `make repo-map-update`
when maintained files change. Documentation belongs in the public guides;
superseded plans and session diaries belong in Git history.

Publication is separate from contribution. Maintainers follow
[Releasing](RELEASING.md) after CI passes for the exact source commit.
