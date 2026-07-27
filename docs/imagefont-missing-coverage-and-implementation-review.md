# ImageFont missing coverage and implementation review

Date: 2026-07-27

Scope reviewed:

- Pillow oracle: repo-local `.oracle-venv`, Pillow `12.2.0`, `PIL.ImageFont.py`, native `PIL._imagingft`.
- Rust public surface: `pillow-rs/src/lib.rs`, `pillow-rs/src/font/mod.rs`, `pillow-rs/src/font/pilfont.rs`, `pillow-rs/src/font/imagingft.rs`.
- Lower FreeType implementation used by ImageFont: `pillow-rs-freetype/src/ffi/handles.rs` and related `pillow-rs-freetype/src/*`.
- Active tests only: `pillow-rs/tests/fixtures/font/inputs/public-api` through `pillow-rs/tests/font_public_api.rs`.

Coverage evidence:

- Coverage MCP suite: `font-with-freetype`
- Run: `e5d5a1c8-7070-4260-9fcb-f22f832f1c32`
- Snapshot: `3f959b1c-cb26-4af4-92e3-c6c0c736163e`
- Measured commit: `71e191b50dd59dd752024bccefe02a91819a0809`
- Current later commits after that snapshot are documentation-only for this area; do not treat them as new coverage evidence.

## Current defensible status

The active ImageFont fixture corpus is exact live-oracle parity for the rows it covers.

- 336 input-only fixture rows are active.
- Expected output is generated at runtime by Pillow 12.2.0, not stored in the input JSON.
- The Rust test compares Rust `Result` status/payload to live Pillow status/payload.
- RAQM rows are only no-libraqm error parity rows.

The correct claim is:

> Current active ImageFont fixture rows match Pillow 12.2.0 exactly.

The incorrect claim is:

> Rust has complete `PIL.ImageFont` parity.

That is not defensible yet because uncovered regions and known implementation mismatches remain.

## 1. Uncovered-line logic-based analysis

Coverage MCP reports this for `pillow-rs/src/font/imagingft.rs`:

| Metric | Covered | Total | Rate |
|---|---:|---:|---:|
| lines | 1642 | 1666 | 98.56% |
| branches | 246 | 254 | 96.85% |
| functions | 163 | 174 | 93.68% |
| regions | 2547 | 2645 | 96.29% |

Relevant remaining line gaps:

| Line(s) | Coverage reason | Logic | Meaning | Decision |
|---:|---|---|---|---|
| 91, 92, 253, 271 | partial/uncovered | FreeType error message table and table miss handling | The table is source-aligned with Pillow `_imagingft.c::geterror`, but rare FreeType status values are not reachable through current public ImageFont fixtures. | Do not add private table unit tests as proof. Add only public `PIL.ImageFont` inputs that naturally trigger these errors. |
| 796 | partial branch | `KERN_DEFAULT` constant declaration area | Coverage artifact marks this from LLVM segment normalization. It is not meaningful product behavior. | No product change. Keep recorded as coverage artifact noise unless future MCP source context proves otherwise. |
| 826, 829 | partial branches | `floor26` / `ceil26` 26.6 conversion helpers | Current rows do not cover all rounding-region shapes. This can hide bbox, offset, and mask-size edge differences. | Add public rows with negative bearings, fractional starts, descenders, ascenders, and fonts/glyphs that cross floor/ceil boundaries. |
| 928 | partial branch | `bbox_from_run_with_flags(..., load_flags)` path | This line is part of BASIC bbox flow; the remaining partial branch indicates not all load-flag/bbox paths are independently proven. | Add minimal rows that exercise normal and mono load flags across `getbbox`, `getbbox_binary`, `getmask`, `getmask2`, and byte text. |
| 1094, 1097, 1099 | partial/uncovered | stroked width/height extent clamps | This is Rust-only compatibility logic around stroked bitmap extents. Width path executes; height clamp body remains unproven. | Treat as suspect until lower stroker/bbox parity is fixed. Do not preserve this permanently unless C/Pillow trace proves equivalent behavior. |
| 1193, 1194 | partial/uncovered | `stroke_filled=true` routes to `FT_Outline_Glyph_StrokeBorder` | The public option is wired, but no active successful ImageFont row proves real `stroke_filled=true` output. | Blocked by lower general stroker support. Add public success rows only after real outline stroke-border works. |

Other direct `pillow-rs/src/font` coverage:

| File | Line status | Region status | Meaning |
|---|---:|---:|---|
| `pillow-rs/src/font/default_aileron.rs` | 100.00% | 100.00% | Covered by default FreeType font rows. |
| `pillow-rs/src/font/mod.rs` | 100.00% | 100.00% | Root ImageFont adapter methods are covered. |
| `pillow-rs/src/font/pilfont.rs` | 97.01% | 92.69% | Bitmap `ImageFont.ImageFont` is mostly covered, but functions/regions are not fully trusted yet. |
| `pillow-rs/src/font/imagingft.rs` | 98.56% | 96.29% | FreeType-backed ImageFont remains below region goal. |

Lower FreeType files are still a larger trust gap for ImageFont because `imagingft.rs` delegates into them:

| File | Line coverage | Region coverage | Risk |
|---|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 13.05% | 12.10% | High. Contains FreeType ABI-style handles, glyph, stroker, bitmap, charmap, and face routes used under ImageFont. |
| `pillow-rs-freetype/src/api.rs` | 17.54% | 15.83% | High. Public fontdone API paths feeding ImageFont. |
| `pillow-rs-freetype/src/font.rs` | 26.54% | 25.64% | High. Face loading, glyph machinery, metrics. |
| `pillow-rs-freetype/src/render.rs` | 39.24% | 39.13% | High. Render output parity. |
| `pillow-rs-freetype/src/tt/sbit.rs` | 12.29% | 14.66% | High for embedded bitmap/color glyph rows. |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 0.00% | 0.00% | Unproven horizontal device metrics. |
| `pillow-rs-freetype/src/tt/mvar.rs` | 0.00% | 0.00% | Unproven variation metric deltas. |
| `pillow-rs-freetype/src/tt/vhea.rs` | 0.00% | 0.00% | Unproven vertical metrics. |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 0.00% | 0.00% | Unproven vertical metrics. |

## 2. Pillow `ImageFont` public surface vs Rust implementation

Pillow 12.2.0 exposes these relevant public surfaces:

| Pillow surface | Public API | Rust status |
|---|---|---|
| module functions | `load`, `load_path`, `load_default_imagefont`, `load_default`, `truetype` | Represented by Rust byte-oriented constructors and fixture operations. Filesystem path handling is intentionally binding/test harness owned, not core owned. |
| `ImageFont.ImageFont` bitmap class | `getmask`, `getbbox`, `getlength`, `info` | Implemented as `PilFont`, not the same Rust class shape as FreeType `ImageFont`. Covered by active bitmap fixture rows, but `pilfont.rs` is not 100% region covered. |
| `ImageFont.FreeTypeFont` class | constructor, `getname`, `getmetrics`, `getlength`, `getbbox`, `getmask`, `getmask2`, `font_variant`, variations APIs | Mostly implemented as Rust `ImageFont`. BASIC no-raqm path is parity-tested. Successful libraqm shaping is out of scope. Successful `stroke_filled=true` is not proven. |
| `ImageFont.TransposedFont` class | constructor, `getmask`, `getbbox`, `getlength` | Rust exposes helper-style operations, not a class. Fixture rows cover behavior, but class-shape parity is not implemented. |
| `Layout` enum | `Layout.BASIC`, `Layout.RAQM` | BASIC is implemented. RAQM success is intentionally unsupported. No-libraqm error parity is tested. |

Rust extra helper operations that are not Pillow public methods:

- `getbbox_binary`
- `getmask2_with_start`
- `get_transposed_mask`
- `transposed_bbox`
- `validate_transposed_length`
- `text_bbox`
- `render_text`
- `render_text_binary`

These are acceptable only as binding/test adapters. They should not become independent public behavior beyond what Pillow rows prove.

## 3. Missing or wrong Rust implementation across files

### A. General stroker implementation is incomplete

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/handles.rs`

Observed issue:

- `imagingft.rs` routes stroked text through `FT_Outline_Glyph_Stroke` or `FT_Outline_Glyph_StrokeBorder`.
- `handles.rs` currently has a DejaVu glyph-36 fixture-specific guard around `FT_Outline_Glyph_Stroke`.
- Real conic/cubic/closed glyph segment parsing/export remains pending in `FT_Stroker_ParseOutline`.
- Exploratory stroked descender rows such as `jQ` showed Pillow succeeds but Rust returns `FT_Err_Unimplemented_Feature`.

Decision:

- This is wrong/incomplete implementation, not just missing coverage.
- Do not add more glyph-specific shortcuts.
- Fix by implementing general outline parse/export in `pillow-rs-freetype`, then add public Pillow rows for stroked ascenders, descenders, mono mode, and `stroke_filled=true`.

### B. Stroked extent clamp is a Rust-only compatibility shim

File:

- `pillow-rs/src/font/imagingft.rs`

Observed issue:

- Rust mutates `x_max`/`y_max` when actual stroked bitmap extents exceed expected bbox-derived dimensions.
- Pillow allocates from `_imagingft.c::bounding_box_and_anchors` and clips during render.
- Current coverage proves the width clamp but not the height clamp body.

Decision:

- Treat as suspect. It may be compensating for lower stroker geometry mismatch.
- After general stroker support lands, either remove this shim or prove it with a C/Pillow trace and fixture rows.

### C. Successful RAQM shaping is not implemented

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs/src/error.rs`
- `pillow-rs/tests/support/font_runner.rs`

Observed issue:

- Pillow supports successful `direction`, `features`, and `language` shaping when built with libraqm.
- Current oracle environment is no-libraqm, so Pillow returns an error for these arguments.
- Rust correctly returns a dedicated internal `PilError::UnsupportedLibraqm`, then maps outward to the Pillow-compatible `KeyError` payload.

Decision:

- This is an explicit scope exclusion, not full parity.
- Keep rows proving no-libraqm error behavior.
- Do not claim full `PIL.ImageFont` until either RAQM success is implemented or permanently excluded in product scope.

### D. Bitmap `ImageFont.ImageFont` is implemented as `PilFont`, not one class shape

Files:

- `pillow-rs/src/font/pilfont.rs`
- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/lib.rs`

Observed issue:

- Pillow has `ImageFont.ImageFont` for bitmap fonts and `FreeTypeFont` for TrueType/OpenType fonts.
- Rust has `PilFont` for bitmap fonts and `ImageFont` for FreeType fonts.
- This is operationally testable, but not a 1:1 type shape.

Decision:

- If the product goal is Rust API shape parity with Pillow names, decide whether to introduce a public enum/class-like wrapper or rename/split surfaces.
- If the product goal is behavior parity only, keep the split but continue fixture coverage until `pilfont.rs` reaches 100% trusted region coverage.

### E. Path and stream behavior is not core-owned

Files:

- `pillow-rs/src/lib.rs`
- `pillow-rs-py`
- `pillow-rs-js`
- `pillow-rs/tests/support/font_runner.rs`

Observed issue:

- Pillow module functions accept paths and binary streams.
- Core Rust takes bytes and options.

Decision:

- This is correct architecture if bindings stay thin.
- Binding crates may load bytes, but must not own font parsing, glyph rendering, layout, or Pillow-specific comparison logic.

### F. Embedded bitmap, variation metrics, and vertical metrics are untrusted

Files:

- `pillow-rs-freetype/src/tt/sbit.rs`
- `pillow-rs-freetype/src/tt/hdmx.rs`
- `pillow-rs-freetype/src/tt/mvar.rs`
- `pillow-rs-freetype/src/tt/vhea.rs`
- `pillow-rs-freetype/src/tt/vmtx.rs`

Observed issue:

- Coverage is low or zero in lower FreeType tables that can affect ImageFont rendering/metrics.
- Active fixtures include embedded-bitmap font assets, but the measured lower coverage is not high enough to trust all table paths.

Decision:

- Add public ImageFont fixture rows that naturally exercise these paths, or explicitly mark each table irrelevant to current product scope.
- Do not add FreeType-only unit tests and count them as ImageFont parity proof.

### G. Error table completeness is not behavior coverage

File:

- `pillow-rs/src/font/imagingft.rs`

Observed issue:

- Rust has a broad FreeType error mapping table.
- Several table entries are uncovered because no public ImageFont row reaches those exact FreeType errors.

Decision:

- Table presence is not enough for parity trust.
- Add only real public Pillow inputs that trigger the same error class/message behavior.

## 4. What is currently covered by fixtures

Active input files and case counts:

| Input file | Cases |
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
| `font.getbbox.json` | 29 |
| `font.getbbox_binary.json` | 9 |
| `font.getlength.json` | 18 |
| `font.getmask.json` | 35 |
| `font.getmask2.json` | 42 |
| `font.getmask2_with_start.json` | 23 |
| `font.getmetrics.json` | 4 |
| `font.getname.json` | 5 |
| `font.has_variations.json` | 4 |
| `font.layout_failure.json` | 1 |
| `font.load.json` | 25 |
| `font.load_default_imagefont.json` | 1 |
| `font.load_failure.json` | 7 |
| `font.load_path.json` | 1 |
| `font.render_text.json` | 7 |
| `font.render_text_binary.json` | 9 |
| `font.text_bbox.json` | 6 |
| `font.transposed_bbox.json` | 7 |
| `font.unsupported_operation.json` | 1 |
| `font.validate_transposed_length.json` | 5 |
| `font.variations.json` | 36 |
| total | 336 |

## 5. Action list for decision

Recommended next actions, in order:

1. Fix general FreeType stroker support in `pillow-rs-freetype/src/ffi/handles.rs`; do not add fixture-specific shortcuts.
2. Add public ImageFont rows for successful stroked descenders, `stroke_filled=true`, mono stroked output, and height-side clipping.
3. Add public rows for `floor26`/`ceil26` edge behavior: negative bearings, fractional starts, ascenders/descenders, and bbox/mask offsets.
4. Add public rows that hit embedded bitmap, horizontal device metrics, and variation metric paths if those are in product scope.
5. Decide whether Rust public type shape must mirror Pillow class shape (`ImageFont.ImageFont`, `FreeTypeFont`, `TransposedFont`) or whether behavior-only parity is enough.
6. Keep RAQM marked out of scope unless the product decision changes.
7. Re-run `make -C pillow-rs font-tests`.
8. Re-run Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`.
9. Update this file and `docs/imagefont-parity-gap-analysis.md` with new run/snapshot IDs and remove only gaps proven by live Pillow oracle rows.

## Final decision point

The branch currently has trustworthy parity for the active 336-row live Pillow corpus.

The branch does not yet have complete `PIL.ImageFont` parity. The main blocker is not more fixture JSON; it is incomplete general stroke geometry under `pillow-rs-freetype/src/ffi/handles.rs`. More fixture rows should be added after that implementation becomes real, otherwise the new rows will correctly fail against Pillow.
