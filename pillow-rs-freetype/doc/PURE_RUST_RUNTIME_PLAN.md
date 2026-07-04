# Pure Rust Runtime And Harness Plan

Goal: make `pillow-rs-freetype` succeed only by matching FreeType C exactly from Rust code.

This is not a plan to get a high percentage. It is a plan to build a harness where the Rust implementation has no escape route: no runtime C fallback, no skipped oracle, no threshold called success, no missing raw bytes, no unexecuted fixture family counted as parity, and no undocumented one-off generator needed to reproduce fixtures.

## Success Definition

The project is successful when all in-scope public FreeType endpoint families have executable Rust parity gates and every gate is exact:

- Rendered output: raw pixel bytes plus width, rows, pitch, left, top, bbox, and advance.
- Scalar APIs: exact numeric/string/flag values.
- SFNT/table APIs: exact raw bytes and parsed fields where exposed.
- Error paths: exact behavior for invalid inputs.
- Interface coverage: every in-scope mapped path reports `passing == total`.
- Runtime boundary: no FreeType C link, bridge, or FFI in the crate runtime.

## Runtime Boundary Plan

Status: enforced by `tests/no_runtime_ffi.rs`.

Rules:

1. No runtime `build.rs` C compilation or C linking.
2. No `extern "C"` blocks in `src/`.
3. No `native_ft` bridge modules.
4. No `freetype-sys`, `bindgen`, `pkg-config`, `cc::`, or `rustc-link-lib=freetype` in runtime crate files.
5. `BitmapBackend::PIL` and `BitmapBackend::FreeType` are Rust behavior modes, not FFI selectors.

Allowed C use:

1. Vendored FreeType source for reading and audits.
2. `scripts/` fixture generators that write C-oracle references.
3. Test-local scalar oracle helpers that are compiled for tests only and not linked into runtime.

## Harness Plan

### Gate 1: Runtime Boundary

Current gate:

```bash
cargo test -p pillow-rs-freetype --test no_runtime_ffi --locked
```

Intent:

- Fail immediately if runtime C, FFI, or native bridge hooks return.
- Keep C reference tooling outside runtime files.

### Gate 2: Harness Contract

Current gate:

```bash
cargo test -p pillow-rs-freetype --test harness_contract --locked
```

Intent:

- Lock broad matrix sizes and operation counts.
- Require raw bytes for exact render gates.
- Prevent `native_tt_default` from pretending to be complete.
- Keep present-but-unexecuted matrices named as debt.

Next promotions:

1. Add operation-specific contract checks for `metrics_only`, `outline_cbox`, `no_hinting`, `render_mono`, `render_lcd`, and `render_lcd_v`.
2. Fail the contract if any implemented endpoint has no executable parity family.
3. Fail the contract if any exact gate can pass using SHA-only or size-only fallback.

### Gate 3: Generator Reproducibility

Current gate:

```bash
cargo test -p pillow-rs-freetype --test generator_contract --locked
```

Intent:

- Treat fixture generators as maintained harness code.
- Ensure every maintained generator is documented in `doc/GENERATOR_SYSTEM.md`.
- Ensure every fixture family is registered in the main generator and C oracle helper.
- Reject generated Python bytecode or scratch artifacts under `scripts/`.

Plan:

1. Keep `doc/GENERATOR_SYSTEM.md` as the reproduction source of truth.
2. Route new fixture families through `scripts/gen_ft_refs.c` and `scripts/build_ft_fixture.py` by default.
3. Add dedicated generators only when the reason is documented.
4. Require generator updates in the same change as new committed fixtures.

### Gate 4: Exact Matrix Runner

Current exact runner:

```bash
cargo test -p pillow-rs-freetype --test coverage_matrix_tests --locked
```

Current exact matrix:

- `force_autohint_matrix.json`: 22,168 active rows.

Current incomplete matrix:

- `native_tt_default_matrix.json`: `3176/7640`, threshold baseline only.

Plan:

1. Keep `force_autohint_matrix.json` exact and broad.
2. Promote `native_tt_default_matrix.json` from threshold to exact.
3. Add execution support for `no_hinting_matrix.json`.
4. Add execution support for `metrics_only_matrix.json`.
5. Add execution support for `outline_cbox_matrix.json`.
6. Add execution support for `render_mono_matrix.json`.
7. Add execution support for `render_lcd_matrix.json`.
8. Add `render_lcd_v_matrix.json` if LCD_V is in scope.

Promotion rule: once a matrix is executable, it must fail on any row mismatch. Threshold mode is temporary debt only.

### Gate 5: Render Mode Matrix

Current gate:

```bash
cargo test -p pillow-rs-freetype --test render_mode_matrix --locked
```

Intent:

- Compare render-mode raw bytes and metadata.
- Prevent tests from regenerating references from Rust output.

Plan:

1. Keep fixture generation in explicit C-oracle scripts.
2. Expand beyond the current 16-row matrix without replacing it.
3. Feed mono, LCD, and LCD_V fixture families into the unified matrix runner.

### Gate 6: Fixed Math

Current gate:

```bash
cargo test -p pillow-rs-freetype --test fixed_parity --locked
```

Intent:

- Mandatory C-oracle comparison for fixed-point arithmetic.
- No skipped `/tmp/ftecho` dependency.

Plan:

1. Keep the current exhaustive spot-domain checks.
2. Add generated edge-case fixtures for overflow boundaries.
3. Extend the same mandatory scalar-oracle pattern to vector, matrix, and trigonometric FreeType math when implemented.

### Gate 7: Interface Coverage

Current gate:

```bash
cargo test -p pillow-rs-freetype --test interface_coverage --locked
```

Intent:

- Every public `FT_EXPORT` symbol is mapped.
- Every mapped path has truthful status.
- `native_tt_default` cannot report `7640/7640` until it is actually exact.

Plan:

1. Require executable parity family references for every `complete` endpoint.
2. Require `passing == total` for every complete fixture-backed path.
3. Keep partial endpoints partial until their exact gates exist and pass.

## Execution Order

1. Preserve runtime purity.
2. Preserve generator reproducibility.
3. Preserve exact existing gates.
4. Convert unexecuted fixture families into executable gates.
5. Fix Rust implementation failures exposed by those gates.
6. Promote `native_tt_default` from threshold baseline to exact gate.
7. Expand endpoint coverage only with matching C-oracle fixtures and default test execution.

## Required Verification

Run before claiming progress:

```bash
cargo fmt --all --check
cargo test -p pillow-rs-freetype --locked
cargo clippy -p pillow-rs-freetype --all-targets --locked -- -D warnings
```

Run before claiming project-level parity:

```bash
cargo test -p pillow-rs-freetype --test no_runtime_ffi --locked
cargo test -p pillow-rs-freetype --test harness_contract --locked
cargo test -p pillow-rs-freetype --test generator_contract --locked
cargo test -p pillow-rs-freetype --test coverage_matrix_tests --locked -- --nocapture
cargo test -p pillow-rs-freetype --test render_mode_matrix --locked
cargo test -p pillow-rs-freetype --test fixed_parity --locked
cargo test -p pillow-rs-freetype --test interface_coverage --locked -- --nocapture
```

Passing tests are not enough if the plan still lists threshold or unexecuted debt. Those debts must be promoted into exact gates.
