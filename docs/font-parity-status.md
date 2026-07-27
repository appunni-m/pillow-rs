# PIL.ImageFont Public-API Parity Status

Last updated: 2026-07-27 (Asia/Kolkata) after enforcing that the target is the
full `PIL.ImageFont` module surface. `libraqm` shaping is the only explicit
out-of-scope area; `ImageFont.FreeTypeFont`/`_imagingft` remains in scope
because Pillow exposes it through `PIL.ImageFont`.

## Oracle and fixture contract

- Active `PIL.ImageFont` parity fixtures live under
  `pillow-rs/tests/fixtures/font/inputs/public-api`.
- The legacy imagingft corpus is not active; remaining imagingft files are under
  `deprecated/imagingft/`.
- Input JSON files are input-only. They must not contain expected output,
  hashes, status, or expected error payloads.
- Expected results are generated at test runtime by
  `pillow-rs/scripts/font_oracle.py` using the repo-local
  `.oracle-venv/bin/python`.
- The oracle target is the public `PIL.ImageFont` module. For
  `ImageFont.FreeTypeFont` rows, the oracle additionally asserts that
  `PIL.ImageFont.core` is `_imagingft` and that `PIL._imagingft` is a native
  extension before producing results. Bitmap `ImageFont.ImageFont` rows stay on
  Pillow's Python/PILfont path.
- Rust test results are compared against Pillow through `Result`-style
  status/value/error payloads. Success payloads include exact bytes; error
  payloads include kind and message.
- `pillow-rs/tests/font_public_api.rs` now fails if the manifest oracle section
  drifts away from `expected_path: PIL.ImageFont`, `rust_runtime:
  pillow_rs::ImageFont`, or the rule that `_imagingft` is only an
  implementation assertion for FreeTypeFont-backed rows.
- The public-surface verifier also reads every live non-underscore
  `PIL.ImageFont` module name. Behavioral classes/functions/enums must be
  explicitly classified, and public imports/constants/types such as
  `MAX_STRING_LENGTH`, `Axis`, and `DeferredError` must stay explicitly marked
  as non-endpoint names. This prevents silently ignoring new public module
  names when checking manifest completeness.

## PIL.ImageFont public surface comparison

The pinned Pillow `11.3.0` oracle reports these public `PIL.ImageFont`
module/class surfaces in scope:

- module functions: `load`, `load_path`, `load_default_imagefont`,
  `load_default`, `truetype`
- `ImageFont.ImageFont`: `getbbox`, `getlength`, `getmask`
- `ImageFont.TransposedFont`: `getbbox`, `getlength`, `getmask`
- `ImageFont.FreeTypeFont`:
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

`font_manifest.yaml` now classifies the full active `PIL.ImageFont` surface,
not only `FreeTypeFont`. Bitmap `ImageFont.ImageFont` rows execute through
`pillow-rs/src/font/pilfont.rs`; FreeType rows execute through the
`pillow_rs::ImageFont` handle and `_imagingft`-compatible path. The active test
still compares every row against a live Pillow oracle at runtime; input JSON
files contain only inputs.

The target is not `_imagingft` and not `pillow-rs-freetype` by itself. Those are
implementation routes used to reproduce the public `PIL.ImageFont` behavior.
Coverage or parity at the lower layer is only trusted when it is connected back
to an active `PIL.ImageFont` fixture or a documented ImageFont blocker.

`libraqm` is the only explicit out-of-scope public behavior. Inputs using
`direction`, `features`, or `language` remain in scope as error-parity rows:
they must match Pillow's no-libraqm `KeyError`/message behavior rather than
being skipped.

The active test now enforces `font_manifest.yaml.required_operations` as the
exact union of live `PIL.ImageFont` public operations reported by the pinned
oracle and the explicit repo helper/consumer operations maintained around that
surface. This prevents hidden manifest drift in either direction.
It also enforces the manifest `out_of_scope` list exactly: the only permitted
public-surface exclusion is successful libraqm shaping; `direction`,
`features`, and `language` no-libraqm error rows remain active parity rows.
The lower FreeType stroker group is not out of scope for ImageFont anymore:
the Rust/C-ABI routes exist and are classified as partial until the maintained
and pending `FT_Stroker_*`, `FT_Glyph_Stroke`, and
`FT_Glyph_StrokeBorder` rows are exact.
For covered public parameters, the verifier now requires 76 concrete values
from the active corpus. This includes no-libraqm values
(`direction="rtl"`, `language="en"`, `features=[]`, and
`features=["-kern"]` where accepted) plus the currently exercised
`stroke_width`, `anchor`, `start`, `ink`, `args`, `kwargs`, and
`font_variant(layout_engine)` values. It also now requires active
`truetype(index=0)`, `truetype(encoding="")`, and
`truetype(layout_engine=RAQM)` rows.
Bitmap `ImageFont` and `TransposedFont` variadic `*args`/`**kwargs` parameters
are also covered by input-only rows that pass those extras into the live Pillow
oracle at runtime and compare the ignored-result behavior exactly.
The test also queries the live `PIL.ImageFont.Layout` enum and requires exactly
`BASIC` and `RAQM`; active `font_variant` rows must exercise both values while
successful RAQM shaping remains the only layout behavior outside the target.

The repo also keeps additional public test operations around this surface:
`font_size`, `text_bbox`, `getbbox_binary`, `getmask2_with_start`,
`get_transposed_mask`, `has_variations`, `transposed_bbox`,
`validate_transposed_length`, `draw_text`, and `render_text_binary`. These are
repo public helpers/consumers that exercise the same `PIL.ImageFont` behavior.

Current blocked public parameters:

- None.

## Missing implementation

The remaining non-libraqm public parity gaps are:

- general `stroke_width != 0` for all visible glyph outlines. The active Font
  corpus now includes the maintained lower-level DejaVuSans `"A"` route at
  `stroke_width=1.5`; broader stroked glyph coverage still depends on the
  general pure-Rust FreeType stroker.
- stroked visible glyphs with public `mode="1"`. A temporary input-only sweep
  for DejaVuSans `"A"` at 24px, `stroke_width=1.5`, and `mode="1"` proved this
  is a distinct Pillow public behavior: Pillow still returns an `L` mask with
  antialiased stroked coverage, not the binary mask Rust currently produces
  through the mono stroked render path. Forcing normal stroked rendering in the
  adapter still matched the existing `mode="L"` path rather than Pillow's
  mono-targeted stroked outline, so the real fix belongs in the lower-level
  mono-targeted glyph load/stroker interaction before this row can become an
  active passing fixture.

Pillow `11.3.0` passes stroke rendering from Python into `_imagingft.c`, where
the native render path creates an `FT_Stroker`, obtains a glyph with
`FT_Get_Glyph`, applies `FT_Glyph_Stroke` or `FT_Glyph_StrokeBorder`, converts
the stroked glyph through `FT_Glyph_To_Bitmap`, and then copies that bitmap into
the mask.

The Rust Font adapter can only close stroked glyph routes that the lower-level
pure-Rust FreeType stroker already supports. The dependency is the pure-Rust
FreeType stroker implementation in
`pillow-rs-freetype/src/ffi/handles.rs`. Its current
`FT_Stroker_ParseOutline` route is intentionally limited to empty/single-point
contours, opened two-point horizontal lines, and a closed horizontal line after
degenerate contour skipping. It still returns `FT_Err_Unimplemented_Feature` for
most normal line/conic/cubic glyph outlines. Broad visible stroked glyph masks
require completing real FreeType-compatible stroker geometry/export and then
expanding the `pillow-rs/src/font/imagingft.rs` route beyond the maintained
DejaVuSans glyph fixture.

The Font test pins this blocker to the remaining lower-level FreeType success
rows:

- `ftstroke.FT_Glyph_Stroke.destroy_original_option`
- `ftstroke.FT_Glyph_StrokeBorder.outside_border_success`
- `ftstroke.FT_Glyph_StrokeBorder.inside_border_success`
- `ftstroke.FT_Glyph_StrokeBorder.destroy_original_option`

Latest narrow lower-level check:

```bash
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke
```

Result after this update:
`FT_Glyph_Stroke.outline_glyph_stroked_success` now runs as real
C/Rust/WASM parity for the explicit DejaVuSans glyph 36 fixture at 24px with
radius-96 round stroke. The route compares the replacement outline points,
tags, contours, CBox, status sequence, and preserve-original ownership against
the pinned C oracle. It is not the general glyph stroker implementation.
Current focused stroker rows pass. The broader `ftstroke.FT_Glyph_Stroke`
filter now compares 5 runnable rows, all exact, with three explicit pending
blockers: `FT_Glyph_Stroke.destroy_original_option`,
`FT_Glyph_StrokeBorder.destroy_original_option`, and lower stroked-bitmap
geometry. `FT_Glyph_StrokeBorder.inside_border_success` now has an explicit
maintained C-oracle/Rust/C-ABI/WASM-ABI route.
Forced pending-case runs no longer accept the shared generic fallback for these
rows. A pending stroke row must have an explicit maintained runtime route before
it can be promoted to C/Rust/WASM parity evidence.
The inside-border row now points at the existing
`input/fonts/cff/fontinfo-populated.otf` CFF fixture instead of an unresolved
future PostScript-orientation asset; the remaining blocker is runtime route and
geometry parity, not fixture availability.

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
  ASCII kerning pairs, descenders, over-`MAX_STRING_LENGTH` public errors,
  CFF font, embedded strike/sbit fixtures.
- Layout options: default mode, `"1"` binary mode, `"RGBA"` error mode, bad
  mode, direction/features/language no-raqm errors.
- Anchor options: left/top/ascender, middle/middle, right/descender, bad anchor.
- Mask/render options: start offsets, ink, ignored `getmask2` args/kwargs,
  binary rendering, `draw_text`, and the maintained visible DejaVuSans `"A"`
  stroke route at `stroke_width=1.5`.
- Variation options: axes/names, name bytes, repeated name, invalid name,
  non-variable errors, large and out-of-range coordinate values, malformed fvar
  table rows copied from maintained FreeType fixtures.
- Transposed font behavior: all supported transpose orientations plus invalid
  orientation/type errors and length rejection for 90°/270° rotation.

## Coverage MCP status

Managed command: `font-tests-coverage-with-freetype`

- Run: `150b677f-c229-4846-8c18-4705aa8d4bcd`
- Snapshot: `2a895073-ef7f-474a-ae79-f4fdc34c81b4`
- Status: passed
- Coverage artifact: ingested
- Commit measured: `665da57df87b79b316ea86cee8ccfb59c6a39392`

Target file metrics:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/default_aileron.rs` | `17/17` (`100.00%`) | n/a | `3/3` (`100.00%`) | `24/24` (`100.00%`) |
| `pillow-rs/src/font/imagingft.rs` | `1618/1643` (`98.48%`) | `268/278` (`96.40%`) | `159/171` (`92.98%`) | `2591/2687` (`96.43%`) |
| `pillow-rs/src/font/mod.rs` | `374/374` (`100.00%`) | n/a | `78/78` (`100.00%`) | `487/487` (`100.00%`) |
| `pillow-rs/src/font/pilfont.rs` | `715/737` (`97.01%`) | `142/142` (`100.00%`) | `58/78` (`74.36%`) | `1014/1094` (`92.69%`) |

Current full-module scope note:

- The active test now includes bitmap `PIL.ImageFont.ImageFont` rows and
  repo-local `load`/`load_path` PILfont assets, so `pilfont.rs` is now part of
  the coverage target.
- Coverage is not 100% yet for the full `PIL.ImageFont` target because
  `imagingft.rs` still has public-reachable FreeType/variation/stroke gaps.
  The shared bitmap compositor refactor removed duplicate normal/stroked paste
  branches, but it did not make unsupported stroke/variation routes complete.
- `pilfont.rs` branch coverage remains complete (`142/142`, `100.00%`). The
  current LLVM source map reports one uncovered relevant line on a doc-comment
  line for `from_pilfont_data`; no uncovered executable branch was reported.
  Additional byte-text and over-`MAX_STRING_LENGTH`
  `PIL.ImageFont.ImageFont.getlength` rows pass exact live Pillow parity, but
  they do not close the remaining LLVM function/region accounting.
- `default_aileron.rs` now embeds the decoded Aileron TTF bytes directly instead
  of validating a checked-in base64 payload at runtime. Corrupt repo data is not
  a public `PIL.ImageFont` input, so the user-facing `Result` boundary remains
  at `fontdone` font loading while the default-font embed path is fully covered.
- PILfont rows now include repo-local PNG, GIF/PBM discovery behavior, valid
  `P1` and `P4` PBM, CRLF P4 raster separator behavior, CRLF short-raster lazy
  loader semantics, L-mode glyph images, clipped PILfont metrics, public
  `ImageFont.info`, byte-text bitmap `getbbox`/`getmask`, malformed
  metrics/header cases, invalid glyph-image mode mapping, PBM tokenizer
  failures, truncated raster failures, and
  Pillow-matching `SystemError` render failures.

Latest Font wrapper movement:

- Added input-only `font.getmask2.hinter_code_overflow_stroked` and
  `font.getmask2.hinter_nested_defs_stroked` rows. Expected errors are
  generated at runtime by the live Pillow oracle. These rows exercise the
  public stroked `FreeTypeFont.getmask2` error path where glyph loading fails
  before the lower stroker runs, moving `imagingft.rs` region coverage from
  `2590/2687` to `2591/2687`.
- Added input-only `font.getlength.hinter_code_overflow` and
  `font.getlength.hinter_nested_defs` rows. Expected errors are generated at
  runtime by the live Pillow oracle. This independently exercises the public
  `FreeTypeFont.getlength` endpoint for the same rare hinter error classes
  already covered through bbox/mask endpoints and moved `imagingft.rs` region
  coverage from `2588/2687` to `2590/2687`.
- Added `font.text_bbox.invalid_maxp_too_many_instruction_defs` as an
  input-only row. The expected error is generated at runtime by the pinned
  Pillow oracle.
- This covers `Font::text_bbox`'s `getbbox?` error propagation region.
- Added active input-only visible stroke rows for
  `font.getmask.dejavusans24_a_stroke_1_5_l`,
  `font.getmask2.dejavusans24_a_stroke_1_5_l`, and public `start` success/error
  variants. Expected mask bytes and errors are generated only by the live
  Pillow oracle.
- Added active input-only empty stroked text rows for
  `font.getmask.dejavusans24_empty_stroke_1_5_l` and
  `font.getmask2.dejavusans24_empty_stroke_1_5_l`. These are public
  `PIL.ImageFont` inputs, not hardcoded expected outputs. The rows exposed and
  fixed a real mismatch: Pillow allocates `ceil(stroke_width * 2)` and reports
  `getmask2` offset `(-2, -2)` for `stroke_width=1.5`, while Rust previously
  allocated `ceil(stroke_width) * 2` and rounded the negative top offset toward
  zero.
- Added active input-only multi-glyph stroked rows for
  `font.getmask.dejavusans24_aa_stroke_1_5_l` and
  `font.getmask2.dejavusans24_aa_stroke_1_5_l`. They stay inside the maintained
  DejaVuSans `"A"` stroker route while exercising real multi-glyph stroked
  composition and the non-empty previous-glyph advance/kerning path.
- Added active input-only negative empty stroked text rows for
  `font.getmask.dejavusans24_empty_negative_stroke_l` and
  `font.getmask2.dejavusans24_empty_negative_stroke_l`. Pillow raises
  `ValueError("bad image size")` for these public inputs; Rust previously
  clamped the empty negative allocation to a zero-size success.
- Removed the unreachable Font-adapter `FT_Stroker_New` error branch from
  `imagingft.rs`. The lower-level FreeType-compatible function still owns null
  C-style argument validation; the safe `PIL.ImageFont` adapter always supplies
  both the library and output handle, so that branch was not a recoverable
  public Font error path.
- Centralized FreeType status handling for size requests and variation setters.
  This keeps Result-based propagation intact while avoiding duplicate
  status-check branches at public `PIL.ImageFont` call sites that all use the
  same success/error contract.
- Removed the redundant stroked zero-canvas early return. The stroked path now
  follows the same allocation result through the shared paste routine, which
  already no-ops for zero-sized output and returns the same `(width, height,
  bytes)` payload. This removed an unreachable public-Font branch without
  changing live Pillow oracle output.
- Wired `pillow-rs/src/font/imagingft.rs` through the existing pure-Rust
  lower-level `FT_Outline_Glyph_Stroke` route for the maintained DejaVuSans
  glyph fixture, then `FT_Outline_Glyph_To_Bitmap`, and reused Pillow's stroked
  bbox allocation rule. The visible `L`-mode rows pass exact byte comparison.
- `font.getmask`/`font.getmask2` `mode="1", stroke_width=1.5` probes for
  DejaVuSans `"A"` were rejected because they expose real lower-level
  mismatches. Pillow returns `L` payloads with size `19x21`; `getmask2` offset is
  `(-2, 4)`. The live Pillow oracle payload is grayscale coverage even though
  the public call uses `mode="1"`. Rust's current mono route keeps size/offset
  but returns thresholded bytes. A 2026-07-27 probe that forced the stroked glyph
  through normal grayscale rendering also kept size/offset but still produced
  different coverage bytes, so this remains a lower stroker/raster route issue
  rather than a trustworthy Font adapter row.
- `font.getmask2` stroked probes for DejaVuSans `"jQ"` and `"T"` were rejected.
  Pillow succeeds for both public `PIL.ImageFont` rows, while Rust currently
  returns an error. These rows would be valid Font coverage only after the
  lower-level pure-Rust stroker supports those glyph outlines with exact
  `FT_Glyph_Stroke`/bitmap parity.
- A `font.getmask2` `text="A\uFFFF", stroke_width=1.5` probe was also rejected.
  Pillow succeeds, while Rust currently returns an error when the stroked run
  crosses from a valid DejaVuSans glyph into the missing-glyph route. This would
  cover the stroked kerning guard's `g == 0` branch, but it cannot be kept as an
  active fixture until the lower-level missing-glyph stroke path matches Pillow.
Remaining targeted gaps in `imagingft.rs` from snapshot
`2273e018-4ff1-493a-96cd-9927148b3b26`:

- `91`, `105`: generic and rare mapped FreeType error branches. No public Font fixture has
  been found that reaches this via the Pillow-compatible surface without
  manufacturing invalid internal state. A sweep across the tracked Font assets
  and available FreeType fixture assets found only the already-mapped runtime
  errors: `code overflow`, `nested DEFS`, `too many instruction definitions`,
  and `too many function definitions`. The active corpus now includes these
  mapped errors through `getbbox`, `getmask`, and `getmask2` public rows where
  Pillow exposes them, but LLVM still reports the generic fallback line as
  uncovered because no live public row reaches an unmapped FreeType error.
- `253`: `set_variation_by_name` error propagation from
  `FT_Set_Named_Instance` after a valid public name match.
  Current tracked variable fonts accept all discovered named instances in the
  Pillow oracle; no deterministic public input has been found that matches a
  name and then fails only in the lower-level setter. A 2026-07-27 isolated
  subprocess sweep across 62 tracked Font/FreeType variable assets found no
  structured Pillow setter errors for valid names; the only candidates were
  Pillow segfaults on malformed variable-name fonts, which are not admissible
  active oracle fixtures.
- `271`: `set_variation_by_axes` error propagation from
  `FT_Set_Var_Design_Coordinates` after variation-face validation. Current
  tracked variable fonts accept empty, short, exact, overlong, and extreme
  finite coordinate arrays in the Pillow oracle. The 62-font isolated sweep
  likewise found no structured Pillow axes-setter errors; crash-only malformed
  rows remain excluded from the active corpus.
- `796`, `826-827`, `829`, `857`, `860`, `928`, and `959`: general visible non-zero
  `stroke_width`; partially routed through real pure-Rust `FT_Glyph_Stroke` for
  maintained DejaVuSans `"A"` single-glyph and multi-glyph rows plus the
  Pillow-compatible empty-text allocation path, with broader visible glyph
  coverage still blocked on complete `FT_Glyph_Stroke`/
  `FT_Glyph_StrokeBorder` implementation. The `mode="1"` stroked row is also
  blocked here: Pillow's C path uses mono-targeted glyph loading but converts
  the stroked outline to antialiased `L` coverage, while Rust's current mono
  stroked path renders binary coverage.
- stroked `.notdef` glyph and negative non-empty stroke rows. Pillow succeeds
  for inputs such as a missing Unicode scalar followed by `"A"` and for
  `text="A", stroke_width=-1.5`; Rust currently errors because the lower-level
  pure-Rust stroker only supports the maintained positive-radius DejaVuSans
  `"A"` route.

Latest blocker verification:

```bash
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke
make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_StrokeBorder
```

Result after the `FT_Glyph_Stroke.outline_glyph_stroked_success` movement:
`FT_Glyph_Stroke` filtered rows pass (`5/5`) with three route-pending blocker
rows still reported, and `FT_Glyph_StrokeBorder` runnable rows include the
maintained inside-border route with only destroy-option parity still pending.
The interface map now
classifies the lower stroker group as partial rather than out of scope;
successful stroke-border geometry remains pending rather than excluded.

The active Font corpus now promotes only the maintained DejaVuSans `"A"` stroke
route and the Pillow-compatible empty stroked-text behavior. Additional visible
stroke rows must wait for the remaining lower-level routes to become real
parity.

## Required next implementation sequence

1. Complete pure-Rust FreeType stroker geometry/export for real glyph outlines
   in `pillow-rs-freetype`.
2. Make the lower-level `FT_Glyph_Stroke` and `FT_Glyph_StrokeBorder` success
   fixture rows runnable and exact.
3. Expand stroked glyph rendering in `pillow-rs/src/font/imagingft.rs` as each
   lower-level stroker route becomes real parity.
4. Add minimal active input-only Font rows for each newly supported visible
   glyph stroke path.
5. Rerun `make -C pillow-rs font-tests` and Coverage MCP
   `font-tests-coverage-with-freetype`.

Do not add fake space-only stroke rows to cover branches. Empty stroked text is
now covered because it is a distinct public `PIL.ImageFont` behavior and exposed
a real size/offset mismatch. Remaining visible glyph stroke coverage must come
from lower-level stroker implementation, not from synthetic fixture padding.
