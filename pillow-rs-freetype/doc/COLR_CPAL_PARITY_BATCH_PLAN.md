# COLR/CPAL parity batch plan

Status: active parity work plan.

Objective: move `ftcolor` rows only when the same font input compares exactly
through pinned C FreeType, Rust FFI, thin C ABI, and WASM ABI. Constants,
missing-font fallbacks, and color-disabled alternate-build assumptions do not
count as runtime color parity.

Baseline at start of this plan

- Branch: `main`
- Baseline commit: `54b1bd499`
- Route audit: `pending-route=335`, `real-parity=4628`
- Focused `ftcolor.get_color_glyph_layer`: `passed=2`, `pending=3`

Implemented batch: COLR v0 layer iterator

- Added a generated compact fixture:
  - `tests/fixtures/fonts/color/colr-v0-layers-cpal.ttf`
  - Rebuilt with `make -C pillow-rs-freetype font-fixture-color`
- Added a pure-Rust COLR v0 table parser for base glyph records and layer
  records.
- Added `FT_Get_Color_Glyph_Layer` through core Rust FFI, C ABI, and WASM ABI.
- Added a maintained C oracle route for:
  - `ftcolor.FT_Get_Color_Glyph_Layer.layer_iteration_success`
  - `ftcolor.FT_Get_Color_Glyph_Layer.foreground_color_index`
  - `ftcolor.FT_Get_Color_Glyph_Layer.terminal_false_preserves_last_outputs`

Current focused status after implementation

- `make -C pillow-rs-freetype test-op OP=ftcolor.get_color_glyph_layer`
- Result: `passed=5`, `pending=0`

Remaining color batches

1. COLR v1 paint graph traversal:
   - `ftcolor.get_paint_graph`
   - `ftcolor.traverse_paint_graph`
   - `ftcolor.get_paint`
   - `ftcolor.get_color_glyph_paint_and_get_paint`
   - `ftcolor.get_gradient_paint_and_stops`
   - `ftcolor.get_colorline_stops`
2. COLR v1 clipbox and foreground paint:
   - `ftcolor.get_color_glyph_clipbox`
   - `ftcolor.palette_set_foreground_color`
3. Palette disabled-build rows:
   - require an explicit disabled-color-layer oracle build or must remain
     pending.
4. Malformed COLR fixtures:
   - keep malformed-row parity scoped to exact malformed inputs and output
     preservation; do not use success fixtures as substitutes.

Verification gates for any color commit

- Focused operation target(s), for example:
  - `make -C pillow-rs-freetype test-op OP=ftcolor.get_color_glyph_layer`
  - `make -C pillow-rs-freetype test-op OP=ftcolor.get_paint_graph`
  - `make -C pillow-rs-freetype test-op OP=ftcolor.get_gradient_paint_and_stops`
- `make fontdone-ffi-compat`
- `make fontdone-lint`
- `make fontdone-parity`

Notes

- FreeType references for this batch are `src/sfnt/ttcolr.c`,
  `src/sfnt/ttcpal.c`, and `src/base/ftcolor.c`.
- The COLR v0 layer route intentionally does not claim COLR v1 paint graph,
  gradient, transform, composite, or clipbox parity.
