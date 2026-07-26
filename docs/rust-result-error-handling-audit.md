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
- Current generated rows: `6,902`
- Current generated classification counts:
  - `ok_result`: `2,547`
  - `likely_infallible`: `3,825`
  - `parser_review`: `309`
  - `review_non_result_fallible`: `134`
  - `review_panic_path`: `87`

## Current interpretation

The generated `classification` column is conservative:

- `ok_result`: function signature already returns `Result`.
- `likely_infallible`: no obvious fallibility signal was detected.
- `review_non_result_fallible`: non-`Result` function contains an obvious fallibility signal such as `unwrap`, `expect`, filesystem/process use, or checked arithmetic.
- `review_panic_path`: non-`Result` function contains `panic!`, `todo!`, or `unimplemented!`.
- `parser_review`: mechanical parser found a mixed signal that requires manual review.

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

## Next review queue

Start with public, non-test rows classified as `review_non_result_fallible` or
`review_panic_path`, then private helpers in the same module so errors bubble
without boundary translation loss.

Highest-priority current production rows include:

- `pillow-rs/src/color.rs`: grayscale conversion helpers use `expect` on dimension/buffer invariants.
- `pillow-rs/src/compute/mod.rs`: backend routing state uses mutex `expect` and returns bare values.
- `pillow-rs/src/compute/op_def.rs`: duplicate op registration panics.
- `pillow-rs/src/image_utils.rs`: `raw_bytes_to_image_trusted` uses `expect`.
- `pillow-rs-image/src/types/buffer.rs`: indexed pixel accessors panic on out-of-bounds access.
