# Contributing

`pillow-rs-freetype` is a harness-first pure-Rust FreeType parity project.
Correctness and reproducibility matter more than clever shortcuts.

## Development Setup

```bash
cargo build --locked
cargo test --locked
cargo fmt -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
```

The Makefile wraps the same commands:

```bash
make ci
```

Optional supply-chain checks:

```bash
cargo install cargo-deny cargo-audit --locked
make supply-chain
```

## Architecture Rules

- Runtime code is pure Rust.
- No unsafe code.
- No runtime FreeType FFI, native build hooks, `freetype-sys`, `bindgen`,
  `pkg-config`, `cc`, `extern "C"`, or `dlopen`.
- Public APIs take Rust primitives and in-memory data. File and process access
  belongs in examples, tests, scripts, or caller code.
- C FreeType is allowed only as an offline oracle for fixtures, diagnostics,
  and trace comparison.
- Fixture generation is maintained infrastructure, not one-off scripting.

## Parity Workflow

When a glyph, metric, bbox, or bitmap differs from C FreeType:

1. Pick one font, one glyph, one size, one endpoint.
2. Dump Rust and C at the same pipeline stages.
3. Find the first divergent value.
4. Read the exact C function that produced the oracle behavior.
5. Fix the Rust root cause.
6. Re-run the narrow lane, then the full matrix.

Useful commands:

```bash
cargo test --test coverage_matrix_tests --locked -- --nocapture
cargo test --test no_runtime_ffi --locked -- --nocapture
```

Never clamp output, special-case a glyph, delete a fixture row, or weaken a
threshold to make a lane pass.

## Fixture Changes

Before changing fixtures or generators, read:

- `PROJECT_GOALS.md`
- `doc/GENERATOR_SYSTEM.md`
- `doc/REFERENCES.md`

Fixture updates must be reproducible through documented commands and generated
from the C oracle. Rust output is never the expected reference.

## Benchmark Changes

Before changing benchmark code or reporting speedups, read:

- `doc/PERFORMANCE_BENCHMARKING.md`
- `doc/PERFORMANCE_DOCUMENTATION_REFACTOR_PLAN.md`

Required validation:

```bash
PYTHONPYCACHEPREFIX=target/pycache python3 -m py_compile scripts/bench_freetype.py
python3 scripts/bench_freetype.py --self-test
python3 scripts/bench_freetype.py --compare-c --samples 2 --table
cargo test --test perf_benchmark_contract --locked
```

Performance reports must keep raw samples, machine metadata, trust labels,
timing boundaries, and workload profiles visible.

## Documentation

- Use rustdoc for public APIs.
- Use short comments only for non-obvious implementation decisions.
- Put long debugging or process guidance in `doc/`.
- Update `CHANGELOG.md` for user-visible runtime, harness, fixture, benchmark,
  or release changes.

## Commit And PR Expectations

- Explain what changed and why.
- Include before/after parity counts for parity fixes.
- Include benchmark commands and machine/report metadata for performance claims.
- Confirm no runtime FFI and no fixture/test weakening.

## Release Checklist

1. Update `Cargo.toml` version.
2. Update `CHANGELOG.md`.
3. Run `make ci`.
4. Run `make supply-chain` when audit tools are available.
5. Run a publish dry run:

```bash
cargo publish --dry-run --locked
```

6. Publish:

```bash
cargo publish --locked
```
