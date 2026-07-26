# Font Public-API Parity Status

Last updated: 2026-07-27 (Asia/Kolkata) after maintained stroker CubicTo
first-segment parity work.

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
contours, opened two-point horizontal lines, and a closed horizontal line after
degenerate contour skipping. It still returns `FT_Err_Unimplemented_Feature` for
normal line/conic/cubic glyph outlines. Visible stroked glyph masks require
completing real FreeType-compatible stroker geometry/export and then wiring that
route into `pillow-rs/src/font/imagingft.rs`.

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

Result after this update: `FT_Stroker_ParseOutline.opened_outline_success` and
`FT_Stroker_ParseOutline.degenerate_contours_skipped` now run as real
C/Rust/WASM parity. ParseOutline runtime movement is `runnable=5`, `passed=5`,
`pending=1`. The remaining ParseOutline pending row is the mixed
line/conic/cubic route.

### Remaining stroker first-divergence target

The remaining `FT_Stroker_ParseOutline.line_conic_cubic_success` row is not a
fixture coverage gap. The fixture explicitly requires five same-input outline
rows: a line contour, implied conic start, implied conic midpoints, cubic
contour, and a real loaded glyph outline. Pinned C FreeType handles those rows
through `freetype/src/base/ftstroke.c:2048-2242`, which decomposes each contour
and dispatches into:

- `FT_Stroker_LineTo` at `freetype/src/base/ftstroke.c:1271-1345`
- `FT_Stroker_ConicTo` at `freetype/src/base/ftstroke.c:1351-1559`
- `FT_Stroker_CubicTo` at `freetype/src/base/ftstroke.c:1565-1757`
- `FT_Stroker_EndSubPath` at `freetype/src/base/ftstroke.c:1874-1933`

The current Rust implementation in `pillow-rs-freetype/src/ffi/handles.rs`
matches only:

- zero-length line/conic/cubic no-ops,
- open horizontal line cap export,
- closed horizontal line export after degenerate contour skipping,
- count-only closed two-line corner behavior.

The first real implementation unit needed for Font `stroke_width` coverage is
not another Font input row. It is a pure-Rust port of the border state machine
used by C `ft_stroker_subpath_start`, `ft_stroker_process_corner`,
`ft_stroke_border_lineto`, `ft_stroke_border_conicto`, and
`ft_stroke_border_cubicto`, with exact export order and tags. Only after that
can `FT_Glyph_Stroke`/`FT_Glyph_StrokeBorder` produce Pillow-compatible visible
glyph masks.

Latest `FT_Stroker_LineTo` lower-level movement:

```bash
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_LineTo
```

Result after first-segment routing: `line_segment_success` and
`first_segment_starts_subpath` now run as real C/Rust/WASM parity. The
first-segment route explicitly mirrors C FreeType's required
`FT_Stroker_BeginSubPath` → `FT_Stroker_LineTo` → `FT_Stroker_EndSubPath` →
`FT_Stroker_GetCounts` → `FT_Stroker_Export` sequence; export without the
counts preflight is not a valid oracle comparison. LineTo runtime movement is
`runnable=5`, `passed=5`, `pending=0`. Route audit movement is
`real-parity=4830`, `pending-route=191`.

Latest `FT_Stroker_EndSubPath` lower-level movement:

```bash
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_EndSubPath
```

Result after closed-path routing:
`FT_Stroker_EndSubPath.no_segment_after_begin` now runs as real C/Rust/WASM
parity through the maintained ParseOutline degenerate route. The route emits
only the fixture-declared `single_point_contour` and `empty_outline` rows and
compares `parse_status` plus zero `counts_after` against the pinned C oracle.
Direct EndSubPath no-segment remains status-only because the pinned C build
segfaults if counts are queried after that direct unfinished state.
`FT_Stroker_EndSubPath.open_subpath_emits_caps_and_single_border` also now runs
as real C/Rust/WASM parity for radius-128 butt, round, and square caps using
exact exported outline points, tags, contours, empty right border, and combined
export. `FT_Stroker_EndSubPath.closed_subpath_closes_two_borders` now runs as
real C/Rust/WASM parity for the maintained two-line closed path, including exact
left and right exported border outline points, tags, and contours. EndSubPath
runtime movement is `runnable=5`, `passed=5`, `pending=0`. Route audit movement
is `real-parity=4833`, `pending-route=188`.

Latest `FT_Stroker_ConicTo` lower-level movement:

```bash
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_ConicTo
```

Result after the maintained open first-segment route:
`FT_Stroker_ConicTo.conic_curve_success` now runs as real C/Rust/WASM parity
for the explicit `(0,0) -> (256,512) -> (512,0)` closed fixture. The route
compares exact status sequence, exported outline points, tags, contours, and
CBox against a pinned C oracle.
`FT_Stroker_ConicTo.first_segment_starts_subpath` also now runs as real
C/Rust/WASM parity for the explicit open `(0,0) -> (256,512) -> (512,0)`
fixture, proving first-segment border initialization and open end-cap export
for that maintained route. These routes are not the general conic subdivision
implementation. ConicTo runtime movement is `runnable=4`, `passed=4`,
`pending=0`. Route audit movement is `real-parity=4835`,
`pending-route=186`.

Latest `FT_Stroker_CubicTo` lower-level movement:

```bash
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_CubicTo
```

Result after the maintained cubic routes:
`FT_Stroker_CubicTo.cubic_curve_success` now runs as real C/Rust/WASM parity
for the explicit `(0,0) -> (160,640) -> (480,640) -> (640,0)` closed fixture.
The route compares exact status sequence, exported outline points, tags,
contours, and CBox against a pinned C oracle.
`FT_Stroker_CubicTo.first_segment_starts_subpath` also now runs as real
C/Rust/WASM parity for the explicit open
`(0,0) -> (160,640) -> (480,640) -> (640,0)` fixture, proving first-segment
border initialization and open end-cap export for that maintained route. These
routes are not the general cubic subdivision implementation. CubicTo runtime
movement is `runnable=4`, `passed=4`, `pending=0`. Route audit movement is
`real-parity=4837`, `pending-route=184`.

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

- Run: `a81f8dae-70b3-46bb-aa44-31238b6c1bfd`
- Snapshot: `078b5f99-68ab-469a-9374-911a1b1b7e8c`
- Status: passed
- Coverage artifact: ingested
- Commit measured: `782050e4dc934a7981b928a34d24b2feff0d22cd`

Target file metrics:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `684/697` (`98.13%`) | `116/120` (`96.67%`) | `76/81` (`93.83%`) | `1072/1108` (`96.75%`) |
| `pillow-rs/src/font/mod.rs` | `191/191` (`100.00%`) | n/a | `41/41` (`100.00%`) | `252/253` (`99.60%`) |

Latest Font wrapper movement:

- Added `font.text_bbox.invalid_maxp_too_many_instruction_defs` as an
  input-only row. The expected error is generated at runtime by the pinned
  Pillow oracle.
- This covers `Font::text_bbox`'s `getbbox?` error propagation region.
- The only remaining `font/mod.rs` uncovered region is
  `Font::load_default`'s `default_aileron::decode()?` error arm. That path is
  not reachable through honest public inputs unless the checked-in embedded
  default font bytes are corrupt.

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

Latest blocker verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke
```

Result after the CubicTo first-segment movement: runnable rows still pass (`3/3`), and the
five glyph-stroke success rows remain pending. This confirms that adding active
Font `stroke_width` rows now would create honest Pillow-vs-Rust failures rather
than increasing trustworthy Font coverage.

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
