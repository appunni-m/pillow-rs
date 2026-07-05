# Contributing

`freetype` is a harness-first pure-Rust FreeType parity project.
Correctness and reproducibility matter more than clever shortcuts.

## Development Setup

The crate's MSRV is Rust 1.87. The checked-in `rust-toolchain.toml` pins Rust
1.96.1 for day-to-day development and primary CI gates; CI also runs a 1.87
MSRV test lane.

```bash
make build
make test
make fmt
make clippy
```

Run the complete local gate before handing off a change:

```bash
make ci
```

Optional supply-chain checks:

```bash
make setup
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
make test-parity
make test-ffi
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
make bench-self-test
make bench-quick
make test-perf
```

Performance reports must keep raw samples, machine metadata, trust labels,
timing boundaries, and workload profiles visible.

## Documentation

- Use rustdoc for public APIs.
- Use short comments only for non-obvious implementation decisions.
- Put long debugging or process guidance in `doc/`.
- Keep `../docs/REPO_MAP.md` current when important source, harness,
  generator, or project-goal files move.
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
make release-dry-run
```

6. Publish:

```bash
make release
```
