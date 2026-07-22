# FTStroker parity batch plan

Status: active parity work plan.

Objective: move `ftstroke` rows only when Rust FFI, thin C ABI, and WASM ABI
produce the same public output as pinned C FreeType for the same input. Do not
promote geometry rows through Rust-only state inspection or local placeholders.

Baseline at start of plan

- Branch: `main`
- Baseline commit: `16ba2b798`
- Route audit: `pending-route=334`, `real-parity=4629`
- `ftstroke` pending-route rows: 56

Current classification

- Already-routed exact no-op/lifecycle rows:
  - `FT_Stroker_Set.null_stroker_noop`
  - `FT_Stroker_Rewind.null_stroker_noop`
  - `FT_Stroker_Done.null_stroker_noop`
  - `FT_Stroker_New.valid_library_allocates_stroker`
  - `FT_Stroker_Done.valid_stroker_releases_buffers`
  - `FT_Stroker.unparsed_handle_lifecycle_matches_c`
  - `FT_Stroker_Export.invalid_inputs_noop`
  - `FT_Stroker_ExportBorder.invalid_inputs_or_border_noop`
- Must stay pending until real geometry exists:
  - `FT_Stroker_Set` attribute, miter-limit, and path-clearing rows.
  - `FT_Stroker_Rewind` attribute/path-clearing rows.
  - `FT_Stroker_BeginSubPath`, `LineTo`, `ConicTo`, `CubicTo`, and
    `EndSubPath` success/state rows.
  - `FT_Stroker_GetCounts`, `GetBorderCounts`, `Export`, and `ExportBorder`
    geometry rows.
  - `FT_Glyph_Stroke` and `FT_Glyph_StrokeBorder` glyph-object rows.

Batch order

1. Keep the current no-op/lifecycle routes as the guardrail; do not expand their
   claimed parity beyond what the harness compares.
2. Implement real stroker path construction in pure Rust:
   - state: radius, line cap, line join, miter limit, open/closed subpath,
     current point, left/right border buffers;
   - operations: begin, line, conic, cubic, end, rewind;
   - output: border counts and full outline export.
3. For each operation, add/extend oracle route output so C/Rust/C ABI/WASM
   compare exact errors, counts, points, tags, contours, and output-preservation
   behavior.
4. Promote route-audit rows only after focused `make -C pillow-rs-freetype
   test-op OP=...` shows runtime rows passing across all three ABI lanes.

Verification gates for any ftstroke commit

- Focused changed operation target(s), for example:
  - `make -C pillow-rs-freetype test-op OP=ftstroke.line_to`
  - `make -C pillow-rs-freetype test-op OP=ftstroke.get_counts`
  - `make -C pillow-rs-freetype test-op OP=ftstroke.export_border`
- `make -C pillow-rs-freetype test-harness`
- `make fontdone-ffi`
- `make fontdone-ffi-compat`
- `make fontdone-lint`

Notes

- FreeType reference is `freetype/src/base/ftstroke.c`.
- A row that only observes "no crash" is not geometry parity.
- A row that only observes Rust-private `StrokerState` is not C parity.
- `FT_Stroker.unparsed_handle_lifecycle_matches_c` is intentionally narrower
  than `FT_Stroker.lifecycle_contract`: it proves constructor, setter, unparsed
  export no-op, rewind, and destruction behavior through the same C/Rust/C
  ABI/WASM harness, but it does not claim path commands, counts, or geometry.
