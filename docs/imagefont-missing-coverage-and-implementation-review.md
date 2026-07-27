# ImageFont missing coverage and implementation review

Date: 2026-07-27

Purpose: provide a decision document for what is still missing before claiming
truth-level parity between Pillow 12.2.0 `PIL.ImageFont` and the Rust
implementation.

This document combines two views:

1. Coverage-MCP uncovered-line / uncovered-region analysis.
2. Source-level comparison of Pillow `ImageFont` public behavior against Rust
   code across `pillow-rs/src/font/**`, `pillow-rs/src/lib.rs`, and the lower
   `pillow-rs-freetype/src/**` implementation used by FreeType-backed fonts.

## Evidence boundary

- Coverage MCP suite: `font-with-freetype`
- Latest trusted run: `974f35c7-e61d-4dec-bc8a-16ba4e91978e`
- Latest trusted snapshot: `06e0a61c-a56e-43e5-bfe7-a8b821be22f1`
- Measured commit: `13c410dc64fa93576f87377e2c8dde8f671f7ca9`
- Oracle: repo-local `.oracle-venv`, Pillow `12.2.0`,
  `PIL.ImageFont`, native `PIL._imagingft`
- Rust public entry points: `pillow-rs/src/lib.rs`,
  `pillow-rs/src/font/mod.rs`
- Rust FreeType adapter: `pillow-rs/src/font/imagingft.rs`
- Rust bitmap PILfont adapter: `pillow-rs/src/font/pilfont.rs`
- Lower FreeType implementation: `pillow-rs-freetype/src/**`
- Active fixture root: `pillow-rs/tests/fixtures/font/inputs/public-api`
- Manifest: `pillow-rs/tests/fixtures/font/font_manifest.yaml`
- Harness gate: `pillow-rs/tests/font_public_api.rs`

The trusted snapshot has 348 passing active input-only rows. The working tree
currently contains two additional uncommitted fixture rows, making 350 local
rows. Those two rows are not part of the trusted snapshot yet. One of them is a
known real mismatch: Pillow reports `OSError("too many instruction definitions")`
while Rust currently reports `OSError("invalid outline")`.

## Current defensible status

The current trusted active ImageFont fixture corpus has exact live-oracle parity
for the rows it exercises.

- Expected output is generated at runtime by Pillow 12.2.0.
- Input JSON files do not contain expected output hashes, expected errors,
  stored pixels, or oracle payloads.
- Rust results are compared against live Pillow results by `Result`-style
  success/error payload semantics.
- The manifest and test gate reject output-looking keys in fixture input JSON.
- The tested public operation list has 33 operations and includes
  `ImageFont.*`, `FreeTypeFont`-style operations, `TransposedFont.*`, load
  operations, variation operations, and controlled harness helpers.
- RAQM/libraqm successful shaping is explicitly out of scope. Current RAQM rows
  only prove no-libraqm error parity.

Correct claim:

> Current trusted active ImageFont fixture rows match Pillow 12.2.0 exactly.

Incorrect claim:

> Rust has complete `PIL.ImageFont` parity.

That is not defensible yet because uncovered regions and implementation gaps
remain, especially under stroked FreeType rendering and lower fontdone table /
glyph paths.

## 1. Uncovered-line logic-based analysis

### Direct Rust Font implementation coverage

Coverage snapshot `06e0a61c-a56e-43e5-bfe7-a8b821be22f1` reports:

| File | Lines | Branches | Functions | Regions | Decision |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | Covered by default FreeType font rows. |
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | Covered at adapter method level. |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | Bitmap `ImageFont.ImageFont` is mostly covered but not fully trusted. |
| `pillow-rs/src/font/imagingft.rs` | 1642/1666 98.56% | 246/254 96.85% | 163/174 93.68% | 2547/2645 96.29% | FreeType-backed ImageFont remains below the region goal. |

### `pillow-rs/src/font/imagingft.rs` remaining gaps

Coverage MCP reports 13 relevant direct gaps: 5 uncovered lines and 8
partial-branch lines.

| Line(s) | Coverage reason | Logic | Meaning | Decision |
|---:|---|---|---|---|
| `91` | partial branch | FreeType error-table miss handling. | Pillow `_imagingft.c::geterror` returns `unknown freetype error` for table misses. Rust has the behavior, but the miss branch is not reached by active public ImageFont rows. | Do not add private table unit tests as parity proof. Add only public rows if a real Pillow input can trigger this. |
| `92` | uncovered | `unknown freetype error` payload. | Same as above. | Same as above. |
| `253` | uncovered | `FT_Err_Too_Many_Instruction_Defs -> "too many instruction definitions"`. | A local pending fixture row already found a real mismatch: Rust currently collapses this path to `invalid outline`. | Fix lower `fontdone` error propagation/mapping so public ImageFont matches Pillow, then rerun Font coverage. |
| `271` | uncovered | `FT_Err_Invalid_Horiz_Metrics -> "invalid horizontal metrics"`. | Horizontal metrics errors can affect `truetype`, layout, metrics, bbox, and masks. Existing missing-`hmtx` coverage is a different error. | Add a real malformed public ImageFont row only if Pillow naturally reaches this exact error. |
| `796` | partial branch | Constant/declaration area around kerning defaults. | LLVM segment normalization artifact; not meaningful product behavior by itself. | No product change unless later source context proves a real missing branch. |
| `826`, `829` | partial branches | `floor26` / `ceil26` fixed-point conversion. | Can hide bbox and mask-size off-by-one differences for negative bearings, fractional starts, and descenders. | Add independent public rows that naturally cross floor/ceil boundaries. Avoid duplicate BASIC rows. |
| `928` | partial branch | `bbox_from_run_with_flags(..., load_flags)`. | Mono `mode="1"` rows for `AV` and `jQ` already prove useful public behavior, but this marker remains partial. | Treat as lower priority; do not chase with duplicate rows. Recheck after stroke/error work. |
| `1094`, `1097`, `1099` | partial/uncovered | Stroked bitmap width/height extent clamp. | This is Rust-only compatibility logic. Pillow allocates from `_imagingft.c::bounding_box_and_anchors` and clips writes. Width path executes; height body is still unproven. | Suspect implementation. Remove or prove with C/Pillow trace after lower stroker parity is fixed. |
| `1193`, `1194` | partial/uncovered | `stroke_filled=true` route to `FT_Outline_Glyph_StrokeBorder`. | Pillow supports successful `getmask2(..., stroke_filled=True)`. Rust has the public option wired, but real-glyph success is not proven. | Blocked by incomplete lower stroker/border export. Add success rows only after the lower implementation is real. |

### `pillow-rs/src/font/pilfont.rs` gap

`pilfont.rs` has high line coverage but weaker function/region coverage.
Coverage marks a rustdoc line around `from_pilfont_data` as uncovered, but the
important issue is broader: bitmap ImageFont paths are not yet 100% trusted by
region/function coverage. Add only public bitmap rows that prove distinct
Pillow behavior: malformed PIL descriptors, raster mode variants, glyph
clipping, missing glyphs, and text length boundaries. Do not count private
loader unit tests as ImageFont parity proof.

### Lower `pillow-rs-freetype` gaps that affect ImageFont trust

`imagingft.rs` is not the whole Font implementation. It delegates into
`pillow-rs-freetype` for face loading, glyph lookup, metrics, hinting,
rasterization, embedded bitmap handling, variations, and stroking. These lower
files must be covered through public `PIL.ImageFont` rows before the ImageFont
claim is trustworthy.

Current high-risk lower coverage from snapshot
`06e0a61c-a56e-43e5-bfe7-a8b821be22f1`:

| File | Lines | Branches | Functions | Regions | ImageFont risk |
|---|---:|---:|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 1107/8186 13.52% | 77/2075 3.71% | 94/586 16.04% | 1442/11495 12.54% | Very high: face, glyph, charmap, bitmap, stroker, variation, and handle routes. |
| `pillow-rs-freetype/src/api.rs` | 208/1186 17.54% | 35/294 11.90% | 25/105 23.81% | 275/1737 15.83% | High: lower public font API feeding ImageFont. |
| `pillow-rs-freetype/src/font.rs` | 1286/4747 27.09% | 161/702 22.93% | 126/392 32.14% | 1777/6728 26.41% | High: font load, face properties, glyph machinery, metrics. |
| `pillow-rs-freetype/src/render.rs` | 965/2459 39.24% | 157/486 32.30% | 76/158 48.10% | 1343/3432 39.13% | High: rendered mask byte parity. |
| `pillow-rs-freetype/src/tt/sbit.rs` | 100/814 12.29% | 13/72 18.06% | 13/108 12.04% | 186/1269 14.66% | High for embedded bitmap/color glyph behavior. |
| `pillow-rs-freetype/src/tt/cmap.rs` | 271/809 33.50% | 39/174 22.41% | 10/58 17.24% | 395/1089 36.27% | High for Unicode/bytes charmap behavior. |
| `pillow-rs-freetype/src/tt/glyf.rs` | 174/545 31.93% | 34/96 35.42% | 8/20 40.00% | 219/694 31.56% | High for TrueType outlines. |
| `pillow-rs-freetype/src/tt/cff.rs` | 355/735 48.30% | 37/112 33.04% | 29/81 35.80% | 507/1087 46.64% | High for CFF/OpenType outlines. |
| `pillow-rs-freetype/src/tt/hinter/exec.rs` | 722/1489 48.49% | 146/476 30.67% | 32/48 66.67% | 1296/3103 41.77% | High for hinted TrueType and bytecode error parity. |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 26/42 61.90% | 6/12 50.00% | 1/2 50.00% | 44/67 65.67% | Partially proven by public `font.getlength.hdmx_observable_av`; malformed paths remain untrusted. |
| `pillow-rs-freetype/src/tt/mvar.rs` | 58/67 86.57% | 3/6 50.00% | 4/7 57.14% | 92/113 81.42% | Partially proven by public `font.getmetrics.mvar_vertical_metrics`; unsupported/malformed value-tag paths remain untrusted. |
| `pillow-rs-freetype/src/tt/vhea.rs` | 8/11 72.73% | 1/2 50.00% | 1/1 100.00% | 8/9 88.89% | Partially proven by public `font.getmetrics.vertical_vhea_only`; short/error path remains untrusted. |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 28/50 56.00% | 3/8 37.50% | 1/2 50.00% | 44/65 67.69% | Partially proven by public `font.getmetrics.vertical_vhea_only`; malformed/overflow paths remain untrusted. |

Decision: do not treat near-complete `imagingft.rs` line coverage as enough.
The real target is public Pillow rows that naturally execute these lower files
and match Pillow output or error payload exactly.

## 2. Pillow `ImageFont` public surface vs Rust implementation

Pillow 12.2.0 source inspected:

- `.oracle-venv/lib/python3.12/site-packages/PIL/ImageFont.py`
- Native FreeType path: `PIL._imagingft`

Rust source inspected:

- `pillow-rs/src/lib.rs`
- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs/src/font/pilfont.rs`
- `pillow-rs/tests/support/font_runner.rs`

### Public surface matrix

| Pillow surface | Pillow public API | Rust implementation status | Missing/wrong/coverage decision |
|---|---|---|---|
| Module constants | `MAX_STRING_LENGTH`, `Layout.BASIC`, `Layout.RAQM` | `MAX_STRING_LENGTH` is internal to the Font adapters; BASIC is implemented; RAQM success is unsupported. | No-libraqm error parity is covered. Full RAQM parity is not implemented. |
| Module function | `load(filename)` | Represented through fixture `load` operation and `PilFont` byte/data loaders. | Core should not own filesystem I/O. Keep bindings thin and byte-oriented. |
| Module function | `load_path(filename)` | Represented through fixture operation. | Path search is binding/test-harness behavior, not core font logic. |
| Module function | `load_default_imagefont()` | Represented through bitmap default rows. | Behavior is covered, but bitmap `pilfont.rs` region/function coverage is not complete. |
| Module function | `load_default(size=None)` | Rust uses embedded Aileron subset through `ImageFont::load_default`. | Covered for current rows. Keep Pillow 12.2.0 Aileron behavior explicit. |
| Module function | `truetype(font, size, index=0, encoding="", layout_engine=None)` | Rust exposes root byte constructors and options: `imagefont_from_bytes*`, `ImageFont::from_bytes*`. | Correct architecture if Python/JS only load bytes and pass options. Stream/path object shape is not core-owned. |
| Bitmap class | `ImageFont.ImageFont.getbbox` | Implemented in `PilFont`/adapter paths. | Active rows pass; add distinct bitmap edge rows only where coverage indicates real behavior gaps. |
| Bitmap class | `ImageFont.ImageFont.getlength` | Implemented in `PilFont`/adapter paths. | Active rows pass; not full function/region coverage. |
| Bitmap class | `ImageFont.ImageFont.getmask` | Implemented in `PilFont`. | Active rows pass; remaining trust depends on malformed/raster/clipping cases. |
| Bitmap class | `ImageFont.ImageFont.info` | Implemented as `PilFont::info`. | Covered by active rows. |
| FreeType class | `FreeTypeFont.__init__` | `ImageFont::from_bytes_with_options`. | Constructor bytes/options covered; path/stream are binding-owned. |
| FreeType class | `__getstate__`, `__setstate__` | No explicit Rust public pickle/state API. | Missing if Rust API-shape parity includes serialization/state. Ignore only if product scope is behavior-only rendering/metrics. |
| FreeType class | `getname` | `ImageFont::getname` / root `imagefont_getname*`. | Covered. Keep missing-name rows because Pillow fallback details matter. |
| FreeType class | `getmetrics` | `ImageFont::getmetrics`. | Covered for standard/fixed-width/metric fallback rows; lower metrics tables are still undercovered. |
| FreeType class | `getlength` | `ImageFont::getlength*`. | BASIC rows pass. RAQM success missing. Rounding, bytecode error, and lower metrics edge rows remain. |
| FreeType class | `getbbox` | `ImageFont::getbbox*`. | BASIC rows pass. Rounding, anchor extremes, stroke, and lower bbox/stroker gaps remain. |
| FreeType class | `getmask` | `ImageFont::getmask*`. | BASIC rows pass. Stroked output and embedded bitmap paths remain weak. |
| FreeType class | `getmask2` | `ImageFont::getmask2*`. | BASIC/start/offset rows pass. Successful `stroke_filled=true` is not proven. |
| FreeType class | `font_variant` | `ImageFont::font_variant*`. | Covered by rows; variation-related lower tables remain only partially trusted. |
| FreeType class | `get_variation_names` | `ImageFont::get_variation_names`. | Covered by rows; name-table edge cases remain a lower-trust area. |
| FreeType class | `set_variation_by_name` | `ImageFont::set_variation_by_name`. | Covered by rows. Add render/metric rows after changes where distinct. |
| FreeType class | `get_variation_axes` | `ImageFont::get_variation_axes`. | Covered by rows. `mvar` is now exercised by a public metrics row, but malformed/value-tag paths remain. |
| FreeType class | `set_variation_by_axes` | `ImageFont::set_variation_by_axes`. | Covered by rows. Add rows that prove axes affect metrics/rendering where possible. |
| Transposed class | `TransposedFont.__init__` | No Rust class; helper operations use orientation. | Class-shape parity is missing. Behavior parity is covered by helper rows. |
| Transposed class | `TransposedFont.getmask` | `get_transposed_mask` helper. | Active rows pass. |
| Transposed class | `TransposedFont.getbbox` | `transposed_bbox` helper. | Active rows pass. |
| Transposed class | `TransposedFont.getlength` | `validate_transposed_length` helper. | Active rows pass. |

### Rust helper/test surfaces that are not Pillow public endpoints

These exist as binding/test adapters around Pillow behavior. They must not
become independent truth sources:

- `getbbox_binary`
- `getmask2_with_start`
- `get_transposed_mask`
- `transposed_bbox`
- `validate_transposed_length`
- `text_bbox`
- `draw_text`
- `render_text`
- `render_text_binary`

Decision: keep these only when each maps cleanly to a Pillow public behavior
under the runtime oracle. Do not let helper-specific expected values define
parity.

## 3. Missing or wrong Rust implementation across files

### A. General stroked outline support is incomplete

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/handles.rs`

Pillow behavior:

- `FreeTypeFont.getmask` / `getmask2` support `stroke_width`.
- `getmask2(..., stroke_filled=True)` routes through
  `FT_Glyph_StrokeBorder`.
- Stroked outline glyphs are rendered to bitmap with FreeType normal render
  mode even when the public `mode="1"` path set mono load flags.

Rust status:

- `ImageFontTextOptions` carries `stroke_width` and `stroke_filled`.
- `imagingft.rs` routes to `FT_Outline_Glyph_Stroke` or
  `FT_Outline_Glyph_StrokeBorder`.
- `FT_Stroker_ParseOutline` now follows FreeType 2.14.3
  `ftstroke.c:2067-2242` contour/tag parser control flow.
- The lower stroker/export implementation is still incomplete for general real
  glyph outlines.
- `FT_Outline_Glyph_Stroke` still has a DejaVu glyph-36 fixture-specific
  successful path documented in `handles.rs`.
- A prior stroked descender sweep showed Pillow succeeds for cases Rust still
  reports as `FT_Err_Unimplemented_Feature`.

Decision:

- This is a real implementation gap, not just missing coverage.
- Do not add more glyph-specific shortcuts.
- Finish general outline segment stroker geometry and border export in
  `pillow-rs-freetype`.
- Then add public Pillow rows for successful `stroke_width`,
  `stroke_filled=true`, descenders/ascenders such as `jQ`, mono stroke, and
  clipping at each side.

### B. Stroked extent clamp is suspect Rust-only behavior

File:

- `pillow-rs/src/font/imagingft.rs`

Rust mutates `x_max`/`y_max` when actual stroked bitmap extents exceed
bbox-derived expected dimensions. The code comment says this is a compatibility
clip retained because the current lower stroker can produce a larger bitmap than
the active Pillow-compatible target.

Pillow instead allocates through `_imagingft.c::bounding_box_and_anchors` and
clips while writing pixels.

Decision:

- Treat the clamp as temporary compatibility logic.
- Once lower stroker parity is fixed, remove the clamp unless a C/Pillow trace
  proves the same behavior.
- Current coverage executes the width condition but not the height body; that
  lack of coverage is useful because it points at unproven stroke geometry.

### C. TrueType bytecode error mapping is incomplete

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/convert.rs`
- `pillow-rs-freetype/src/tt/hinter/exec.rs`

Rust has table entries for FreeType bytecode errors, including
`FT_Err_Too_Many_Instruction_Defs`. The pending local fixture
`font.getlength.hinter_too_many_instruction_defs` proves a current mismatch:

- Pillow: `OSError("too many instruction definitions")`
- Rust: `OSError("invalid outline")`

Decision:

- This is a real parity bug.
- Fix lower `fontdone` error propagation/mapping so the public Rust Font result
  reaches the same FreeType status as Pillow.
- Keep the fixture row after the fix; it should cover `imagingft.rs:253`.

### D. Successful libraqm shaping is not implemented

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs/src/error.rs`
- `pillow-rs/tests/support/font_runner.rs`

Pillow can support successful `direction`, `features`, and `language` when
compiled with libraqm. The current oracle is no-libraqm, so Pillow returns
errors for those parameters.

Rust correctly uses a dedicated internal `PilError::UnsupportedLibraqm`, then
maps outward to the Pillow-compatible no-libraqm `KeyError` payload for the
fixture comparison.

Decision:

- This is an explicit scope exclusion, not complete ImageFont parity.
- Keep no-libraqm error rows.
- Do not claim full `PIL.ImageFont` parity unless successful RAQM shaping is
  implemented or permanently excluded from product scope.

### E. Bitmap `ImageFont.ImageFont` is not a 1:1 Rust type shape

Files:

- `pillow-rs/src/font/pilfont.rs`
- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/lib.rs`

Pillow has a bitmap `ImageFont.ImageFont` class and a FreeType
`ImageFont.FreeTypeFont` class. Rust currently has `PilFont` for bitmap fonts
and `ImageFont` for FreeType fonts.

Decision:

- If product scope requires Rust API-shape parity, introduce an explicit public
  shape that distinguishes bitmap `ImageFont`, `FreeTypeFont`, and
  `TransposedFont`.
- If product scope is behavior parity only, the split can remain, but fixture
  coverage must reach trusted region/function coverage for `pilfont.rs`.

### F. `FreeTypeFont.__getstate__` / `__setstate__` are not represented

File:

- Pillow `ImageFont.py`

Rust has no direct public pickle/state equivalent. This may be acceptable for a
Rust core API, but it is a public Pillow class surface.

Decision:

- Explicitly exclude state/pickle parity if the target is rendering/metrics
  behavior only.
- If Python API parity is in scope later, add thin binding-level behavior and
  public rows comparing Pillow state roundtrips.

### G. Path and stream behavior is binding-owned

Files:

- `pillow-rs/src/lib.rs`
- `pillow-rs-py`
- `pillow-rs-js`
- `pillow-rs/tests/support/font_runner.rs`

Pillow accepts paths and binary streams. Core Rust accepts bytes plus options.

Decision:

- This is correct architecture. Core should not own filesystem or Python/JS
  object behavior.
- Bindings may load bytes, but must not implement font parsing, layout,
  rendering, or parity comparison logic.

### H. Embedded bitmap, cmap, CFF, variation, and vertical/device metrics remain undertrusted

Files:

- `pillow-rs-freetype/src/tt/sbit.rs`
- `pillow-rs-freetype/src/tt/cmap.rs`
- `pillow-rs-freetype/src/tt/cff.rs`
- `pillow-rs-freetype/src/tt/hdmx.rs`
- `pillow-rs-freetype/src/tt/mvar.rs`
- `pillow-rs-freetype/src/tt/vhea.rs`
- `pillow-rs-freetype/src/tt/vmtx.rs`
- `pillow-rs-freetype/src/tt/gvar.rs`
- `pillow-rs-freetype/src/tt/hvar.rs`

Current public rows have started exercising hdmx, mvar, vhea, and vmtx, but
coverage remains partial. Embedded bitmap coverage is particularly weak.

Decision:

- Add public ImageFont rows that naturally hit each supported lower table path.
- If a table path does not affect supported ImageFont behavior, record the
  exclusion explicitly.
- Do not count lower FreeType-only unit tests as ImageFont parity proof.

## 4. Active fixture files and case counts

Trusted snapshot case count: 348.

Working-tree count including uncommitted pending rows: 350.

| Input file | Working-tree cases |
|---|---:|
| `font.ImageFont.getbbox.json` | 4 |
| `font.ImageFont.getlength.json` | 4 |
| `font.ImageFont.getmask.json` | 19 |
| `font.ImageFont.info.json` | 3 |
| `font.TransposedFont.getbbox.json` | 3 |
| `font.TransposedFont.getlength.json` | 3 |
| `font.TransposedFont.getmask.json` | 6 |
| `font.constructor.json` | 9 |
| `font.get_transposed_mask.json` | 10 |
| `font.getbbox.json` | 32 |
| `font.getbbox_binary.json` | 9 |
| `font.getlength.json` | 22 |
| `font.getmask.json` | 36 |
| `font.getmask2.json` | 43 |
| `font.getmask2_with_start.json` | 23 |
| `font.getmetrics.json` | 8 |
| `font.getname.json` | 5 |
| `font.has_variations.json` | 4 |
| `font.layout_failure.json` | 1 |
| `font.load.json` | 25 |
| `font.load_default_imagefont.json` | 1 |
| `font.load_failure.json` | 8 |
| `font.load_path.json` | 1 |
| `font.render_text.json` | 7 |
| `font.render_text_binary.json` | 9 |
| `font.text_bbox.json` | 6 |
| `font.transposed_bbox.json` | 7 |
| `font.unsupported_operation.json` | 1 |
| `font.validate_transposed_length.json` | 5 |
| `font.variations.json` | 36 |
| Total | 350 |

Pending rows not included in the trusted snapshot:

- `font.getbbox.hhea_descender_only_av`
- `font.getlength.hinter_too_many_instruction_defs`

The second row is intentionally failing until the bytecode error mapping is
fixed. Keep it as a real bug detector.

## 5. Action list for decision

1. Fix the bytecode error mismatch so
   `font.getlength.hinter_too_many_instruction_defs` matches Pillow and covers
   `imagingft.rs:253`.
2. Complete general stroked outline support in
   `pillow-rs-freetype/src/ffi/handles.rs`; remove fixture-specific stroker
   shortcuts instead of adding more.
3. Re-evaluate and either remove or prove the `imagingft.rs` stroked extent
   clamp with a C/Pillow trace.
4. Add public ImageFont rows for successful stroke, `stroke_filled=true`,
   stroked descenders/ascenders, mono stroke, and side clipping after lower
   stroker support works.
5. Add targeted public rows for `floor26`/`ceil26` rounding via negative
   bearings, fractional `start`, descenders, and glyphs crossing pixel
   boundaries.
6. Add public rows for embedded bitmap/sbit behavior and supported cmap/CFF
   shapes. Use live Pillow output only.
7. Decide whether Python/Rust API-shape parity includes
   `FreeTypeFont.__getstate__` / `__setstate__` and distinct public type shapes
   for bitmap `ImageFont`, `FreeTypeFont`, and `TransposedFont`.
8. Keep RAQM/libraqm success excluded unless the product decision changes.
   Current no-libraqm rows are not proof of successful shaping parity.

## Final conclusion

The branch has a trustworthy runtime-oracle fixture harness for current active
rows, but it does not yet have complete `PIL.ImageFont` parity.

The biggest real implementation blocker is general stroked outline behavior
under `pillow-rs-freetype/src/ffi/handles.rs`. The next concrete bug is the
pending TrueType bytecode error mapping mismatch for
`too many instruction definitions`. Coverage is useful here because it shows
which Rust paths are merely present versus actually proven through public
Pillow ImageFont behavior.
