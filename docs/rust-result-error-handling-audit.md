# Rust Result error-handling audit

This document tracks the project-wide migration rule:

- fallible Rust APIs should return `Result<_, _>`;
- bindings convert Rust errors into host exceptions at the binding boundary;
- tests may use `unwrap`/`expect` only to assert test invariants;
- silent success fallbacks for real errors are migration targets.

The exhaustive function/method inventory is generated, not hand-maintained:

```bash
python3 scripts/audit_rust_result_methods.py
```

Generated inventory:

- `docs/generated/rust-method-result-audit.tsv`
- Current generated rows: `6,119`
- Current generated scope counts:
  - `production`: `3,324`
  - `test`: `2,758`
  - `example`: `25`
  - `bench`: `12`
- Current generated classification counts:
  - `ok_result`: `2,593`
  - `likely_infallible`: `2,583`
  - `abi_status_code`: `451`
  - `test_assertion_or_fixture_harness`: `176`
  - `abi_optional_snapshot_or_handle`: `85`
  - `freetype_ffi_optional_lookup_or_table`: `74`
  - `freetype_optional_font_feature_parse`: `65`
  - `freetype_optional_sfnt_lookup`: `48`
  - `example_fail_fast_or_smoke_harness`: `8`
  - `pillow_clip_control_flow`: `5`
  - `bench_fail_fast_harness`: `5`
  - `freetype_void_api_internal_status`: `4`
  - `iterator_exhaustion_control_flow`: `3`
  - `documented_invariant_panic`: `3`
  - `freetype_metrics_lookup_absence`: `3`
  - `freetype_sentinel_arithmetic`: `3`
  - `freetype_default_value_control_flow`: `3`
  - `palette_absence_control_flow`: `2`
  - `font_bitmap_absence_control_flow`: `1`
  - `freetype_geometry_absence_control_flow`: `1`
  - `freetype_test_support_sample`: `1`
  - `freetype_raster_clip_control_flow`: `1`
  - `binding_display_fallback`: `1`

Current unresolved generated classification counts:

- `parser_review`: `0`
- `review_non_result_fallible`: `0`
- `review_panic_path`: `0`

## Current interpretation

The generated `classification` column is conservative:

- `ok_result`: function signature already returns `Result`.
- `likely_infallible`: no obvious fallibility signal was detected.
- `review_non_result_fallible`: non-`Result` function contains an obvious fallibility signal such as `unwrap`, `expect`, filesystem/process use, or checked arithmetic.
- `review_panic_path`: non-`Result` function contains `panic!`, `todo!`, or `unimplemented!`.
- `parser_review`: mechanical parser found a mixed signal that requires manual review.
- `abi_status_code`: C/WASM ABI surface returns an explicit status code such as `FT_Error` or `FontdoneWasmStatus`; internal Rust helpers should still use `Result` unless they are direct boundary-status adapters.
- `abi_optional_snapshot_or_handle`: C/WASM ABI inspection helpers and nullable handle lookups use `Option` to model absent handles, absent snapshots, or unsupported optional ABI data.
- `documented_invariant_panic`: the deliberately named `InfallibleExt::because` invariant mechanism; this is not a recoverable user/input error path and has no production call sites after the current audit.
- `binding_display_fallback`: binding display/debug helpers intentionally format fallback strings instead of raising while constructing `repr`.
- `pillow_clip_control_flow`: Pillow drawing geometry helpers use checked arithmetic to clip/skip non-renderable coordinates, matching Pillow-style draw semantics instead of raising.
- `iterator_exhaustion_control_flow`: `Option` is used to represent iterator exhaustion, not an error.
- `palette_absence_control_flow`: `Option` is used for Pillow-compatible palette/transparency absence.
- `font_bitmap_absence_control_flow`: `Option` is used while reading bitmap coverage for out-of-bounds pixels or unsupported bitmap modes that should be skipped.
- `freetype_geometry_absence_control_flow`: `Option` is used for empty-outline geometry absence.
- `freetype_metrics_lookup_absence`: `Option` is used for absent optional metrics/style records where FreeType falls back rather than raising.
- `freetype_ffi_optional_lookup_or_table`: FreeType FFI helpers use `Option` for nullable handle lookups, optional table/service data, and output rows where the C API communicates absence through null/zero/status.
- `freetype_optional_font_feature_parse`: Type 1/BDF/name-feature parsers use `Option` for optional font-program features whose absence is legal and covered by fallback behavior.
- `freetype_optional_sfnt_lookup`: SFNT table helpers use `Option` for optional or out-of-range table records that callers translate into FreeType-compatible absence/fallback.
- `freetype_sentinel_arithmetic`: FreeType-compatible fixed-point math returns C sentinel values for cases such as division by zero, preserving oracle behavior instead of surfacing Rust errors.
- `freetype_void_api_internal_status`: helpers under a FreeType void API path use internal success/failure booleans because the public C-compatible caller intentionally ignores those failures while preserving metric side effects.
- `freetype_default_value_control_flow`: FreeType-compatible variation/default helpers return default vectors or empty values for absent named instances/tables, preserving public oracle behavior.
- `freetype_test_support_sample`: feature-gated ABI test-support sampling helper; not part of the public runtime C/WASM ABI.
- `freetype_raster_clip_control_flow`: rasterizer cell movement uses C-compatible clipping/dumpster control flow instead of reporting a recoverable error.
- `test_assertion_or_fixture_harness`: test-only function uses fail-fast setup/assertion behavior such as `unwrap`, `expect`, `panic!`, filesystem access, or subprocess oracle execution.
- `example_fail_fast_or_smoke_harness`: example executable uses fail-fast setup or smoke-test behavior at the executable boundary.
- `bench_fail_fast_harness`: benchmark harness uses fail-fast setup or measurement helpers; it is not a library API boundary.

The generated `scope` column separates production code from integration tests,
unit-test modules, examples, and benchmark crates. Production rows are the
primary migration queue; test/example/bench rows can keep `unwrap`/`expect`
when they assert setup invariants or fail-fast executable setup.

This file is the queue driver. A row is only considered fixed after source has
been inspected and either:

1. the function is changed to return `Result` and callers are updated; or
2. the function is documented here as intentionally infallible/test-only.

## Completed migrations

- Font public text/layout/rendering APIs now directly return `Result<_, PilError>` without `_result` endpoint aliases.
- Font rendering allocation overflow now returns `PilError::DimensionError` instead of producing empty output.
- JS exported `statFromList` now returns `Result<JsValue, JsValue>` and bubbles `Reflect::set` failures instead of `expect`.
- JS exported `outlineCurve` now returns `Result<Vec<i32>, JsValue>` and rejects negative `steps` instead of panicking on integer conversion.
- PyO3 `PyImage` default constructor now returns `PyResult<Self>` and maps core construction errors instead of `expect`.
- Compute backend activation, inspection, and routing now return `Result<_, PilError>` and report poisoned global backend state as `PilError::InternalError`; Python/JS bindings bubble those errors.
- Raw byte image construction and Pillow grayscale conversion helpers now return `Result<_, PilError>` instead of using `expect` for buffer shape mismatches; Draw image restoration now returns `Result` and bindings bubble failures.
- Legacy compute operation registration helpers now return `Result<_, PilError>` for duplicate keys and poisoned registry state; tests assert the structured error instead of `#[should_panic]`.
- The historical `pillow-rs-image` crate was removed from this repository after
  codec ownership moved to the sibling `image-slash-star` package, so its
  methods are no longer part of this repository's Rust method audit.
- CPU ImageChops invert and auxiliary GPU registry helpers now return
  `Result<_, PilError>` instead of panicking on buffer shape or mutex poisoning.
- SIMD dynamic-image reconstruction helpers now return `Result<_, PilError>`;
  every SIMD adapter bubbles reconstruction failures with `?` instead of
  using `expect` on image buffer creation.
- SIMD palette normalization and CPU effects/geometry transform/rotate helpers
  now return `Result<_, PilError>` instead of panicking or silently returning
  fallback images on dimension/buffer reconstruction failures.
- GPU putalpha output reconstruction now returns `Result<_, PilError>` instead
  of using `expect` when converting the readback image into LA output.
- P-mode quantize helpers now return `Result<_, PilError>` and bubble checked
  dimension failures instead of returning empty palette/index vectors as a
  silent success value.
- Quantize histogram insertion now updates matched entries directly through
  mutable match bindings instead of using `expect` for an already-proven
  internal invariant.
- FreeType scaled glyph loading now returns `Result<GlyphOutline, FontError>`
  through the recursive helper and bubbles invalid `loca`, out-of-range `glyf`,
  simple/composite parse failures, and invalid composite attachment points
  instead of relying on `expect` after a separate validation pass.
- The generated audit scanner now handles Rust character literals, preventing
  false panic/fs/unwrap attribution when a function contains characters such as
  `'"'` inside char literals.
- Compute registry initialization now returns `Result` through the registry and
  backend support path. Missing SIMD registration keys bubble as
  `PilError::InternalError` instead of panicking through `expect`, while normal
  unsupported operations still surface as `PilError::ValueError`.
- Reviewed production non-`Result` fallibility signals have been split into
  explicit compatibility buckets: ABI status-code functions, Pillow draw
  clipping control flow, FreeType sentinel arithmetic, FreeType void-API
  internal status, documented invariant panics, and FreeType default-value
  control flow. Current production `review_panic_path` and
  `review_non_result_fallible` counts are both `0`; remaining production
  manual work is in `parser_review`.
- The generated audit scanner now strips comments and string/character
  literals before fallibility signal detection. This prevents documentation
  text, error messages, and character constants from being counted as executable
  `?`, `panic!`, `expect`, or status-construction paths.
- Core `pillow-rs/src` production parser-review rows are now classified:
  Pillow draw iterator exhaustion, imagingft bitmap-coverage absence, and
  palette/transparency absence are tracked as reviewed `Option` control flow.
- `fontdone::Font::getlength` and its private `layout_advance` helper now
  return `Result<_, FontError>` and bubble glyph metric lookup failures instead
  of silently keeping the accumulated advance. The unified parity test and
  benchmark example were updated to handle the fallible API explicitly.
- Remaining production parser-review rows have been classified into explicit
  FreeType compatibility buckets: optional ABI snapshots/handles, FFI optional
  table lookups, optional font-feature parsers, optional SFNT lookups, metrics
  lookup absence, empty-outline geometry absence, raster clipping control flow,
  and binding display fallback. Current production `parser_review`,
  `review_panic_path`, and `review_non_result_fallible` counts are all `0`.
- Remaining non-production review rows have been classified as
  test/example/bench harness behavior. The generated inventory currently has no
  rows left in `parser_review`, `review_non_result_fallible`, or
  `review_panic_path` across any scope.
- JS Mandelbrot quality conversion, PyDraw argument length reporting, CPU
  transform mesh reconstruction, CPU invert reconstruction, and SIMD grayscale
  reconstruction now bubble structured errors instead of using `expect` inside
  production `Result`-returning functions.

## Next review queue

There are currently no rows in any scope classified as `parser_review`,
`review_non_result_fallible`, or `review_panic_path`.

Before marking the project goal complete, run a completion audit against the
current source tree:

1. verify that production `unwrap`, `expect`, `panic!`, `todo!`,
   `unimplemented!`, `Command::`, filesystem I/O, checked arithmetic, `?`,
   `ok_or`, `map_err`, and `Err` signals are either `Result`-returning or
   assigned to a documented compatibility bucket;
2. inspect remaining non-production rows and either classify them as
   test/example/bench setup behavior or convert any real library-style helper
   errors to `Result`;
3. run maintained crate tests/checks after any final changes.
