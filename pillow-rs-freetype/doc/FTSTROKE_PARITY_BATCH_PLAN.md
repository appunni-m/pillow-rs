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
  - `FT_Stroker_LineTo.zero_length_line_noop`
  - `FT_Stroker_ConicTo.coincident_control_and_end_noop`
  - `FT_Stroker_CubicTo.coincident_controls_and_end_noop`
- Must stay pending until real geometry exists:
  - `FT_Stroker_Set` attribute, miter-limit, and path-clearing rows.
  - `FT_Stroker_Rewind` attribute/path-clearing rows.
  - Remaining `FT_Stroker_BeginSubPath`, `LineTo`, `ConicTo`, `CubicTo`, and
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
- 2026-07-22: `FT_Stroker_LineTo.zero_length_line_noop` is the first
  non-null path-state foothold.  The maintained route calls
  `FT_Stroker_New`, `FT_Stroker_Set`, `FT_Stroker_BeginSubPath`, a
  zero-length `FT_Stroker_LineTo`, and `FT_Stroker_GetCounts` through pinned C,
  Rust FFI, thin C ABI, and WASM ABI.  C reference behavior:
  `src/base/ftstroke.c:1765-1795` records the subpath start/current point, and
  `src/base/ftstroke.c:1279-1284` returns `FT_Err_Ok` before changing center
  or emitting border geometry when the line delta is zero.  Public observable
  output is `status=0`, `points=0`, and `contours=0`.
  Route audit moved `real-parity=4802 -> 4803` and
  `pending-route=217 -> 216`; full runtime parity moved
  `7072/7072 -> 7073/7073`.
- 2026-07-22: `FT_Stroker_ConicTo.coincident_control_and_end_noop` and
  `FT_Stroker_CubicTo.coincident_controls_and_end_noop` are the next
  degenerate path-state routes.  The maintained route calls
  `FT_Stroker_New`, `FT_Stroker_Set`, `FT_Stroker_BeginSubPath`, the
  coincident curve command, and `FT_Stroker_GetCounts` through pinned C, Rust
  FFI, thin C ABI, and WASM ABI.  C reference behavior:
  `src/base/ftstroke.c:69-71` defines `FT_EPSILON=2` with strict
  `FT_IS_SMALL` bounds; `src/base/ftstroke.c:1361-1373` and
  `src/base/ftstroke.c:1566-1581` update only the current center and return OK
  before curve subdivision or border emission when all control/end deltas are
  small.  Public observable output is `status=0`, `points=0`, and
  `contours=0`; full conic/cubic geometry rows remain pending.
  Route audit moved `real-parity=4803 -> 4805` and
  `pending-route=216 -> 214`; full runtime parity moved
  `7073/7073 -> 7075/7075`.
- `FT_Stroker.unparsed_handle_lifecycle_matches_c` is intentionally narrower
  than `FT_Stroker.lifecycle_contract`: it proves constructor, setter, unparsed
  export no-op, rewind, and destruction behavior through the same C/Rust/C
  ABI/WASM harness, but it does not claim path commands, counts, or geometry.
