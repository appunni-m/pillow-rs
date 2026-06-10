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
4. Add binding delegation in `pillow-rs-py` and `pillow-rs-js`
5. Write tests with `@pytest.mark.covers(...)` markers
6. Run tests, then `scripts/compute_coverage.py` to update coverage

### Git
- Never sign commits with `Co-Authored-By: Claude` or Anthropic references
- Use your own git identity; the AI provides code, you own the commits