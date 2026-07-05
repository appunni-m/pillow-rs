# Standalone Project Plan

`pillow-rs-freetype` is becoming an independent open-source crate, not a
directory that relies on a parent repository for discipline. The parent project
provided useful defaults; this crate now needs to own those defaults directly.

## Parent Lessons Adopted

- Strict Rust and Clippy lints belong in this crate's `Cargo.toml`.
- CI must split formatting, linting, tests, docs, benchmark contracts, and
  supply-chain checks so failures are easy to triage.
- A Makefile should expose the same commands contributors and CI run.
- `--locked` commands require a crate-local `Cargo.lock` after extraction.
- Fixture generation, benchmark tooling, and parity reports are source code,
  not ad hoc scripts.
- Contributor docs should start from the exact commands a fresh checkout needs.
- Runtime code and binding/integration code must stay separate.

## Standalone Ownership

This crate owns:

- package metadata, repository URL, license, README, changelog, and release flow
- rust toolchain components, lints, formatting, documentation, and clippy policy
- CI and local `make` targets
- cargo-deny and cargo-audit policy
- FreeType C oracle build scripts and fixture generators
- exact parity harnesses and no-runtime-FFI checks
- Rust-vs-C benchmark reports with raw samples and machine metadata
- security, code of conduct, contributing, and agent instructions

Parent projects may consume the crate and run integration-specific tests, but
they are downstream gates. They must not be required to validate this crate's
own correctness, performance contract, or release readiness.

## Extraction Checklist

1. Keep `Cargo.toml` free of workspace-inherited package fields and lints.
2. Commit a crate-local `Cargo.lock` so `cargo test --locked` works in a fresh
   standalone clone.
3. Keep `.github/workflows/ci.yml` runnable from this directory as the
   repository root.
4. Keep `Makefile` targets aligned with CI.
5. Keep active docs using crate-root commands.
6. Run `make ci` before release branches.
7. Run `make supply-chain` when audit tooling is available.
8. Run `cargo publish --dry-run --locked` before publishing.
9. Add badges only after the standalone remote and CI workflow names are final.
10. Keep parent integration tests in downstream repositories or explicit
    integration documentation.

## Release Readiness Gates

The crate is not release-ready unless these pass from the standalone root:

```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --test coverage_matrix_tests --locked -- --nocapture
cargo test --test no_runtime_ffi --locked -- --nocapture
cargo test --locked
PYTHONPYCACHEPREFIX=target/pycache python3 -m py_compile scripts/bench_freetype.py
python3 scripts/bench_freetype.py --self-test
cargo test --test perf_benchmark_contract --locked
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
```

Performance claims additionally require:

```bash
python3 scripts/bench_freetype.py --compare-c --samples 10 --profile default --table
```

## Maintenance Rule

When this crate gains a new public endpoint, fixture family, benchmark row, or
debugging workflow, add the standalone command path at the same time. No future
workflow should require rediscovering a parent-project command.
