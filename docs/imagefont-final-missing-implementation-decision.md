# ImageFont missing coverage and implementation decision list

Date: 2026-07-27

Target: Pillow `PIL.ImageFont` 12.2.0 parity in `pillow-rs`.

This document is the consolidated decision file for the current ImageFont gap
review. It combines:

1. Coverage-MCP uncovered-line / uncovered-region analysis.
2. A source-surface comparison of Pillow `PIL.ImageFont` 12.2.0 against the
   Rust implementation across `pillow-rs/src/font/**`,
   `pillow-rs/src/lib.rs`, and the lower `pillow-rs-freetype/src/**` paths used
   by FreeType-backed ImageFont.

Important evidence boundary:

- Coverage run: `a6721cc4-bd8e-4049-8846-a913fb52f71e`
- Coverage snapshot: `2c1810bd-489d-49aa-96d5-bbaa5de7c71d`
- Suite: `font-with-freetype`
- Measured commit: `fd0bb7ccafd8968031e962c1f3e12c5102a5e5f0`
- Oracle: repo-local `.oracle-venv`, Pillow `12.2.0`, native
  `PIL._imagingft`
- Stroker parser note: commit `fd0bb7ccafd8968031e962c1f3e12c5102a5e5f0`
  makes `FT_Stroker_ParseOutline` follow FreeType 2.14.3 contour/tag parser
  control flow and delegate to existing line/conic/cubic segment routes. This
  does not yet prove successful ImageFont stroke parity because the maintained
  mixed-outline route, general segment stroker geometry, and border export are
  still pending.

## Current defensible status

The active input-only Font fixture corpus has exact live-oracle parity for the
rows it exercises.

- Active rows: 345.
- Oracle outputs are generated at runtime by Pillow 12.2.0.
- Input JSON files do not contain expected output hashes, pixel data, or
  expected errors.
- Rust results are compared against Pillow results by `Result`-style
  success/error payload semantics.
- `make -C pillow-rs font-tests` passed before this document.
- Coverage MCP managed run passed and ingested snapshot
  `2c1810bd-489d-49aa-96d5-bbaa5de7c71d`.

The correct product claim is:

> Current active ImageFont fixture rows match Pillow 12.2.0 exactly.

The incorrect product claim is:

> Rust has complete `PIL.ImageFont` parity.

That is not true yet.

## 1. Uncovered-line logic-based analysis

### Direct Font implementation coverage

Coverage snapshot `2c1810bd-489d-49aa-96d5-bbaa5de7c71d` reports:

| File | Lines | Branches | Functions | Regions | Decision |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | Covered. |
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | Covered at adapter level. |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | Bitmap ImageFont is not fully trusted. |
| `pillow-rs/src/font/imagingft.rs` | 1642/1666 98.56% | 246/254 96.85% | 163/174 93.68% | 2547/2645 96.29% | FreeType ImageFont is not fully trusted. |

### `imagingft.rs` uncovered/partial lines

| Line(s) | Logic | Why it matters | Decision |
|---:|---|---|---|
| 91, 92 | `FT_ERROR_MESSAGES` table miss comment/table start. | Pillow `_imagingft.c::geterror` uses FreeType error tables and reports `unknown freetype error` for misses. The Rust table exists, but the table-miss route is not reached through current public ImageFont rows. | Do not add private table unit tests as parity proof. Add only public ImageFont inputs that naturally trigger the same Pillow error behavior. |
| 253 | `FT_Err_Too_Many_Instruction_Defs` mapping. | Rare TrueType instruction error is mapped but not reached by active public rows. | Needs a real malformed-font row if Pillow can trigger it through `ImageFont.truetype`. |
| 271 | `FT_Err_Invalid_Horiz_Metrics` mapping. | Horizontal metrics failures affect `truetype`, metrics, layout, and mask creation. | Needs a real malformed-font row distinct from the existing missing-`hmtx` row. |
| 796 | `KERN_DEFAULT` declaration segment. | LLVM line attribution marks this partial, but this is a constant declaration rather than product behavior. | Treat as coverage artifact unless source-context evidence later proves a real missing branch. |
| 826, 829 | `floor26` / `ceil26` 26.6 fixed-point helpers. | These determine bbox and mask edge rounding. Partial branch coverage can hide off-by-one parity bugs. | Add public rows with negative bearings, fractional start offsets, descenders, ascenders, and glyphs that cross floor/ceil boundaries. |
| 928 | `bbox_from_run_with_flags(..., load_flags)` parameter path. | BASIC layout with normal/mono load flags is important for `getbbox`, `getmask`, and `getmask2`. Existing mono rows prove useful behavior but do not close this region marker. | Do not add duplicate mono rows only to chase this marker. Recheck after stroker/error work. |
| 1094, 1097, 1099 | Stroked bitmap extent clamp. | This is Rust-only compatibility logic around stroked bitmap extents. Pillow allocates from `_imagingft.c::bounding_box_and_anchors` and clips writes. | Suspect implementation. Remove or prove by C/Pillow trace after lower stroker parity is fixed. |
| 1193, 1194 | `stroke_filled=true` route to `FT_Outline_Glyph_StrokeBorder`. | Pillow supports `getmask2(..., stroke_filled=True)`. Rust has the option wired, but successful real-glyph stroke-border output is not proven. | Blocked by incomplete lower stroker implementation. Add public success rows only after lower support works. |

### `pilfont.rs` uncovered line

| Line | Logic | Why it matters | Decision |
|---:|---|---|---|
| 140 | Rustdoc line documenting truncated descriptor error from `from_pilfont_data`. | Coverage marks this as uncovered because the function entry is not used directly; the active harness uses `from_pilfont_glyph_data` / loader-style paths. | Not a behavior gap by itself, but `pilfont.rs` region/function coverage is still below target. Add public bitmap ImageFont rows only where they prove distinct Pillow behavior. |

### Lower `pillow-rs-freetype` coverage gaps that affect ImageFont trust

`imagingft.rs` delegates face loading, glyph lookup, glyph metrics, hinting,
rasterization, embedded bitmap handling, variations, and stroking into
`pillow-rs-freetype`. These lower files are therefore in scope for ImageFont
trust, even if the public test entry point is `PIL.ImageFont`.

High-risk lower coverage from the same snapshot:

| File | Lines | Branches | Functions | Regions | Risk |
|---|---:|---:|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 1056/8093 13.05% | 74/2045 3.62% | 90/581 15.49% | 1375/11364 12.10% | Very high: handles, glyphs, charmap, bitmap, stroker. |
| `pillow-rs-freetype/src/api.rs` | 208/1186 17.54% | 35/294 11.90% | 25/105 23.81% | 275/1737 15.83% | High: public lower font API. |
| `pillow-rs-freetype/src/font.rs` | 1266/4747 26.67% | 157/702 22.36% | 119/392 30.36% | 1735/6728 25.79% | High: face loading, metrics, glyph machinery. |
| `pillow-rs-freetype/src/render.rs` | 965/2459 39.24% | 157/486 32.30% | 76/158 48.10% | 1343/3432 39.13% | High: rendered mask bytes. |
| `pillow-rs-freetype/src/tt/sbit.rs` | 100/814 12.29% | 13/72 18.06% | 13/108 12.04% | 186/1269 14.66% | High for embedded bitmap/color glyph rows. |
| `pillow-rs-freetype/src/tt/cmap.rs` | 271/809 33.50% | 39/174 22.41% | 10/58 17.24% | 395/1089 36.27% | High for char mapping and byte/unicode behavior. |
| `pillow-rs-freetype/src/tt/glyf.rs` | 174/545 31.93% | 34/96 35.42% | 8/20 40.00% | 219/694 31.56% | High for TrueType outlines. |
| `pillow-rs-freetype/src/tt/cff.rs` | 355/735 48.30% | 37/112 33.04% | 29/81 35.80% | 507/1087 46.64% | High for CFF/OpenType outlines. |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 0/42 0.00% | 0/12 0.00% | 0/2 0.00% | 0/67 0.00% | Unproven horizontal device metrics. |
| `pillow-rs-freetype/src/tt/mvar.rs` | 0/67 0.00% | 0/6 0.00% | 0/7 0.00% | 0/113 0.00% | Unproven variation metric deltas. |
| `pillow-rs-freetype/src/tt/vhea.rs` | 0/11 0.00% | 0/2 0.00% | 0/1 0.00% | 0/9 0.00% | Unproven vertical metrics. |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 0/50 0.00% | 0/8 0.00% | 0/2 0.00% | 0/65 0.00% | Unproven vertical metrics. |

Decision: do not treat `imagingft.rs` near-100% line coverage as enough.
ImageFont parity requires public Pillow rows that naturally execute the lower
FreeType implementation and compare live Pillow output.

## 2. Pillow ImageFont 12.2.0 surface vs Rust coverage/implementation

Source inspected:

- Pillow: `.oracle-venv/lib/python3.12/site-packages/PIL/ImageFont.py`
- Rust public API: `pillow-rs/src/lib.rs`
- Rust Font adapter: `pillow-rs/src/font/mod.rs`
- Rust FreeType adapter: `pillow-rs/src/font/imagingft.rs`
- Rust bitmap PILfont adapter: `pillow-rs/src/font/pilfont.rs`

### Public surface matrix

| Pillow surface | Pillow public member | Rust status | Coverage/implementation decision |
|---|---|---|---|
| Module | `load` | Modeled by input fixtures and byte-oriented core path. | Keep I/O in bindings/tests. Core should not own filesystem behavior. |
| Module | `load_path` | Modeled in fixtures, but path search is not core-owned. | Thin binding/test harness responsibility. Do not move search logic into core. |
| Module | `load_default_imagefont` | Covered by bitmap default row. | Covered behavior, but `pilfont.rs` regions still below target. |
| Module | `load_default` | Covered for default FreeType font path. | Covered rows pass; keep Pillow 12.2.0 embedded Aileron behavior explicit. |
| Module | `truetype` | Modeled as `imagefont_from_bytes*` / `ImageFont::from_bytes*`. | Correct architecture if bindings only load bytes and pass options. Missing full path/stream API shape in core is intentional. |
| Class | `ImageFont.ImageFont.getbbox` | Rust bitmap `PilFont` supports equivalent behavior via harness operations. | Active rows pass; not 100% region/function coverage. |
| Class | `ImageFont.ImageFont.getlength` | Rust bitmap `PilFont::getsize`/adapter computes length. | Active rows pass; add only distinct bitmap edge rows if needed. |
| Class | `ImageFont.ImageFont.getmask` | Rust bitmap `PilFont::getmask`. | Active rows pass; ensure mode `1`, `L`, clipping, missing glyphs remain public-oracle-tested. |
| Class | `ImageFont.ImageFont.info` | Rust exposes `PilFont::info`. | Active rows pass. |
| Class | `FreeTypeFont.__init__` | Rust `ImageFont::from_bytes_with_options`. | Constructor bytes/options covered; path object behavior is binding-owned. |
| Class | `FreeTypeFont.getname` | Rust `ImageFont::getname`. | Covered; fallback names differ only if not matched by rows. Keep missing-name rows. |
| Class | `FreeTypeFont.getmetrics` | Rust `ImageFont::getmetrics`. | Covered for standard, fixed-width, and hhea-zero/no-OS2 rows; lower metrics tables remain undercovered. |
| Class | `FreeTypeFont.getlength` | Rust `ImageFont::getlength*`. | BASIC rows pass. Libraqm success is missing by scope. Rounding/kerning edge coverage still needed. |
| Class | `FreeTypeFont.getbbox` | Rust `ImageFont::getbbox*`. | BASIC rows pass. Rounding, anchor, stroke, and lower bbox/stroker gaps remain. |
| Class | `FreeTypeFont.getmask` | Rust `ImageFont::getmask*`. | BASIC rows pass. Stroked output and embedded bitmap paths remain weak. |
| Class | `FreeTypeFont.getmask2` | Rust `ImageFont::getmask2*`. | BASIC/start/offset rows pass. `stroke_filled=true` success is not proven. |
| Class | `FreeTypeFont.font_variant` | Rust `ImageFont::font_variant*`. | Covered through fixture rows; validate lower variation-table coverage separately. |
| Class | `FreeTypeFont.get_variation_names` | Rust `ImageFont::get_variation_names`. | Covered by rows but lower variation coverage is incomplete. |
| Class | `FreeTypeFont.set_variation_by_name` | Rust `ImageFont::set_variation_by_name`. | Covered by rows; lower variation edge cases remain. |
| Class | `FreeTypeFont.get_variation_axes` | Rust `ImageFont::get_variation_axes`. | Covered by rows; `mvar` remains 0% and variation metric deltas are unproven. |
| Class | `FreeTypeFont.set_variation_by_axes` | Rust `ImageFont::set_variation_by_axes`. | Covered by rows; add metric/render rows after axis changes if not already distinct. |
| Class | `TransposedFont.__init__` | Rust does not expose a class; helper operations use orientation. | Behavior can be tested, but class-shape parity is not implemented. |
| Class | `TransposedFont.getmask` | Rust `get_transposed_mask`. | Behavior rows pass. |
| Class | `TransposedFont.getbbox` | Rust `transposed_bbox` helper. | Behavior rows pass. |
| Class | `TransposedFont.getlength` | Rust `validate_transposed_length`. | Behavior/error rows pass. |
| Enum/constant | `Layout.BASIC`, `Layout.RAQM`, `MAX_STRING_LENGTH` | BASIC supported; RAQM success unsupported; max length implemented. | No-libraqm error parity is covered. Full RAQM parity is excluded. |

### Rust helper/test surfaces that are not Pillow public API

These exist in Rust or fixtures but are adapters around Pillow behavior, not
independent Pillow public endpoints:

- `getbbox_binary`
- `getmask2_with_start`
- `get_transposed_mask`
- `transposed_bbox`
- `validate_transposed_length`
- `text_bbox`
- `render_text`
- `render_text_binary`

Decision: keep them only as controlled harness/binding helpers. They must not
become their own source of truth, and they must not hide a mismatch with a real
Pillow public method.

## 3. Wrong or missing Rust implementation across files

### A. General stroker support is incomplete

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs-freetype/src/ffi/handles.rs`

Current issue:

- Pillow `FreeTypeFont.getmask/getmask2` supports `stroke_width`.
- Pillow `getmask2(..., stroke_filled=True)` routes through
  `FT_Glyph_StrokeBorder`.
- Rust wires these options, but lower `pillow-rs-freetype` stroker support is
  incomplete for real glyph outlines.
- Existing lower code has had fixture-specific successful paths for selected
  glyph/stroker cases.
- `FT_Stroker_ParseOutline` now follows FreeType 2.14.3
  `src/base/ftstroke.c:2067-2242` contour/tag control flow, including implied
  conic starts and conic/cubic close handling. This removes the parser-level
  two-point-line-only limitation, but it deliberately propagates errors from
  the delegated segment routes instead of hard-coding output geometry.

Decision:

- This is the highest-priority real implementation gap.
- Do not add more glyph-specific shortcuts.
- Finish general outline segment stroker geometry and export in
  `pillow-rs-freetype`.
- Then add public Pillow ImageFont rows for:
  - successful `stroke_width`;
  - successful `stroke_filled=true`;
  - descenders/ascenders such as `jQ`;
  - mono `mode="1"` with stroke;
  - edge cases where stroked glyphs clip at top/bottom/left/right.

### B. Stroked extent clamp is suspect Rust-only logic

File:

- `pillow-rs/src/font/imagingft.rs`

Current issue:

- Rust clamps `x_max`/`y_max` after rendered stroked bitmap extents are known.
- Pillow computes allocation through `_imagingft.c::bounding_box_and_anchors`
  and clips rendered writes to the target.

Decision:

- Treat this as a temporary compatibility shim, not a proven Pillow behavior.
- After general stroker parity lands, remove it unless an exact C/Pillow trace
  proves equivalent behavior.

### C. Successful libraqm shaping is not implemented

Files:

- `pillow-rs/src/font/imagingft.rs`
- `pillow-rs/src/error.rs`
- `pillow-rs/tests/support/font_runner.rs`

Current issue:

- Pillow can support successful `direction`, `features`, and `language` when
  compiled with libraqm.
- The current oracle is no-libraqm, so Pillow returns errors for those inputs.
- Rust correctly has a dedicated unsupported-libraqm error path for this scope.

Decision:

- This is an explicit product exclusion for now.
- Keep no-libraqm error parity rows.
- Do not claim full `PIL.ImageFont` parity unless successful RAQM shaping is
  implemented or permanently excluded from the product target.

### D. Bitmap `ImageFont.ImageFont` is not a 1:1 Rust type shape

Files:

- `pillow-rs/src/font/pilfont.rs`
- `pillow-rs/src/font/mod.rs`
- `pillow-rs/src/lib.rs`

Current issue:

- Pillow has `ImageFont.ImageFont` for bitmap fonts and `FreeTypeFont` for
  FreeType fonts.
- Rust uses `PilFont` for bitmap fonts and `ImageFont` for FreeType fonts.

Decision:

- Behavior parity can continue with this split.
- If API-shape parity is required, introduce/rename surfaces so Rust names
  match Pillow more directly.
- Regardless of naming, `pilfont.rs` must reach trusted region coverage through
  public Pillow rows.

### E. Path/search/stream behavior is not implemented in core

Files:

- `pillow-rs/src/lib.rs`
- `pillow-rs-py/src/lib.rs`
- `pillow-rs-js/src/lib.rs`
- `pillow-rs/tests/support/font_runner.rs`

Current issue:

- Pillow `truetype`, `load`, and `load_path` accept paths and file-like input.
- Core Rust accepts bytes and typed options.

Decision:

- This is the correct thin-binding architecture.
- Bindings may load bytes and normalize user input.
- Bindings must not implement font parsing, glyph layout, rasterization, or
  parity logic.

### F. Embedded bitmap, variation metrics, and vertical metrics are untrusted

Files:

- `pillow-rs-freetype/src/tt/sbit.rs`
- `pillow-rs-freetype/src/tt/hdmx.rs`
- `pillow-rs-freetype/src/tt/mvar.rs`
- `pillow-rs-freetype/src/tt/vhea.rs`
- `pillow-rs-freetype/src/tt/vmtx.rs`

Current issue:

- Coverage is low or zero in table paths that can affect ImageFont metrics and
  rendering.

Decision:

- Add public Pillow ImageFont rows that naturally exercise these tables, or
  explicitly mark the table out of scope.
- Do not count lower unit tests as ImageFont parity proof unless the public
  Pillow oracle row passes.

### G. Error mapping table is present but not exhaustively behavior-proven

File:

- `pillow-rs/src/font/imagingft.rs`

Current issue:

- Rust has broad FreeType error mappings, but rare rows are not reached by
  current public ImageFont fixtures.

Decision:

- Keep the table source-aligned with Pillow `_imagingft.c::geterror`.
- Add only real malformed-font or public-input rows that Pillow itself routes
  through `PIL.ImageFont` to the same error.

## 4. Action list for decision

Recommended order:

1. Finish general `pillow-rs-freetype` stroker support.
2. Prove `stroke_width` and `stroke_filled=true` through public ImageFont rows.
3. Re-evaluate and remove/prove the stroked extent clamp in `imagingft.rs`.
4. Add distinct rounding rows for `floor26`/`ceil26`: negative bearings,
   descenders, ascenders, fractional starts, and edge anchors.
5. Add public malformed-font rows for reachable FreeType errors still uncovered
   in `FT_ERROR_MESSAGES`.
6. Add public rows for embedded bitmap and variation-metric behavior.
7. Decide whether API-shape parity requires renaming/splitting Rust
   `ImageFont`/`PilFont` to mirror Pillow `ImageFont.ImageFont` and
   `FreeTypeFont`.
8. Re-run `make -C pillow-rs font-tests`.
9. Re-run Coverage MCP command
   `font-tests-coverage-with-freetype-pillow-12-2`.
10. Update this document with the new run/snapshot and remove only gaps proven
    by live Pillow oracle rows.

## Final decision summary

The implementation is currently trustworthy for the active 345 runtime-oracle
fixture rows.

It is not yet trustworthy for the full Pillow 12.2.0 `PIL.ImageFont` surface.
The biggest real implementation gap is lower FreeType stroker/stroke-border
behavior. The biggest testing/coverage gap is that lower FreeType files used by
ImageFont remain heavily undercovered by public Pillow rows.
