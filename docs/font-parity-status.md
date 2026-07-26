# Font Public-API Parity Status

Last updated: 2026-07-27 (Asia/Kolkata) after opened-outline stroker
ParseOutline parity work.

## Oracle and fixture contract

- Active Font parity fixtures live under
  `pillow-rs/tests/fixtures/font/inputs/public-api`.
- The legacy imagingft corpus is not active; remaining imagingft files are under
  `deprecated/imagingft/`.
- Input JSON files are input-only. They must not contain expected output,
  hashes, status, or expected error payloads.
- Expected results are generated at test runtime by
  `pillow-rs/scripts/font_oracle.py` using the repo-local
  `.oracle-venv/bin/python`.
- The oracle asserts that `PIL.ImageFont.core` is `_imagingft` and that
  `PIL._imagingft` is a native extension before producing results.
- Rust test results are compared against Pillow through `Result`-style
  status/value/error payloads. Success payloads include exact bytes; error
  payloads include kind and message.

## Pillow Font public surface comparison

The pinned Pillow `11.3.0` oracle reports these public
`ImageFont.FreeTypeFont` methods:

- `font_variant`
- `get_variation_axes`
- `get_variation_names`
- `getbbox`
- `getlength`
- `getmask`
- `getmask2`
- `getmetrics`
- `getname`
- `set_variation_by_axes`
- `set_variation_by_name`

`font_manifest.yaml` currently classifies every live Pillow public signature
parameter for those methods. The only blocked public parameters are the
documented stroke rendering parameters below.

The repo also keeps additional public test operations around the Font surface:
`load_default`, `truetype`, `font_size`, transposed font behavior,
`draw_text`, `render_text_binary`, `text_bbox`, and explicit load/layout
negative rows. These are not extra Pillow `FreeTypeFont` methods; they are
parity checks for repo public APIs that consume the same Font path.

Current blocked public parameters:

- `getmask.stroke_width`
- `getmask2.stroke_width`

## Missing implementation

`stroke_width != 0` for `getmask`/`getmask2` is the remaining real Font public
parity implementation gap.

Pillow `11.3.0` passes stroke rendering from Python into `_imagingft.c`, where
the native render path creates an `FT_Stroker`, obtains a glyph with
`FT_Get_Glyph`, applies `FT_Glyph_Stroke` or `FT_Glyph_StrokeBorder`, converts
the stroked glyph through `FT_Glyph_To_Bitmap`, and then copies that bitmap into
the mask.

The Rust Font adapter cannot honestly close this gap alone. The dependency is
the pure-Rust FreeType stroker implementation in
`pillow-rs-freetype/src/ffi/handles.rs`. Its current
`FT_Stroker_ParseOutline` route is intentionally limited to empty/single-point
contours and returns `FT_Err_Unimplemented_Feature` for normal multi-point glyph
outlines. Visible stroked glyph masks require completing real
FreeType-compatible stroker geometry/export and then wiring that route into
`pillow-rs/src/font/imagingft.rs`.

The Font test pins this blocker to lower-level FreeType success rows:

- `ftstroke.FT_Glyph_Stroke.outline_glyph_stroked_success`
- `ftstroke.FT_Glyph_Stroke.destroy_original_option`
- `ftstroke.FT_Glyph_StrokeBorder.outside_border_success`
- `ftstroke.FT_Glyph_StrokeBorder.inside_border_success`
- `ftstroke.FT_Glyph_StrokeBorder.destroy_original_option`

Latest narrow lower-level check:

```bash
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke
```

Result: current runnable rows pass (`3/3`), and the five glyph-stroke success
rows above remain pending lower-level routes.

Additional lower-level blocker reduction:

```bash
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_ParseOutline
```

Result after this update: `FT_Stroker_ParseOutline.opened_outline_success`
now runs as real C/Rust/WASM parity. ParseOutline runtime movement is
`runnable=4`, `passed=4`, `pending=2`. Remaining ParseOutline pending rows are
the mixed line/conic/cubic route and the broader degenerate-contour route.

## Edge cases already covered by active Font fixtures

- Constructor/load paths: `load_default`, `truetype`, missing asset, invalid
  font, invalid size.
- Text input variants: `str`, Python `bytes`, empty text, space-only text,
  ASCII kerning pairs, descenders, CFF font, embedded strike/sbit fixtures.
- Layout options: default mode, `"1"` binary mode, `"RGBA"` error mode, bad
  mode, direction/features/language no-raqm errors.
- Anchor options: left/top/ascender, middle/middle, right/descender, bad anchor.
- Mask/render options: start offsets, ink, ignored `getmask2` args/kwargs,
  binary rendering, `draw_text`.
- Variation options: axes/names, name bytes, repeated name, invalid name,
  non-variable errors, large and out-of-range coordinate values, malformed fvar
  table rows copied from maintained FreeType fixtures.
- Transposed font behavior: all supported transpose orientations plus invalid
  orientation/type errors and length rejection for 90°/270° rotation.

## Coverage MCP status

Managed command: `font-tests-coverage-with-freetype`

- Run: `697e12a8-f26c-44df-8a16-c28ec46480d6`
- Snapshot: `63a81df6-2889-4753-85e5-6a8e8f039a09`
- Status: passed
- Coverage artifact: ingested
- Commit measured: `202d437bf4efd5634b026008fa2709cbaaaa268d`

Target file metrics:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `684/697` (`98.13%`) | `116/120` (`96.67%`) | `76/81` (`93.83%`) | `1072/1108` (`96.75%`) |
| `pillow-rs/src/font/mod.rs` | `191/191` (`100.00%`) | n/a | `41/41` (`100.00%`) | `251/253` (`99.21%`) |

Remaining targeted gaps in `imagingft.rs`:

- `90-91`: generic unknown FreeType error fallback. No public Font fixture has
  been found that reaches this via the Pillow-compatible surface without
  manufacturing invalid internal state.
- `241,246`: `FT_Set_Named_Instance` error after a valid public name match.
  Needs a real font accepted through name discovery but rejected by the
  lower-level named-instance setter.
- `263,267`: `FT_Set_Var_Design_Coordinates` error after variation-face
  validation. Needs a real variable font accepted by axis discovery but rejected
  only after applying public coordinates.
- `366-367`: non-zero `stroke_width`; blocked on real pure-Rust
  `FT_Glyph_Stroke`/`FT_Glyph_StrokeBorder` implementation.

## Required next implementation sequence

1. Complete pure-Rust FreeType stroker geometry/export for real glyph outlines
   in `pillow-rs-freetype`.
2. Make the lower-level `FT_Glyph_Stroke` and `FT_Glyph_StrokeBorder` success
   fixture rows runnable and exact.
3. Wire stroked glyph rendering into `pillow-rs/src/font/imagingft.rs`.
4. Move `getmask.stroke_width` and `getmask2.stroke_width` from `blocked` to
   `covered` in `font_manifest.yaml`.
5. Add minimal active input-only Font rows for visible glyph strokes.
6. Rerun `make -C pillow-rs font-tests` and Coverage MCP
   `font-tests-coverage-with-freetype`.

Do not add fake empty/space-only stroke rows to cover the branch. Pillow
succeeds for visible glyph strokes, so the only trustworthy path to 100% region
coverage is the lower-level stroker implementation.
