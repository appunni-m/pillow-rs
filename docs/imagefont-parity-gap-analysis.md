# ImageFont parity gap analysis against Pillow 12.2.0

Date: 2026-07-27

Rust commit reviewed: `ca7f028efe80465bc17111457d36a7950fe88afb`

Coverage MCP run: `290687f0-e29e-461a-a781-377f662f9173`

Coverage MCP snapshot: `c671fbf6-109d-42db-8a9e-57b0a57dcc25`

Suite: `font-with-freetype`

Oracle runtime:

- Python: `.oracle-venv/bin/python` using Python 3.12.13
- Pillow: 12.2.0
- Native font core: `PIL._imagingft`
- FreeType: 2.14.3

Local Pillow source used for comparison:

- `.oracle-venv/lib/python3.12/site-packages/PIL/ImageFont.py`
- Pillow 12.2.0 `_imagingft.c` reference: <https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/_imagingft.c>

## Executive status

The current live Font fixture corpus has exact runtime-oracle parity for the rows it exercises:

- 336 input-only rows execute.
- 336 rows match live Pillow 12.2.0 exactly.
- Inputs under `pillow-rs/tests/fixtures/font/inputs/public-api` do not contain stored oracle output, expected error payloads, pixel hashes, or self-comparison data.
- The oracle script fails unless the repo-local venv is Pillow 12.2.0.
- `make -C pillow-rs font-tests` passes.
- Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2` passes and ingests snapshot `c671fbf6-109d-42db-8a9e-57b0a57dcc25`.

This is still not enough to claim complete `PIL.ImageFont` parity. The safe claim is:

> Current active Font fixture rows have 100% exact runtime parity against Pillow 12.2.0.

The unsafe claim is:

> `PIL.ImageFont` is fully implemented with complete parity.

That second claim is not defensible until the gaps below are either implemented with oracle fixtures or explicitly excluded from scope.

## Pillow 12.2.0 public ImageFont surface vs Rust surface

The live Pillow oracle exposes the following ImageFont surfaces:

| Pillow surface | Pillow public methods/functions | Rust status |
|---|---|---|
| module functions | `load`, `load_default`, `load_default_imagefont`, `load_path`, `truetype` | Partially modeled. Core Rust intentionally accepts bytes, not filesystem paths. Python/JS binding I/O must stay thin and delegate after byte loading. |
| `ImageFont.ImageFont` bitmap font | `getbbox`, `getlength`, `getmask`, `info` on loaded bitmap fonts | Implemented as separate Rust `PilFont`, not as the same `ImageFont` class shape. Fixture rows exist for bitmap `ImageFont.*`. |
| `ImageFont.FreeTypeFont` | `getname`, `getmetrics`, `getlength`, `getbbox`, `getmask`, `getmask2`, `font_variant`, `get_variation_names`, `set_variation_by_name`, `get_variation_axes`, `set_variation_by_axes` | Mostly modeled through Rust `ImageFont`. BASIC layout paths are oracle-tested. Successful libraqm shaping is out of scope. `stroke_filled=true` is wired but not proven by successful fixture rows. |
| `ImageFont.TransposedFont` | `getmask`, `getbbox`, `getlength` | Not modeled as a Rust class; exposed as helper operations (`get_transposed_mask`, `transposed_bbox`, `validate_transposed_length`) and tested through fixtures. |
| enum-like values | `Layout.BASIC`, `Layout.RAQM` | BASIC implemented. RAQM success intentionally unsupported; no-libraqm behavior is tested as error parity. |

Rust has extra helper surfaces that are not direct Pillow public endpoints:

- `getbbox_binary`
- `getmask2_with_start`
- `render_text_binary`
- `text_bbox`
- `draw_text` / `render_text`
- `get_transposed_mask`
- `transposed_bbox`
- `validate_transposed_length`

These are acceptable only as test/binding adapters around Pillow behavior. They should not become independent behavior specifications.

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

Coverage snapshot: `c671fbf6-109d-42db-8a9e-57b0a57dcc25`.

| File | Lines | Branches | Functions | Regions | Status |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | covered |
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | covered |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | mostly covered; reported line gap is rustdoc on `from_pilfont_data`, but function/region gaps mean additional bitmap-font variants remain untrusted |
| `pillow-rs/src/font/imagingft.rs` | 1624/1649 98.48% | 240/248 96.77% | 163/174 93.68% | 2523/2621 96.26% | real partial branch gaps remain |

Overall snapshot totals for this suite:

- Lines: 15483/50617, 30.59%
- Branches: 2634/10716, 24.58%
- Functions: 1182/3603, 32.81%
- Regions: 22288/78390, 28.43%

The overall totals are low because the suite only targets Font behavior but the coverage artifact includes much of the workspace. For ImageFont decisions, use the file-specific rows above and the lower `pillow-rs-freetype` rows below.

## Uncovered/partial line logic analysis for `imagingft.rs`

Coverage MCP reports 14 relevant gaps in `pillow-rs/src/font/imagingft.rs`: 6 uncovered lines and 8 partial-branch lines.

| Rust line(s) | Rust logic | Pillow 12.2.0 reference | Analysis | Required action |
|---:|---|---|---|---|
| `91`, `92`, `253`, `271` | Complete FreeType 2.14.3 error message table data / table declaration. | `_imagingft.c::geterror` builds the table from FreeType `FT_ERRORS_H` and raises `OSError`; unknown table misses use `"unknown freetype error"`. | Rust is source-aligned and data-driven, but these rare rows are not reached by public ImageFont fixtures. Do not add private table unit tests as proof. | Add public fixture rows only for FreeType failures reachable through `PIL.ImageFont` inputs. |
| `515` | Successful `FT_Set_Named_Instance` return path after variation-name lookup. | `FreeTypeFont.set_variation_by_name` delegates named instance selection into `_imagingft`. | Existing rows exercise variation APIs, but this exact successful named-instance propagation is not covered. | Add one fixture using a font with a named instance that Pillow and Rust both select exactly. |
| `796` | Constant/section instrumentation around FFI helper declarations. | Not a Pillow behavior. | Coverage marks a partial branch here due LLVM segment normalization, not a meaningful behavior gap. | No product action. |
| `826`, `829` | `floor26` / `ceil26` 26.6 conversion helper branch instrumentation. | Pillow BASIC layout converts 26.6 values through `PIXEL(...)`-style rounding in `_imagingft.c`. | Partial markers mean current inputs do not hit every conversion-region shape. This is not an independent feature but can hide bbox/offset rounding differences. | Add targeted bbox/mask rows with negative bearings, fractional starts, ascenders/descenders, and kerning pairs. |
| `928` | Branch in BASIC glyph run construction around previous-glyph kerning. | `_imagingft.c::text_layout_fallback` only adds kerning when a previous glyph exists. | One kerning path remains partially unproven after the shared glyph-run refactor. | Add minimal `"AV"` / non-kerning / first-glyph rows that prove both kerning and no-kerning transitions in length, bbox, mask, and stroke. |
| `1095`, `1098`, `1100` | Rust stroked bitmap extent clamps width/height when actual stroker output exceeds bbox-derived target dimensions. | `_imagingft.c::font_render_impl` allocates from `bounding_box_and_anchors` and clips during paste; it does not show this Rust-style post-stroke extent mutation. | This is the highest-risk Rust-only compatibility shim. Width clamp executes; height clamp body is still uncovered. The need for the shim indicates lower stroker/bbox mismatch. | Fix lower stroker/bbox parity, then remove or justify the clamp with exact C-equivalent evidence. If it remains, add rows proving both width and height clamp behavior against Pillow. |
| `1194`, `1195` | `stroke_filled=true` branch routes to `FT_Outline_Glyph_StrokeBorder`. | `_imagingft.c` chooses `FT_Glyph_StrokeBorder` when `stroke_filled=true`, otherwise `FT_Glyph_Stroke`. | Rust now has a real safe wrapper and branches correctly, but active Font rows do not execute successful `stroke_filled=true`. FreeType narrow route currently has 1 runnable null/error case and 3 success routes pending. | Complete lower `FT_Stroker_ParseOutline`/border-export support for real glyph outlines, then add a successful `stroke_width + stroke_filled=true` ImageFont fixture row. |

## Other ImageFont-related files where coverage is missing

These lower-level `pillow-rs-freetype` files sit underneath `ImageFont` FreeType loading, layout, metrics, glyph loading, hinting, rasterization, and embedded bitmap handling. Full ImageFont parity must either cover these through `PIL.ImageFont` fixtures or explicitly prove they are irrelevant to the supported public surface.

| File | Lines | Branches | Functions | Regions | Parity risk |
|---|---:|---:|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 1056/8093 13.05% | 74/2045 3.62% | 90/581 15.49% | 1375/11364 12.10% | high; includes public FreeType object/lifetime/stroker wrappers under ImageFont |
| `pillow-rs-freetype/src/api.rs` | 208/1186 17.54% | 35/294 11.90% | 25/105 23.81% | 275/1737 15.83% | high |
| `pillow-rs-freetype/src/font.rs` | 1260/4747 26.54% | 153/702 21.79% | 118/392 30.10% | 1725/6728 25.64% | high; font load/face/glyph machinery |
| `pillow-rs-freetype/src/render.rs` | 965/2459 39.24% | 157/486 32.30% | 76/158 48.10% | 1343/3432 39.13% | high; raster output parity |
| `pillow-rs-freetype/src/scaler.rs` | 806/1342 60.06% | 114/186 61.29% | 40/66 60.61% | 918/1436 63.93% | medium/high; scaling and hinted metrics |
| `pillow-rs-freetype/src/grays.rs` | 571/827 69.04% | 122/190 64.21% | 25/35 71.43% | 854/1106 77.22% | medium; antialias rasterizer |
| `pillow-rs-freetype/src/tt/sbit.rs` | 100/814 12.29% | 13/72 18.06% | 13/108 12.04% | 186/1269 14.66% | high for embedded bitmap/color fonts |
| `pillow-rs-freetype/src/tt/cmap.rs` | 271/809 33.50% | 39/174 22.41% | 10/58 17.24% | 395/1089 36.27% | high for charmap/input encoding |
| `pillow-rs-freetype/src/tt/glyf.rs` | 174/545 31.93% | 34/96 35.42% | 8/20 40.00% | 219/694 31.56% | high for TrueType outlines |
| `pillow-rs-freetype/src/tt/cff.rs` | 355/735 48.30% | 37/112 33.04% | 29/81 35.80% | 507/1087 46.64% | high for CFF/OpenType |
| `pillow-rs-freetype/src/tt/hinter/exec.rs` | 722/1489 48.49% | 146/476 30.67% | 32/48 66.67% | 1296/3103 41.77% | high for hinted TrueType |
| `pillow-rs-freetype/src/autohint/latin.rs` | 1988/2962 67.12% | 673/1263 53.29% | 45/67 67.16% | 2806/4283 65.51% | medium/high |
| `pillow-rs-freetype/src/autohint/cjk.rs` | 396/879 45.05% | 130/398 32.66% | 11/18 61.11% | 531/1180 45.00% | high for CJK fonts |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 0/42 0.00% | 0/12 0.00% | 0/2 0.00% | 0/67 0.00% | unproven horizontal device metrics |
| `pillow-rs-freetype/src/tt/mvar.rs` | 0/67 0.00% | 0/6 0.00% | 0/7 0.00% | 0/113 0.00% | unproven variation metrics |
| `pillow-rs-freetype/src/tt/vhea.rs` | 0/11 0.00% | 0/2 0.00% | 0/1 0.00% | 0/9 0.00% | unproven vertical metrics |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 0/50 0.00% | 0/8 0.00% | 0/2 0.00% | 0/65 0.00% | unproven vertical metrics |

## Implementation differences or unproven behavior against Pillow 12.2.0

### 1. Successful libraqm shaping is intentionally not implemented

Pillow exposes `direction`, `features`, and `language` on `FreeTypeFont.getlength`, `getbbox`, `getmask`, and `getmask2`. Those successful shaping paths require libraqm.

Rust currently treats successful libraqm shaping as out of scope and uses a dedicated `PilError::UnsupportedLibraqm` internally. The parity harness maps this to Pillow's no-libraqm error category/message. This is correct only for the no-libraqm environment.

Decision: do not claim complete `PIL.ImageFont` parity while successful RAQM shaping is excluded.

### 2. `stroke_filled=true` is wired but not proven

Pillow `FreeTypeFont.getmask2` accepts `stroke_filled` through keyword arguments and passes it into the C render path. `_imagingft.c` chooses `FT_Glyph_StrokeBorder` when `stroke_filled=true`.

Rust now carries `stroke_filled` in `ImageFontTextOptions` and routes to `fontdone::ffi::FT_Outline_Glyph_StrokeBorder`. That removed the old explicit unsupported guard, but the lower `fontdone` stroke-border geometry for real glyph outlines is still incomplete.

Decision: complete `FT_Stroker_ParseOutline`/border-export for real glyphs, then add successful `stroke_filled=true` fixture rows. Until then, this is a known parity gap.

### 3. Stroked extent clamping is suspect Rust-only logic

Rust clamps stroked `x_max`/`y_max` when actual bitmap extents exceed bbox-derived dimensions.

Pillow allocates the target from `bounding_box_and_anchors` and clips while writing pixels. The current evidence does not show Pillow mutating the computed extent the way Rust does.

Decision: treat this as a compatibility shim, not trusted parity. After lower stroker/bbox parity improves, remove it or prove it with an exact C-equivalent trace.

### 4. BASIC layout is shared and mostly source-aligned

Pillow C lays out glyphs once and rendering consumes the resulting glyph info. Rust now builds a shared BASIC `GlyphRun` for length, bbox, mask, and stroke.

Remaining risk: fixtures need more independent kerning/no-kerning and missing-glyph transitions so shared-run parity is not only proven by duplicate easy rows.

### 5. Error mapping is now table-equivalent but not exhaustively reached

Rust maps FreeType 2.14.3 errors through a full table and returns `PilError::OsError`, matching Pillow's broad `OSError` behavior.

Remaining risk: rare FreeType errors are present as table data but not reachable through current public ImageFont fixtures. They should only be added if a real Pillow input can trigger them.

### 6. Bitmap and FreeType class shape is not 1:1

Pillow has `ImageFont.ImageFont` for bitmap fonts and `ImageFont.FreeTypeFont` for FreeType fonts. Rust currently uses `PilFont` for bitmap and `ImageFont` for FreeType.

Decision: decide whether public Rust naming should mirror Pillow more closely. The current split is testable but not class-shape parity.

### 7. Path/stream behavior is binding-owned, not core-owned

Pillow module functions accept paths and streams. Core Rust accepts bytes and options.

Decision: keep filesystem I/O outside core, but ensure binding crates remain thin and do not reimplement parsing/layout/rendering logic.

### 8. Embedded bitmap, vertical metrics, and device metrics are untrusted

Coverage shows weak or zero coverage for `sbit`, `vhea`, `vmtx`, `hdmx`, and `mvar`.

Decision: add ImageFont oracle rows with fonts that exercise embedded bitmap glyphs, vertical/TTB metrics if/when libraqm enters scope, horizontal device metrics, and variation metric deltas. If a feature is not in supported scope, record the explicit exclusion instead of leaving it ambiguous.

## Recommended action order

1. Finish `FT_Stroker_ParseOutline`/border-export support for real glyph outlines so `stroke_filled=true` can pass against Pillow.
2. Add minimal, independent oracle fixtures for:
   - successful `stroke_width + stroke_filled=true`;
   - stroked mode `"1"`;
   - height-side stroked clipping;
   - kerning and no-kerning transitions across `getlength`, `getbbox`, `getmask`, and stroke;
   - successful named-instance `set_variation_by_name`;
   - embedded bitmap glyph path;
   - reachable FreeType table errors.
3. Re-run `make -C pillow-rs font-tests`.
4. Re-run Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`.
5. Update this document with the new run/snapshot and remove only gaps proven by live Pillow oracle rows.

## Current decision point

The current implementation is good enough to trust the active 336-row Font fixture corpus.

It is not yet good enough to declare full `PIL.ImageFont` parity across Pillow 12.2.0. The biggest action decision is whether to prioritize real `FT_Glyph_StrokeBorder`/stroker geometry first, because that is the clearest concrete mismatch between Pillow public behavior and Rust implementation.
