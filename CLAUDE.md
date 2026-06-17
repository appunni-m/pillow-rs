# pillow-rs Development Instructions

## Architecture

Workspace with three crates:
- `pillow-rs/` — Pure Rust, all image logic, ZERO binding dependencies
- `pillow-rs-py/` — PyO3 bindings, thin wrapper (~200 lines max)
- `pillow-rs-js/` — wasm-bindgen, thin wrapper (~200 lines max)

**Iron rule:** Core never touches Python objects, JS objects, file paths, or network. Core takes Rust primitives, returns Rust primitives. All I/O and type conversion lives in binding crates.

## Python Binding Rules

`pillow-rs-py/python/pillow_rs/` MUST be thin wrappers:
- **NO** `for`/`while` loops, list comprehensions, `import math/os/subprocess/tempfile`
- **NO** arithmetic (`+`, `-`, `*`, `/`, `min`, `max`, `sorted`, `sum`)
- **NO** `if/elif/else` beyond isinstance checks, None defaults, or mode dispatch
- All logic in `pillow-rs/src/`; bindings delegate via `_core.xxx()` or `_rust_image.xxx()`

## Drawing Architecture

**Iron rule: Draw directly in the image's native pixel format. NEVER convert to RGBA for drawing.**

Every draw function dispatches on canvas type:
```
Luma8 (1 byte/px) | LumaA8 (2 bytes/px) | Rgb8 (3 bytes/px) | Rgba8 (4 bytes/px)
```

Mode-specific color: see `pillow-rs/src/draw/mod.rs` dispatch table for per-mode `fill=X` semantics.

## Logging

Use `log` crate macros. NEVER `eprintln!` or `println!` in library code.

| Level | When | Example |
|-------|------|---------|
| `log::error!` | Failures, corrupt data | `log::error!("JPEG: invalid Huffman table at offset {}", off);` |
| `log::warn!` | Recoverable issues, fallbacks | `log::warn!("Unknown EXIF tag, skipping");` |
| `log::info!` | High-level operations | `log::info!("Opening {}×{} {} image", w, h, mode);` |
| `log::debug!` | Algorithm steps, backend selection | `log::debug!("[GPU] {} op(s) {}×{}", ops.len(), w, h);` |
| `log::trace!` | Internal per-scan/pixel detail | `log::trace!("progressive: S[{}] ss={}", idx, ss);` |

**Rules:**
- Prefix messages with context: `"progressive:"`, `"[GPU]"`, `"[SIMD]"`, module name
- Core crates NEVER initialize a logger — bindings do that (`pyo3-log`, `console_log`)
- Test files can use `eprintln!` for progress output
- New core crates must add `log = "0.4"` to `Cargo.toml`

## Rust Code Style

Delegate to `rust-development` skill. Key repo specifics:
- `thiserror` for errors, never `unwrap()`/`expect()` outside tests
- `&str` over `String`, `&[T]` over `Vec<T>` in parameters
- `cargo clippy --all-targets --all-features -- -D warnings` before commit

## Manifest-Driven Development

All work starts from `manifest.yaml` — the single source of truth for the API surface.

**Adding a function:**
1. Add entry to `manifest.yaml` (signature, modes, variants)
2. `scripts/generate_stubs.py` → creates stub in core
3. Implement in `pillow-rs/src/ops/<module>.rs`
4. Add binding delegation in `pillow-rs-py/src/lib.rs`
5. Add Python wrapper in `pillow-rs-py/python/pillow_rs/`
6. Register in `__init__.py` + `ops/mod.rs` if new module
7. Write PIL parity test using `assert_images_equal()` / `assert_values_equal()`
8. Add JSON fixture in `tests/fixtures/` with `operation.module` + `operation.target`
9. Run tests + `scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json`

## Building & Testing

```bash
# Python
maturin develop --manifest-path pillow-rs-py/Cargo.toml

# WASM
wasm-pack build --target web  # from pillow-rs-js/

# Core tests
cargo test -p pillow-rs

# Full build + test (always use this — handles fixtures safely)
bash scripts/build_and_test.sh        # Suite0
bash scripts/build_and_test.sh 1      # Suite1
bash scripts/build_and_test.sh all    # All suites
bash scripts/lint.sh                  # fmt → clippy → tests → trust report
```

**NEVER `rm -rf` manually** — fixtures are read-only. Use the scripts.
**NEVER edit fixture output files** — edit the generator instead.

## Coverage

Trust-based binary: function is TRUSTED if ≥1 PIL parity test passes.
- Map: `scripts/coverage/coverage_map.json` (`"test_name": ["Module.function"]`)
- Report: `python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json`
- Tests that only verify signatures/stubs don't count — must be PIL parity

## Rules

- Public API names match Pillow exactly. Import name: `RSPIL`.
- Reference: **Pillow** for API, **Puhu** (`puhu/`) for algorithms/quirks
- NEVER use git (`commit`, `checkout`, `revert`, `stash`) without explicit permission
- NEVER change fixture output/input JSON images or binaries
- `pillow-rs-py` must contain NO `if`/`else` — all logic in core
- Never leave commit message as "anthropic" or "fable"
- Run only failing tests; remove `show()` function
- Timeout tests at 3 minutes
- Research exact algorithms via internet when needed
- Write separate code paths per mode when needed
- Don't give tasks back to user — do it all yourself
- Add tasks to task list and don't stop until done
