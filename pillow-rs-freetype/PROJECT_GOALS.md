# Project Goals

`pillow-rs-freetype` exists to prove that a Rust implementation can match FreeType C behavior without calling FreeType at runtime.

## Non-Negotiable Direction

- Runtime implementation is 100% Rust.
- FreeType C is the reference oracle, not the runtime engine.
- Pixel and byte parity with FreeType C is the target, not approximate visual similarity.
- Existing broad fixture coverage must not be reduced to smaller smoke tests.
- Any temporary threshold test must be named as a gap and have a path to exact parity.

## What Counts As Parity

- Render endpoints require exact pixel/byte comparison against FreeType-derived fixtures.
- Bitmap metadata requires exact width, rows, pitch, left, top, bbox, and advance comparison.
- Scalar endpoints require exact value comparison.
- Table endpoints require raw byte comparison.
- Error behavior requires explicit invalid-input fixtures.

## Current Runtime Boundary

- No runtime `build.rs` C compilation or C linking.
- No `extern "C"` runtime bindings.
- No `native_ft` bridge.
- No `freetype-sys`, `bindgen`, `pkg-config`, or `cc` runtime dependency.
- C helpers under `scripts/` are allowed only to generate/update reference fixtures.

## Current Parity Baselines

- `force_autohint_matrix.json`: exact Rust-vs-FreeType fixture parity for the broad force-autohint getmask/getbbox matrix.
- `render_mode_matrix.json`: exact byte and metadata parity for the current render-mode matrix.
- `native_tt_default_matrix.json`: still a threshold baseline. This is not done; it must be promoted to exact parity by improving the Rust bytecode/default TrueType path.

## Harness Contract

- Exact parity gates must fail if the matrix is missing.
- Exact render rows must compare raw bytes, not only hashes, dimensions, or thresholds.
- Fixture regeneration must be done by explicit C-oracle scripts, not by test code that blesses Rust output.
- Threshold baselines must remain named and documented as incomplete until they reach exact parity.
- Contract tests must lock current matrix breadth so broad coverage cannot quietly shrink.

## Required Habit

When changing the renderer, scaler, bytecode hinter, rasterizer, or fixtures:

1. Keep broad matrices broad.
2. Prefer adding rows over replacing broad coverage with spot checks.
3. Use C only to produce expected data.
4. Make Rust output match the C expected data.
5. Document any remaining threshold or partial parity as unfinished work.
