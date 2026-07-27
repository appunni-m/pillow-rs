# ImageFont parity gap analysis against Pillow 12.2.0

Date: 2026-07-27

Rust commit reviewed: `d5002746717b69bc06f78897fb32606e0ca577b3`

Coverage MCP run: `a2cec088-38dd-4051-8079-5e662d3b1b6a`

Coverage MCP snapshot: `2ecbaa2d-3a32-4802-bd26-42dbd340bdd4`

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
| `pillow-rs/src/font/mod.rs` | 374/374 100.00% | n/a | 78/78 100.00% | 487/487 100.00% | covered |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | one reported line gap is rustdoc, not executable |
| `pillow-rs/src/font/imagingft.rs` | 1618/1643 98.48% | 268/278 96.40% | 159/171 92.98% | 2591/2687 96.43% | real partial branch gaps remain |

## Uncovered/partial line logic analysis

Coverage MCP reports no fully uncovered executable lines in `imagingft.rs`; all remaining relevant gaps are partial branches.

| Rust line(s) | Rust logic | Pillow 12.2.0 reference | Analysis | Required action |
|---:|---|---|---|---|
| `91`, `105` | `ft_error_to_pil` hand-maps a subset of FreeType errors and falls back to `ValueError("FreeType error N")`. | `_imagingft.c:38-112` builds an `FT_ERRORS_H` table and raises `OSError` for known errors, with unknown fallback also as `OSError`. | Current fixture rows hit several errors, but the implementation is not table-equivalent to Pillow. This is a real parity risk, not only coverage noise. | Replace subset mapping with complete FreeType error table semantics or generate the table from `fontdone` constants. Add rows for unknown/error-table fallback behavior if reachable. |
| `796` | `mask_from_run_with_start` returns `Ok((w, h, canvas))`. | `_imagingft.c:1244-1249` returns image plus offset after cleanup. | Likely compiler-generated `Result`/drop branch. No visible Pillow behavior difference at this line. | Do not hack. Leave unless a safe refactor removes impossible instrumentation without changing behavior. |
| `826` | Stroked path loads each glyph with `FT_Load_Glyph(face, g, load_flags)`. | `_imagingft.c:1007-1011` does a bounds pass with `load_flags | FT_LOAD_RENDER`; `_imagingft.c:1040-1044` then loads with `load_flags`. | Rust does not follow the exact C two-pass render-bounds structure. It may still match many cases, but the path is undercovered and tied to lower stroker correctness. | Add independent stroked success/error rows after correcting implementation deltas below. |
| `827-829` | Rust applies local previous-glyph kerning in the render loop with `prev.filter(|p| *p != 0 && g != 0)`. | `_imagingft.c:1003-1025` and `1237-1238` consume `glyph_info[i].x_offset/x_advance/y_offset/y_advance` from layout. | In Pillow C, layout owns glyph positioning. Rust render code recomputes kerning locally. This can diverge for missing glyphs, zero glyph index transitions, or any future non-BASIC layout. | Refactor render loops to consume a layout run equivalent to Pillow `GlyphInfo` rather than recomputing layout inside render. Add zero-glyph and kerning-pair stroked rows. |
| `857`, `860` | Rust clamps stroked bitmap extents when actual stroked bitmap exceeds bbox-derived expected dimensions. | `_imagingft.c:998-1001` says render dimensions must match `font_getsize`; `_imagingft.c:1115-1128` clips during paste. | This looks like a workaround for bbox/stroker mismatch. Pillow allocates from `bounding_box_and_anchors`, then clips when writing; it does not mutate the computed bitmap extent this way. | Treat as suspect implementation. Fix lower bbox/stroker parity, then remove or justify clamp with exact C evidence. Add rows that prove both clamp sides if it remains. |
| `928` | Rust slices `canvas[dst..dst + cw]` after manual clipping. | `_imagingft.c:1115-1128` clips x/y before writing to target. | Likely bounds-check instrumentation. Still worth covering with partially clipped and fully clipped glyph rows because this is the exact paste boundary. | Add edge rows only if they exercise new clipping behavior. Do not add duplicate rows that do not move coverage. |
| `959` | Rust chooses `FT_RENDER_MODE_MONO` for stroked glyph bitmap conversion when `TGT_MONO` is set. | `_imagingft.c:1053-1055` always calls `FT_Glyph_To_Bitmap(..., FT_RENDER_MODE_NORMAL, ...)` on stroked glyphs. | This is a real C/Rust difference. Current fixture rows do not prove the mono-stroked branch is correct; according to Pillow 12.2.0 it should probably not exist. | Change stroked glyph rendering to always use normal render mode unless a first-divergence trace proves a different C path. |

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

### 1. Stroked render mode is wrong or at least unproven

Pillow C always converts stroked glyphs to bitmap with `FT_RENDER_MODE_NORMAL` in `_imagingft.c:1053-1055`.

Rust uses `FT_RENDER_MODE_MONO` when `load_flags & TGT_MONO != 0` in `imagingft.rs:959-963`.

Decision needed: change Rust to normal mode for stroked glyphs, then regenerate/extend rows for mode `"1"` plus `stroke_width`.

### 2. `stroke_filled` is missing from Rust public text options

Pillow `FreeTypeFont.getmask2` passes `kwargs.get("stroke_filled", False)` into the C render call in `ImageFont.py:632-644`.

Pillow C then chooses between `FT_Glyph_StrokeBorder` and `FT_Glyph_Stroke` in `_imagingft.c:1048-1051`.

Rust `ImageFontTextOptions` only records `has_kwargs`; it does not parse or carry `stroke_filled`. Current parity passes because the active rows do not make this value semantically matter. This is a real public API gap for `getmask2`.

Decision needed: add `stroke_filled: bool` to `ImageFontTextOptions`, parse it in the runner/bindings, implement `FT_Glyph_StrokeBorder` behavior in `fontdone`, and add rows where `stroke_filled=true` changes output.

### 3. Stroker parameters differ

Pillow C calls `FT_Stroker_Set(..., FT_STROKER_LINECAP_ROUND, FT_STROKER_LINEJOIN_ROUND, 0)` in `_imagingft.c:989-995`.

Rust calls `FT_Stroker_Set(..., ROUND, ROUND, 65_536)` in `imagingft.rs:949-955`.

The miter limit may not affect round joins in common cases, but it is still not exact source parity.

Decision needed: set the same value as Pillow unless traced evidence proves no behavioral difference.

### 4. Rust stroked layout/render loop does local kerning instead of consuming a Pillow-equivalent layout run

Pillow C consumes `glyph_info` generated by the layout path. It does not recompute kerning inside the render paste loop.

Rust computes `gid`, loads the glyph, applies a `prev/g != 0` kerning guard, and advances pen inside both normal and stroked render loops.

Decision needed: introduce a shared internal `GlyphRun`/`GlyphInfo` equivalent from BASIC layout and make bbox, length, mask, and stroke consume it. This reduces divergence risk and will make RAQM/no-RAQM separation explicit.

### 5. Rust extent clamping in stroked rendering is suspect

Rust clamps `x_max` and `y_max` if stroked actual extents exceed expected bbox-derived extents.

Pillow C allocates from `bounding_box_and_anchors`, then clips while writing to the target image. The C comment says render dimensions must match `font_getsize`; it does not show this Rust-style post-stroke extent mutation.

Decision needed: after stroker parity improves, remove the clamp or document the exact C-equivalent reason. Add rows that would fail if this clamp hides a real extent bug.

### 6. FreeType error mapping is not table-equivalent

Pillow `_imagingft.c` uses the FreeType error table. Rust maps a small set of errors and uses a different error class for fallback.

Decision needed: make Rust error mapping generated/table-driven and make exact `OSError` message parity part of the manifest.

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

1. Fix the obvious C/Rust stroked rendering differences:
   - render stroked glyphs as `FT_RENDER_MODE_NORMAL`;
   - match `FT_Stroker_Set` miter limit;
   - add `stroke_filled` to typed options and implement `FT_Glyph_StrokeBorder`.
2. Replace `ft_error_to_pil` with table-equivalent Pillow 12.2.0 error mapping.
3. Refactor BASIC layout into a shared run consumed by length, bbox, mask, and stroke.
4. Re-evaluate and remove the stroked extent clamps if they are only masking lower stroker/bbox issues.
5. Add minimal independent fixture rows for:
   - stroked mode `"1"`;
   - `stroke_filled=true`;
   - zero-glyph/missing-glyph kerning transitions;
   - clipped stroked bitmap paste;
   - table-mapped FreeType errors not currently represented.
6. Run `make -C pillow-rs font-tests`, then Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`.
7. Only after coverage moves, update this document with the new snapshot and remaining gaps.

## Current decision point

The safe claim today is:

> Current active Font fixture rows have 100% exact runtime parity against Pillow 12.2.0.

The unsafe claim today is:

> `PIL.ImageFont` is fully implemented with complete parity.

That second claim is not defensible until the stroked rendering, `stroke_filled`, error-table mapping, layout-run ownership, and lower `pillow-rs-freetype` coverage gaps are addressed.
