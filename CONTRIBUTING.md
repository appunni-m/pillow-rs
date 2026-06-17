# Contributing to pillow-rs

Thank you for helping improve pillow-rs! This guide covers everything you need to contribute effectively.

## Quick links

- [Issues](https://github.com/pillow-rs/pillow-rs/issues) — bug reports, feature requests
- [Discussions](https://github.com/pillow-rs/pillow-rs/discussions) — questions, ideas, community

---

## Getting started

### Prerequisites

- **Rust 1.75+** — install via [rustup](https://rustup.rs/)
- **Python 3.8+** with Pillow (`pip install pillow`)
- **Node.js 20+** (for WASM targets)
- `maturin` — `pip install maturin`
- `wasm-pack` — `cargo install wasm-pack`

```bash
# Clone
git clone https://github.com/pillow-rs/pillow-rs
cd pillow-rs

# Install system deps (Ubuntu/Debian)
sudo apt-get install -y fonts-dejavu-core

# Install Python test deps
pip install pillow numpy pyyaml pytest pytest-benchmark pytest-json-report
```

### Build all targets

```bash
# Python
cd pillow-rs-py && maturin develop --release && cd ..

# WASM
cd pillow-rs-js && wasm-pack build --target web && cd ..

# Core
cargo test -p pillow-rs-core
```

---

## Development workflow

pillow-rs is **manifest-driven**. All work starts from `manifest.yaml`.

### Adding a new function

1. **Define the API** — add the function entry to `manifest.yaml` (signature, modes, variants, edge cases)
2. **Generate stubs** — run `python scripts/generate_stubs.py` to scaffold Rust stubs in core
3. **Implement** — fill in the stub in `pillow-rs-core/src/ops/<module>.rs`
4. **Add binding** — add delegation in `pillow-rs-py/src/lib.rs`
5. **Add Python wrapper** — thin wrapper in `pillow-rs-py/python/pillow_rs/` (no loops, no arithmetic; pure delegation)
6. **Register** — add the new module in `pillow-rs-py/python/pillow_rs/__init__.py`
7. **Update mod.rs** — add `pub mod <module>;` in `pillow-rs-core/src/ops/mod.rs`
8. **Write tests** — PIL parity tests in `tests/` using `assert_images_equal()` or `assert_values_equal()`
9. **Add fixture** — JSON fixture in `tests/fixtures/` with `operation.module` + `operation.target`
10. **Map coverage** — add entry to `scripts/coverage/coverage_map.json`
11. **Verify** — run tests and coverage (see below)

### Fixing a failing PIL parity test

1. Run the specific test to see what's failing:
   ```bash
   python -m pytest tests/ -k "test_name_here" -v
   ```
2. Research the Pillow behavior — check [Pillow source](https://github.com/python-pillow/Pillow) or docs
3. Fix the Rust implementation in `pillow-rs-core/src/ops/`
4. Re-run the test to confirm it passes
5. Run the full suite to check for regressions

### Architecture rules

**Iron rule:** Core never touches Python objects, JS objects, file paths, or network.

| Layer | Can do | Cannot do |
|-------|--------|-----------|
| `pillow-rs-core` | Image processing, math, algorithms | Python/JS objects, I/O, file paths |
| `pillow-rs-py` | PyO3 type conversion, file I/O | Loops, arithmetic, image logic |
| `pillow-rs-js` | wasm-bindgen type conversion, JS interop | Loops, arithmetic, image logic |

**Python binding rules** — binding files in `pillow-rs-py/python/pillow_rs/` must be thin wrappers:
- ❌ NO `for`/`while` loops
- ❌ NO list comprehensions
- ❌ NO `import math`, `import tempfile`, `import os`
- ❌ NO arithmetic (`+`, `-`, `*`, `/`, `min`, `max`, `sorted`, `sum`)
- ❌ NO `if`/`elif`/`else` beyond `isinstance` checks, None defaults, or mode dispatch
- ✅ All logic lives in `pillow-rs-core/src/`

---

## Code style

### Rust

```bash
# Auto-format
cargo fmt

# Lint (must pass with zero warnings)
cargo clippy --all-targets --all-features -- -D warnings
```

Checklist:
- [ ] No `unwrap()` or `expect()` outside `#[cfg(test)]`
- [ ] `thiserror` for error types, never bare `anyhow` in core
- [ ] `&str` over `String`, `&[T]` over `Vec<T>` in parameters
- [ ] `#[derive(Debug)]` on all public types
- [ ] `///` doc comments with `# Examples` on all `pub` functions
- [ ] Import order: `std` → external crates → `crate` → `super` → `self`
- [ ] `#[cfg(not(target_arch = "wasm32"))]` guard for rayon
- [ ] No redundant `.clone()` — prefer borrowing

### Python (binding layer only)

- Pure delegation — every method calls `_core.xxx()` or `_rust_image.xxx()`
- No logic, no math, no loops

---

## Testing

### Running tests

```bash
# Full PIL parity suite
python -m pytest tests/ --json-report --json-report-file=/tmp/report.json

# Single test
python -m pytest tests/ -k "test_name" -v

# Core Rust tests
cargo test -p pillow-rs-core
```

### Test structure

Tests live in `tests/` and validate **PIL-RSPIL parity** — identical inputs, same operation, compare outputs:

- `assert_images_equal(rs_img, pil_img)` — pixel-exact image comparison
- `assert_values_equal(rs_val, pil_val)` — non-image output comparison

Every test must:
1. Create identical inputs for both PIL and RSPIL
2. Run the same operation on both
3. Assert binary-identical output

Tests use `@pytest.mark.covers("Module.function", mode=..., variant=...)` markers for self-documentation.

### Coverage

```bash
# Generate fresh test report
python -m pytest tests/ --json-report --json-report-file=/tmp/report.json

# Compute coverage
python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json

# Generate coverage pages
python scripts/coverage/generate_coverage_page.py   # → docs/COVERAGE.md
python scripts/coverage/generate_wasm_coverage.py   # → docs/COVERAGE_WASM.md
```

Adding a new test requires an entry in `scripts/coverage/coverage_map.json` mapping the test name to its function(s).

---

## Benchmarking

```bash
# Full suite
bash scripts/bench/bench_all.sh full

# Incremental (only changed code)
bash scripts/bench/bench_all.sh incremental

# Specific functions
bash scripts/bench/bench_all.sh --only resize,crop
```

Benchmarks use SHA-256 cache keys — unchanged functions skip re-benchmarking. Output: `BENCHMARKS.md`.

---

## Commit guidelines

- Commits are on feature branches — never commit directly to `main`
- Run `cargo clippy --all-targets --all-features -- -D warnings` before committing
- Run `cargo fmt` before committing
- Run the full test suite and check for regressions
- Commit messages describe what changed and why

---

## License

By contributing to pillow-rs, you agree that your contributions will be licensed under the same [MIT-CMU License](LICENSE) as the project.

---

## Questions?

Open a [Discussion](https://github.com/pillow-rs/pillow-rs/discussions) or [Issue](https://github.com/pillow-rs/pillow-rs/issues) — we're happy to help.
