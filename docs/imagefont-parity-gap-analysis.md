# ImageFont parity gap analysis against Pillow 12.2.0

Date: 2026-07-27

Rust commit reviewed: `4eab9da2b4d8fa0f90d47f3b54bd2d0b9589728e`

Coverage MCP run: `7b697179-bbd6-4a45-bc39-67ba09cb82ad`

Coverage MCP snapshot: `24a389d0-d924-4b51-b40d-30c5263bcb4e`

Suite: `font-with-freetype`

Oracle runtime:

- Python: `.oracle-venv/bin/python` using Python 3.12.13
- Pillow: 12.2.0
- Native font core: `PIL._imagingft`
- FreeType: 2.14.3

Pillow source references:

- `PIL/ImageFont.py`: <https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/ImageFont.py>
- `_imagingft.c`: <https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imagingft.c>

## Executive status

The current live Font fixture corpus has exact runtime-oracle parity:

- 336 input-only rows execute.
- 336 rows match live Pillow 12.2.0 exactly.
- Inputs do not contain output, error expectation, pixel hash, oracle result, or stored expected payload.
- The oracle script now fails unless the repo-local venv is Pillow 12.2.0.
- Local verification command `make -C pillow-rs font-tests` passes.
- Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2` passes and ingests the coverage artifact.

This is not enough to claim full `PIL.ImageFont` parity. It proves only the current fixture rows. Coverage and source comparison still show unproven and likely-wrong areas, especially stroked FreeType rendering, error mapping, and lower `pillow-rs-freetype` engine branches.

## Pillow 12.2.0 public ImageFont surface

The live oracle reports these behavioral endpoints:

| Pillow surface | Public methods/functions |
|---|---|
| module functions | `load`, `load_default`, `load_default_imagefont`, `load_path`, `truetype` |
| `ImageFont.ImageFont` | `getbbox`, `getlength`, `getmask`, plus `info` data on loaded bitmap fonts |
| `ImageFont.FreeTypeFont` | `getname`, `getmetrics`, `getlength`, `getbbox`, `getmask`, `getmask2`, `font_variant`, `get_variation_names`, `set_variation_by_name`, `get_variation_axes`, `set_variation_by_axes` |
| `ImageFont.TransposedFont` | `getmask`, `getbbox`, `getlength` |
| enum-like public values | `Layout.BASIC`, `Layout.RAQM` |

The current Rust public model does not mirror the Python class model exactly:

- `pillow_rs::ImageFont` is the FreeType-backed font type.
- `pillow_rs::PilFont` is the bitmap `.pil` font type corresponding to Pillow's base `ImageFont.ImageFont`.
- `TransposedFont` is not a Rust class; Rust exposes helper functions such as `transposed_bbox`, `validate_transposed_length`, and `imagefont_get_transposed_mask`.
- Several Rust functions are test/helper surfaces, not direct Pillow endpoints: `getbbox_binary`, `getmask2_with_start`, `render_text_binary`, `text_bbox`, `draw_text`, `get_transposed_mask`, `transposed_bbox`, and `validate_transposed_length`.

This split is acceptable for the current harness, but it is not a 1:1 Rust class model for `PIL.ImageFont`.

## Live fixture corpus

Current active input files under `pillow-rs/tests/fixtures/font/inputs/public-api`:

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

## Direct `pillow-rs/src/font` coverage status

| File | Lines | Branches | Functions | Regions | Status |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | covered |
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | covered |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | one reported line gap is rustdoc, not executable |
| `pillow-rs/src/font/imagingft.rs` | 1624/1651 98.36% | 240/248 96.77% | 163/174 93.68% | 2523/2618 96.37% | real partial branch gaps remain |

## Uncovered/partial line logic analysis

Coverage MCP reports five uncovered source lines in `imagingft.rs`. Three are rare FreeType error table data rows, one is variation named-instance success propagation, and one is the stroked extent-clamp body. The rest of the targeted gaps are partial branches.

| Rust line(s) | Rust logic | Pillow 12.2.0 reference | Analysis | Required action |
|---:|---|---|---|---|
| `91`, `92`, `253`, `271` | Complete FreeType 2.14.3 error message table data. | `_imagingft.c:38-112` builds an `FT_ERRORS_H` table and raises `OSError` for known errors, with unknown fallback also as `OSError`. | Rust now uses a complete table and always returns `PilError::OsError`; remaining uncovered rows mean those rare error codes are not triggered by the current public ImageFont corpus. | Add public fixture rows only when the corresponding FreeType failures are reachable through `PIL.ImageFont`; do not unit-test private table rows as a parity substitute. |
| `515` | Successful `FT_Set_Named_Instance` after variation-name lookup. | Pillow `FreeTypeFont.set_variation_by_name` selects the named instance through `_imagingft`. | Existing variation rows cover name lookup and post-variation behavior, but this exact success propagation line is not covered. | Add a minimal successful named-instance row if a fixture font exposes a named instance that Pillow and Rust can both select exactly. |
| `796-797` | `stroke_filled` unsupported guard before stroked rendering. | `_imagingft.c:1048-1051` routes `stroke_filled=true` to `FT_Glyph_StrokeBorder`. | Rust now makes the unsupported path explicit instead of silently treating it as default stroke. This is not full Pillow parity; it is a guard against false-positive parity. | Implement `FT_Glyph_StrokeBorder` in `fontdone`, then add a `stroke_width + stroke_filled=true` fixture row that must pass against the live oracle. |
| `826`, `829` | `ceil().max(0.0)` dimensions for the stroke-expanded bbox. | Pillow computes dimensions through `bounding_box_and_anchors` and C integer conversions. | The negative max branch is partially untested after the shared run refactor. This is dimension sanitization, not a separate public feature. | Cover only with an input that moves a real Pillow branch, not by adding duplicate stroke rows. |
| `846`, `849` | Rust clamps stroked bitmap extents when actual stroked bitmap exceeds bbox-derived expected dimensions. | `_imagingft.c:998-1001` says render dimensions must match `font_getsize`; `_imagingft.c:1115-1128` clips during paste. | This looks like a workaround for bbox/stroker mismatch. Pillow allocates from `bounding_box_and_anchors`, then clips when writing; it does not mutate the computed bitmap extent this way. | Treat as suspect implementation. Fix lower bbox/stroker parity, then remove or justify clamp with exact C evidence. Add rows that prove both clamp sides if it remains. |
| `928`, `929` | Rust sets `FT_STROKER_LINEJOIN_ROUND` and miter limit `0` before glyph stroking. | `_imagingft.c:989-995` uses the same line cap, line join, and miter limit. | Source parity is now aligned; the coverage marker reflects constant/argument instrumentation, not a known behavior gap. | No action unless a lower-level stroker fixture shows these values are not honored. |

## Other ImageFont-related files where coverage is missing

The following lower-level `pillow-rs-freetype` files are part of the FreeType-backed `ImageFont` behavior path and still have missing coverage in the Font parity suite.

| File | Lines | Branches | Functions | Regions | Parity risk |
|---|---:|---:|---:|---:|---|
| `pillow-rs-freetype/src/api.rs` | 208/1186 17.54% | 35/294 11.90% | 25/105 23.81% | 275/1737 15.83% | high |
| `pillow-rs-freetype/src/ffi/handles.rs` | 1056/8049 13.12% | 74/2035 3.64% | 90/580 15.52% | 1375/11315 12.15% | high |
| `pillow-rs-freetype/src/font.rs` | 1260/4747 26.54% | 153/702 21.79% | 118/392 30.10% | 1725/6728 25.64% | high |
| `pillow-rs-freetype/src/render.rs` | 965/2459 39.24% | 157/486 32.30% | 76/158 48.10% | 1343/3432 39.13% | high |
| `pillow-rs-freetype/src/scaler.rs` | 806/1342 60.06% | 114/186 61.29% | 40/66 60.61% | 918/1436 63.93% | medium/high |
| `pillow-rs-freetype/src/grays.rs` | 571/827 69.04% | 122/190 64.21% | 25/35 71.43% | 854/1106 77.22% | medium |
| `pillow-rs-freetype/src/tt/sbit.rs` | 100/814 12.29% | 13/72 18.06% | 13/108 12.04% | 186/1269 14.66% | high for embedded bitmap/color fonts |
| `pillow-rs-freetype/src/tt/cmap.rs` | 271/809 33.50% | 39/174 22.41% | 10/58 17.24% | 395/1089 36.27% | high for charmap/input encoding |
| `pillow-rs-freetype/src/tt/glyf.rs` | 174/545 31.93% | 34/96 35.42% | 8/20 40.00% | 219/694 31.56% | high for outline glyphs |
| `pillow-rs-freetype/src/tt/cff.rs` | 355/735 48.30% | 37/112 33.04% | 29/81 35.80% | 507/1087 46.64% | high for CFF/OpenType |
| `pillow-rs-freetype/src/tt/hinter/exec.rs` | 722/1489 48.49% | 146/476 30.67% | 32/48 66.67% | 1296/3103 41.77% | high for hinted TrueType |
| `pillow-rs-freetype/src/autohint/latin.rs` | 1988/2962 67.12% | 673/1263 53.29% | 45/67 67.16% | 2806/4283 65.51% | medium/high |
| `pillow-rs-freetype/src/autohint/cjk.rs` | 396/879 45.05% | 130/398 32.66% | 11/18 61.11% | 531/1180 45.00% | high for CJK fonts |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 0/42 0.00% | 0/12 0.00% | 0/2 0.00% | 0/67 0.00% | unproven |
| `pillow-rs-freetype/src/tt/mvar.rs` | 0/67 0.00% | 0/6 0.00% | 0/7 0.00% | 0/113 0.00% | unproven variation metrics |
| `pillow-rs-freetype/src/tt/vhea.rs` | 0/11 0.00% | 0/2 0.00% | 0/1 0.00% | 0/9 0.00% | unproven vertical metrics |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 0/50 0.00% | 0/8 0.00% | 0/2 0.00% | 0/65 0.00% | unproven vertical metrics |

These are not all direct `PIL.ImageFont` public methods, but they are underneath `FreeTypeFont` loading, layout, metrics, glyph loading, hinting, rasterization, and embedded bitmap handling. Any full ImageFont parity claim must either cover these via `PIL.ImageFont` fixtures or explicitly prove they are irrelevant to the supported public surface.

## Implementation differences or unproven behavior against Pillow 12.2.0

### 1. Stroked render mode is now aligned

Pillow C always converts stroked glyphs to bitmap with `FT_RENDER_MODE_NORMAL` in `_imagingft.c:1053-1055`.

Rust now always uses `FT_RENDER_MODE_NORMAL` for stroked glyph bitmap conversion, including `mode="1"`.

Remaining action: keep/add fixture rows for mode `"1"` plus `stroke_width` so this behavior remains protected by the runtime Pillow oracle.

### 2. `stroke_filled` is explicit but still unsupported for successful rendering

Pillow `FreeTypeFont.getmask2` passes `kwargs.get("stroke_filled", False)` into the C render call in `ImageFont.py:632-644`.

Pillow C then chooses between `FT_Glyph_StrokeBorder` and `FT_Glyph_Stroke` in `_imagingft.c:1048-1051`.

Rust `ImageFontTextOptions` now carries `stroke_filled` explicitly. The adapter refuses `stroke_width != 0 && stroke_filled=true` with `NotImplementedError` instead of silently treating it as the default `FT_Glyph_Stroke` path. Current parity passes because the active rows do not require successful `FT_Glyph_StrokeBorder` rendering.

Decision needed: implement `FT_Glyph_StrokeBorder` behavior in `fontdone`, then add rows where `stroke_filled=true` changes output and remove the explicit unsupported guard.

### 3. Stroker miter parameter is now aligned

Pillow C calls `FT_Stroker_Set(..., FT_STROKER_LINECAP_ROUND, FT_STROKER_LINEJOIN_ROUND, 0)` in `_imagingft.c:989-995`.

Rust now passes miter limit `0` for the stroked ImageFont path.

Remaining action: still implement `stroke_filled`/`FT_Glyph_StrokeBorder` parity; that is the larger stroked-rendering gap.

### 4. BASIC layout is now shared by length, bbox, mask, and stroke

Pillow C consumes `glyph_info` generated by the layout path. It does not recompute kerning inside the render paste loop.

Rust now builds one BASIC `GlyphRun` carrying glyph index, pen, advance, and cbox. Length, bbox, normal mask, and stroked mask consume that run instead of duplicating kerning and pen advancement in render loops.

Remaining action: add zero-glyph/missing-glyph and kerning-pair stroked rows so the shared-run behavior is protected by oracle fixtures.

### 5. Rust extent clamping in stroked rendering is suspect

Rust clamps `x_max` and `y_max` if stroked actual extents exceed expected bbox-derived extents.

Pillow C allocates from `bounding_box_and_anchors`, then clips while writing to the target image. The C comment says render dimensions must match `font_getsize`; it does not show this Rust-style post-stroke extent mutation.

Decision needed: after stroker parity improves, remove the clamp or document the exact C-equivalent reason. Add rows that would fail if this clamp hides a real extent bug.

### 6. FreeType error mapping is now table-equivalent

Pillow `_imagingft.c` uses the FreeType error table. Rust maps a small set of errors and uses a different error class for fallback.

Rust now uses a complete FreeType 2.14.3 error-message table derived from `fterrdef.h`, always returns `PilError::OsError`, and uses Pillow's `"unknown freetype error"` fallback for table misses.

Remaining action: add public fixture rows only for FreeType errors that are reachable through `PIL.ImageFont` inputs. Do not add private unit tests for the table as a parity substitute.

### 7. Libraqm successful shaping is intentionally not implemented

The current manifest says successful libraqm shaping is out of scope. Direction/features/language rows are only trusted for no-libraqm error behavior.

Current action: keep this explicit. Rust core now uses a dedicated `PilError::UnsupportedLibraqm` for direction/features/language paths. Python/JS/test parity mapping still exposes this as Pillow's no-libraqm `KeyError` category with the same message, so the unsupported path is explicit internally without weakening oracle parity.

Decision needed: do not claim full `PIL.ImageFont` parity while successful libraqm shaping is excluded.

### 8. Bitmap `ImageFont` and FreeType `FreeTypeFont` are split into different Rust types

Pillow exposes both through `PIL.ImageFont`. Rust exposes FreeType behavior as `ImageFont` and bitmap font behavior as `PilFont`.

Decision needed: decide whether the public Rust API should model the Python module more directly, for example with an enum/wrapper or separate `BitmapImageFont`/`FreeTypeImageFont` naming. The current setup is testable, but not class-shape parity.

### 9. Path-based loading is not core-owned

Pillow module functions accept paths and binary streams. Core Rust intentionally takes bytes; the test runner reads fixture files before calling Rust.

Decision needed: keep file/path I/O outside core, but ensure Python/JS bindings remain thin and expose Pillow-compatible path/bytes behavior by delegating all logic after I/O to Rust.

## Recommended action order

1. Implement `FT_Glyph_StrokeBorder` and remove the `stroke_filled` unsupported guard.
2. Re-evaluate and remove the stroked extent clamps if they are only masking lower stroker/bbox issues.
3. Add minimal independent fixture rows for:
   - stroked mode `"1"`;
   - `stroke_filled=true`;
   - zero-glyph/missing-glyph kerning transitions;
   - clipped stroked bitmap paste;
   - table-mapped FreeType errors not currently represented.
4. Run `make -C pillow-rs font-tests`, then Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`.
5. Only after coverage moves, update this document with the new snapshot and remaining gaps.

## Current decision point

The safe claim today is:

> Current active Font fixture rows have 100% exact runtime parity against Pillow 12.2.0.

The unsafe claim today is:

> `PIL.ImageFont` is fully implemented with complete parity.

That second claim is not defensible until successful `stroke_filled` rendering, error-table mapping, stroked extent clamping, and lower `pillow-rs-freetype` coverage gaps are addressed.
