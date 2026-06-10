# pillow-rs Development Instructions

## Project Overview

A full-featured reimplementation of Pillow in Rust targeting **Python** (via PyO3) and **WebAssembly/JavaScript** (via wasm-bindgen). Drop-in replacement: `from RSPIL import Image` works identically to `from PIL import Image`.

## Architecture

Workspace with three crates:
- `pillow-rs-core/` — Pure Rust, all image logic, ZERO binding dependencies
- `pillow-rs-py/` — PyO3 bindings, thin wrapper (~200 lines max)
- `pillow-rs-js/` — wasm-bindgen, thin wrapper (~200 lines max)

**Iron rule:** Core never touches Python objects, JS objects, file paths, or network. Core takes Rust primitives, returns Rust primitives. All I/O and type conversion in binding crates.

## Reference Code

- **Puhu** (`puhu/` in this repo) — Rust-based Pillow subset. Reference for algorithms, Pillow compatibility quirks, lazy loading pattern. NOT for architecture (Puhu is monolithic PyO3-only).
- **Pillow** — authoritative API reference. All public names must match exactly.

## Development Workflow

### Rust Code Style
- Follow `rust-development` skill guidelines
- Run `cargo clippy --all-targets --all-features -- -D warnings` before commit
- Use `thiserror` for error types, never `unwrap()` or `expect()` outside tests
- Prefer `&str` over `String`, `&[T]` over `Vec<T>` in function parameters
- Use `rayon` for parallel processing on native targets; `#[cfg(not(target_arch = "wasm32"))]` guard

### Naming Rules
- **Public API names match Pillow exactly** — `Image.open()`, `Image.resize()`, etc.
- **Internal Rust types** can use any naming convention (e.g., `RsImage`, `PyImage`)
- The import name is `RSPIL` (mirrors `PIL`)

### Testing
- Single Python test suite in `tests/` runs against both Pillow and pillow-rs
- Tests use `@pytest.mark.covers("function_name", mode=..., variant=...)` markers
- `manifest.yaml` is the single source of truth for the API surface
- Coverage is computed by `scripts/compute_coverage.py` after each test run

### Building
- Python: `maturin develop --release` (from `pillow-rs-py/`)
- WASM: `wasm-pack build --target web` (from `pillow-rs-js/`)
- Core tests: `cargo test --manifest-path pillow-rs-core/Cargo.toml`

## Key Patterns from Puhu (adopt these)

1. **LazyImage enum** — `Loaded(DynamicImage) | Path(PathBuf) | Bytes(Vec<u8>)`. Defers decode until first operation. Critical for WASM.
2. **Operation immutability** — operations return new `Image` instances; `paste` mutates in-place (matching Pillow semantics).
3. **Parallel chunk processing** — use `par_chunks()` for pixel-level operations on native targets.
4. **Mode-aware fast paths** — check for `Rgb8`, `Rgba8`, `Luma8` before falling back to generic `DynamicImage` methods.

## Manifest-Driven Development

All work starts from `manifest.yaml`. To add a new function:
1. Add its entry to `manifest.yaml` (signature, modes, variants, edge cases)
2. Run `scripts/generate_stubs.py` to create the stub in core
3. Implement the function in `pillow-rs-core/src/ops/<module>.rs`
4. Add binding delegation in `pillow-rs-py/src/lib.rs`
5. Add Python wrapper in `pillow-rs-py/python/pillow_rs/` (Image class, or new module)
6. Register new module in `pillow-rs-py/python/pillow_rs/__init__.py`
7. Update `pillow-rs-core/src/ops/mod.rs` if new module added
8. Write PIL parity tests in `tests/` using `assert_images_equal()` or `assert_values_equal()`
9. **CRITICAL**: Add test name → manifest function mapping in `scripts/compute_coverage.py` `func_name_map` dict — otherwise coverage won't increase
10. Run `python -m pytest tests/ --json-report --json-report-file=/tmp/report.json`
11. Run `python scripts/compute_coverage.py manifest.yaml /tmp/report.json` to verify coverage increased

### Building (correct commands)
- Python: `maturin develop --manifest-path pillow-rs-py/Cargo.toml` (from repo root)
- WASM: `wasm-pack build --target web` (from `pillow-rs-js/`)
- Core tests: `cargo test -p pillow-rs-core`

### Test Requirements
- **Every test must validate PIL-RSPIL parity** — same inputs, same operation, compare outputs
- Use `assert_images_equal(rs_img, pil_img)` for image output comparison (pixel-exact)
- Use `assert_values_equal(rs_val, pil_val)` for non-image output comparison
- For artistic/algorithm-specific operations (filters, drawing), test that output is valid (correct size, mode, no crash)
- Tests that verify signature existence or stub behavior are NOT parity tests — they don't count toward coverage

### Coverage System
- `scripts/compute_coverage.py` maps test function names to manifest entries via hardcoded `func_name_map`
- Every new test function MUST be added to this map or coverage won't reflect the work
- Scoring: 75% base for any passing tests + up to 25% bonus for variant/edge/format completeness
- Coverage report generated after each test run shows per-function pass/fail status
- Use `@pytest.mark.covers(...)` markers for documentation; the script infers coverage from test names