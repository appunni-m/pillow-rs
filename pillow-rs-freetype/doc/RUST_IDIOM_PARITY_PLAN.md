# Rust Idiom + FreeType Parity Plan

Goal: keep `pillow-rs-freetype` pixel-compatible with FreeType while moving
the codebase toward Rust idioms. Pixel parity is the hard constraint; style
changes must be mechanical or covered by fixture parity tests.

## Required Gate

Use fixture/parity tests as the verification gate, not unit tests alone.

Primary gate for this branch:

```bash
cargo test -p pillow-rs-freetype --test direct_ft_compare -- --test-threads=1 --nocapture
```

Current exact-branch baseline:

- Live FreeType fixture comparison: `11084/11084 passed`.
- `cargo clippy -p pillow-rs-freetype --all-targets`: passes.
- Static PIL matrix currently fails at `2149/7640 passed`; treat that as a
  separate stale/static-fixture issue unless the task is specifically about
  `coverage_matrix.json`.

## Working Rules

1. Work one module or harness cluster at a time.
2. Preserve FreeType truncation, wrapping, hinting, and raster behavior.
3. Prefer named conversion helpers or local test-harness lint allowances over
   broad production-code allowances.
4. Keep debug examples and fixture harnesses compiling under all-target Clippy.
5. After every behavioral change, rerun `direct_ft_compare`.

## Next Phases

1. Keep all-target linting green while reducing local test/example allowances.
2. Audit `coverage_matrix.json` versus live FreeType/PIL generation and decide
   whether to regenerate or retire the stale matrix.
3. Continue production refactors module by module: `tt/hinter`, `autohint`,
   raster/scaler, then public font surface.
4. For any production logic change, capture before/after fixture summaries and
   never accept a lower live FreeType pass count.
