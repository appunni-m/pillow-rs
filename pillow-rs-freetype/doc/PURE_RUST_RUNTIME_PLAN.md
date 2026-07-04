# 100% Rust Runtime Plan

Goal: `pillow-rs-freetype` must build, test, and run without runtime FreeType FFI while preserving the real challenge: exact FreeType C pixel/byte parity from Rust code. C FreeType is allowed only as an offline fixture generator under `scripts/` and as vendored reference source for audits.

## Runtime Boundary

- No `build.rs` that compiles or links C for `pillow-rs-freetype`.
- No `extern "C"` blocks in `src/`.
- No `native_ft` bridge modules.
- No `freetype-sys`, `bindgen`, `pkg-config`, `cc::`, or `rustc-link-lib=freetype` in runtime crate files.
- `BitmapBackend::PIL` and `BitmapBackend::FreeType` are Rust behavior modes, not FFI selectors.

## Current Execution

1. Delete `src/native_ft.rs` and `src/native_ft.c`.
2. Delete the runtime `build.rs` C compilation/link step.
3. Route PIL/default rendering through the existing Rust scaler, TrueType hinting, and gray rasterizer path.
4. Add `tests/no_runtime_ffi.rs` to prevent reintroducing runtime FFI hooks.
5. Keep C fixture scripts in `scripts/` as offline oracle tooling only.

## Parity Milestones

1. Keep broad coverage broad; never replace an exhaustive matrix with a small smoke test.
2. Keep `force_autohint_matrix.json` exact: 11,084 `getmask` rows and 11,084 `getbbox` rows.
3. Promote `native_tt_default_matrix.json` from threshold baseline to exact Rust bytecode parity.
4. Expand `render_mode_matrix.json` beyond smoke coverage for normal, mono, LCD, and LCD_V.
5. Add scalar endpoint fixture matrices for face, size, charmap, SFNT, metrics, and error behavior.
6. Update `interface_map.json` so every implemented endpoint references a parity family.

## Required Gates

```bash
cargo fmt --all --check
cargo test -p pillow-rs-freetype --locked
cargo clippy -p pillow-rs-freetype --all-targets --locked -- -D warnings
cargo check --workspace --locked
cargo test -p pillow-rs-freetype --test no_runtime_ffi --locked
```

The `no_runtime_ffi` test is the release gate for this plan's runtime boundary.
