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
  - `ok_result`: `2,575`
  - `likely_infallible`: `3,110`
  - `parser_review`: `262`
  - `review_non_result_fallible`: `98`
  - `review_panic_path`: `74`

## Current interpretation

The generated `classification` column is conservative:

- `ok_result`: function signature already returns `Result`.
- `likely_infallible`: no obvious fallibility signal was detected.
- `review_non_result_fallible`: non-`Result` function contains an obvious fallibility signal such as `unwrap`, `expect`, filesystem/process use, or checked arithmetic.
- `review_panic_path`: non-`Result` function contains `panic!`, `todo!`, or `unimplemented!`.
- `parser_review`: mechanical parser found a mixed signal that requires manual review.

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

## Next review queue

Start with public, non-test rows classified as `review_non_result_fallible` or
`review_panic_path`, then private helpers in the same module so errors bubble
without boundary translation loss.

Highest-priority current production rows are the remaining `scope=production`
rows in `docs/generated/rust-method-result-audit.tsv` classified as
`review_non_result_fallible` or `review_panic_path`.
