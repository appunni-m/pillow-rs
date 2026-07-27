# ImageFont parity gap analysis against Pillow 12.2.0

Date: 2026-07-27

Rust commit reviewed: `21086af6f5fff5921b554e3b6fe76d6613b5874d`

Coverage MCP run: `126a382e-f67f-4f04-9422-6033145acceb`

Coverage MCP snapshot: `e67116f1-f510-46ba-80a0-23768d214d3a`

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

- 352 input-only rows execute.
- 352 rows match live Pillow 12.2.0 exactly.
- Inputs under `pillow-rs/tests/fixtures/font/inputs/public-api` do not contain stored oracle output, expected error payloads, pixel hashes, or self-comparison data.
- The oracle script fails unless the repo-local venv is Pillow 12.2.0.
- `make -C pillow-rs font-tests` passes.
- Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2` passes and ingests snapshot `e67116f1-f510-46ba-80a0-23768d214d3a`.
- Direction/features/language rows now prove two things separately: Rust core returns the dedicated `PilError::UnsupportedLibraqm` variant, and the public parity payload still matches Pillow's no-libraqm `KeyError`.
- Missing horizontal metrics rows now prove the lower `fontdone` error conversion maps `FontError::InvalidFont("missing 'hmtx' table")` to `FT_Err_Hmtx_Table_Missing`, producing Pillow's public `OSError("horizontal metrics (hmtx) table missing")` instead of the old generic `OSError("broken file")`.
- Additional metric rows for fixed-width and hhea-zero/no-OS2 fallback fonts now prove `FreeTypeFont.getmetrics()` parity for two more lower metrics-table shapes.
- Additional mono BASIC rows for `AV` and `jQ` now prove live-oracle parity for normal-vs-mono load-flag behavior across `getlength`, `getbbox`, `getmask`, and `getmask2`. Coverage MCP shows these rows are semantically useful but do not reduce the remaining LLVM-reported `imagingft.rs` region gaps; the next coverage-moving gap is still lower stroker/stroke-border implementation.
- Commit `fd0bb7ccafd8968031e962c1f3e12c5102a5e5f0` moves `FT_Stroker_ParseOutline` from a two-point-line-only parser to the FreeType 2.14.3 contour/tag control flow that delegates line, conic, and cubic segments to the existing segment routes. This is architectural progress for the stroke blocker, but it does not yet move public ImageFont coverage because the mixed-outline route and general segment stroker/export behavior remain pending.
- Commit `13c410dc64fa93576f87377e2c8dde8f671f7ca9` adds three public ImageFont rows for lower metric-table paths: `hdmx_observable` through `getlength`, `mvar_vertical_metrics` through `getmetrics`, and `vertical_vhea_only` through `getmetrics`. These rows move lower `hdmx`, `mvar`, `vhea`, and `vmtx` from 0% to live Pillow-backed coverage without changing `imagingft.rs` region gaps.
- Commit `2e45e4e4dec60bdfca5df2a7a17640f67a0037c7` adds two public ImageFont rows: `font.getbbox.hhea_descender_only_av` and `font.getlength.hinter_too_many_instruction_defs`. It also fixes lower TrueType IDEF opcode-overflow classification so Pillow's public `OSError("too many instruction definitions")` matches Rust. Coverage moved lower `tt/hinter/exec.rs` but did not change direct `imagingft.rs` region totals because LLVM still attributes the static `FT_ERROR_MESSAGES` table line as uncovered.
- Commit `21086af6f5fff5921b554e3b6fe76d6613b5874d` replaces false SBIT `"A"` rows with private-use glyph rows that actually hit embedded bitmap strikes, fixes bitmap glyph layout bbox calculation in `imagingft.rs`, and expands SBIT pixel modes (`GRAY2`, `GRAY4`, `BGRA`) to Pillow-compatible coverage bytes. This moves lower `tt/sbit.rs` coverage from 100/814 lines and 186/1269 regions to 254/814 lines and 375/1269 regions.

This is still not enough to claim complete `PIL.ImageFont` parity. The safe claim is:

> Current active Font fixture rows have 100% exact runtime parity against Pillow 12.2.0.

The unsafe claim is:

> `PIL.ImageFont` is fully implemented with complete parity.

That second claim is not defensible until the gaps below are either implemented with oracle fixtures or explicitly excluded from scope.

## Source ownership boundary

Implementation ownership must follow the original C/Python source boundary:

- FreeType-original behavior belongs in `pillow-rs-freetype`: font tables,
  glyph loading, SBIT, cmap, metrics, hinting, rasterization, stroker geometry,
  FreeType object ownership, and FreeType error-code classification.
- Pillow `_imagingft.c` behavior belongs in `pillow-rs/src/font/imagingft.rs`:
  `FreeTypeFont` adapter arguments, calls into the FreeType-shaped lower API,
  Pillow-visible bbox/mask/getmask2 result shape, offsets, mode conversion, and
  Pillow exception mapping.
- Pillow `ImageFont.py` behavior belongs in the Rust public Font facade, the
  live-oracle tests, and thin host bindings: defaults, wrapper method shape,
  path/stream-to-bytes conversion, and delegation into Rust core.

Any implementation that moves FreeType table/glyph/stroker logic into
`imagingft.rs`, or moves Pillow `_imagingft.c` public adapter behavior into
`pillow-rs-freetype`, should be treated as a design bug unless the code is only
bridging a FreeType-like lower slot into the Pillow public result shape.

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
| `font.getbbox.json` | 32 |
| `font.getbbox_binary.json` | 9 |
| `font.getlength.json` | 22 |
| `font.getmask.json` | 37 |
| `font.getmask2.json` | 44 |
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
| total | 352 |

## Direct `pillow-rs/src/font` coverage status

Coverage snapshot: `e67116f1-f510-46ba-80a0-23768d214d3a`.

| File | Lines | Branches | Functions | Regions | Status |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | covered |
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | covered |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | mostly covered; reported line gap is rustdoc on `from_pilfont_data`, but function/region gaps mean additional bitmap-font variants remain untrusted |
| `pillow-rs/src/font/imagingft.rs` | 1686/1712 98.48% | 252/262 96.18% | 165/176 93.75% | 2618/2718 96.32% | real partial branch gaps remain |

Overall snapshot totals for this suite:

- Lines: 15977/50779, 31.46%
- Branches: 2691/10766, 25.00%
- Functions: 1215/3610, 33.66%
- Regions: 22968/78626, 29.21%

The overall totals are low because the suite only targets Font behavior but the coverage artifact includes much of the workspace. For ImageFont decisions, use the file-specific rows above and the lower `pillow-rs-freetype` rows below.

## Uncovered/partial line logic analysis for `imagingft.rs`

Coverage MCP reports 13 relevant gaps in `pillow-rs/src/font/imagingft.rs`: 5 uncovered lines and 8 partial-branch lines. The previous `FT_Set_Named_Instance` success-propagation gap was resolved by making the returned FreeType status explicit before `check_ft_error(status)?`; the existing public named-instance fixture rows now cover that flow.

| Rust line(s) | Rust logic | Pillow 12.2.0 reference | Analysis | Required action |
|---:|---|---|---|---|
| `91`, `92`, `253`, `271` | Complete FreeType 2.14.3 error message table data / table declaration. | `_imagingft.c::geterror` builds the table from FreeType `FT_ERRORS_H` and raises `OSError`; unknown table misses use `"unknown freetype error"`. | Rust is source-aligned and data-driven. The new `font.getlength.hinter_too_many_instruction_defs` row proves the public Pillow/Rust error payload for `FT_Err_Too_Many_Instruction_Defs`, but LLVM still reports static table line `253` as uncovered because the table data itself is not attributed as executed. | Treat line `253` as behaviorally proven through public `ImageFont` error parity but still LLVM-uncovered. Add future rows only for real remaining public errors such as `Invalid_Horiz_Metrics` or table-miss behavior. |
| `796` | Constant/section instrumentation around FFI helper declarations. | Not a Pillow behavior. | Coverage marks a partial branch here due LLVM segment normalization, not a meaningful behavior gap. | No product action. |
| `826`, `829` | `floor26` / `ceil26` 26.6 conversion helper branch instrumentation. | Pillow BASIC layout converts 26.6 values through `PIXEL(...)`-style rounding in `_imagingft.c`. | Partial markers mean current inputs do not hit every conversion-region shape. This is not an independent feature but can hide bbox/offset rounding differences. | Add targeted bbox/mask rows with negative bearings, fractional starts, ascenders/descenders, and kerning pairs. |
| `928` | Branch in BASIC glyph run construction around previous-glyph kerning. | `_imagingft.c::text_layout_fallback` only adds kerning when a previous glyph exists. | Additional `mode="1"` rows for `AV` and `jQ` now prove public mono load-flag parity across length, bbox, mask, and mask2, but Coverage MCP still reports this line as partial. The remaining marker is therefore not removable by duplicate mono fixture expansion. | Keep this as a coverage artifact/branch-marker gap unless source-context evidence identifies a distinct public input. Do not add more duplicate BASIC rows only to chase this line. |
| `1094`, `1097`, `1099` | Rust stroked bitmap extent clamps width/height when actual stroker output exceeds bbox-derived target dimensions. | `_imagingft.c::font_render_impl` allocates from `bounding_box_and_anchors` and clips during paste; it does not show this Rust-style post-stroke extent mutation. | This is the highest-risk Rust-only compatibility shim. Width clamp executes; height clamp body is still uncovered. The need for the shim indicates lower stroker/bbox mismatch. A sweep row using stroked `jQ` was not committed because Pillow succeeds while Rust fails earlier with `FT_Err_Unimplemented_Feature`; the lower `fontdone` `FT_Outline_Glyph_Stroke` path is not generally implemented yet. | Finish general `FT_Stroker_ParseOutline` segment routes and export support. Then re-add independent stroked descender rows such as `jQ` and prove both width and height clipping against Pillow. |
| `1193`, `1194` | `stroke_filled=true` branch routes to `FT_Outline_Glyph_StrokeBorder`. | `_imagingft.c` chooses `FT_Glyph_StrokeBorder` when `stroke_filled=true`, otherwise `FT_Glyph_Stroke`. | Rust now has a real safe wrapper and branches correctly, but active Font rows do not execute successful `stroke_filled=true`. FreeType narrow route still has the stroke-border success routes pending. This shares the same lower blocker as normal stroked descender glyphs: real outline stroke geometry/export is not implemented generally. | Complete lower `FT_Stroker_ParseOutline` segment routes and border-export support for real glyph outlines, then add successful `stroke_width + stroke_filled=true` and stroked descender ImageFont fixture rows. |

## Other ImageFont-related files where coverage is missing

These lower-level `pillow-rs-freetype` files sit underneath `ImageFont` FreeType loading, layout, metrics, glyph loading, hinting, rasterization, and embedded bitmap handling. Full ImageFont parity must either cover these through `PIL.ImageFont` fixtures or explicitly prove they are irrelevant to the supported public surface.

| File | Lines | Branches | Functions | Regions | Parity risk |
|---|---:|---:|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 1056/8093 13.05% | 74/2045 3.62% | 90/581 15.49% | 1375/11364 12.10% | high; includes public FreeType object/lifetime/stroker wrappers under ImageFont |
| `pillow-rs-freetype/src/api.rs` | 263/1186 22.18% | 37/294 12.59% | 28/105 26.67% | 327/1737 18.83% | high |
| `pillow-rs-freetype/src/font.rs` | 1298/4747 27.34% | 166/702 23.65% | 127/392 32.40% | 1794/6728 26.66% | high; font load/face/glyph machinery |
| `pillow-rs-freetype/src/render.rs` | 965/2459 39.24% | 157/486 32.30% | 76/158 48.10% | 1343/3432 39.13% | high; raster output parity |
| `pillow-rs-freetype/src/scaler.rs` | 806/1342 60.06% | 114/186 61.29% | 40/66 60.61% | 918/1436 63.93% | medium/high; scaling and hinted metrics |
| `pillow-rs-freetype/src/grays.rs` | 571/827 69.04% | 122/190 64.21% | 25/35 71.43% | 854/1106 77.22% | medium; antialias rasterizer |
| `pillow-rs-freetype/src/tt/sbit.rs` | 254/814 31.20% | 21/72 29.17% | 19/108 17.59% | 375/1269 29.55% | improved by active mono/gray/gray2/gray4/BGRA public rows; still high for uncovered SBIT formats and malformed paths |
| `pillow-rs-freetype/src/tt/cmap.rs` | 271/809 33.50% | 39/174 22.41% | 10/58 17.24% | 395/1089 36.27% | high for charmap/input encoding |
| `pillow-rs-freetype/src/tt/glyf.rs` | 174/545 31.93% | 34/96 35.42% | 8/20 40.00% | 219/694 31.56% | high for TrueType outlines |
| `pillow-rs-freetype/src/tt/cff.rs` | 355/735 48.30% | 37/112 33.04% | 29/81 35.80% | 507/1087 46.64% | high for CFF/OpenType |
| `pillow-rs-freetype/src/tt/hinter/exec.rs` | 722/1489 48.49% | 146/476 30.67% | 32/48 66.67% | 1296/3103 41.77% | high for hinted TrueType |
| `pillow-rs-freetype/src/autohint/latin.rs` | 1988/2962 67.12% | 673/1263 53.29% | 45/67 67.16% | 2806/4283 65.51% | medium/high |
| `pillow-rs-freetype/src/autohint/cjk.rs` | 396/879 45.05% | 130/398 32.66% | 11/18 61.11% | 531/1180 45.00% | high for CJK fonts |
| `pillow-rs-freetype/src/tt/hdmx.rs` | 26/42 61.90% | 6/12 50.00% | 1/2 50.00% | 44/67 65.67% | now publicly exercised by `font.getlength.hdmx_observable_av`; malformed hdmx rows remain unproven |
| `pillow-rs-freetype/src/tt/mvar.rs` | 58/67 86.57% | 3/6 50.00% | 4/7 57.14% | 92/113 81.42% | now publicly exercised by `font.getmetrics.mvar_vertical_metrics`; malformed/unsupported value-tag paths remain unproven |
| `pillow-rs-freetype/src/tt/vhea.rs` | 8/11 72.73% | 1/2 50.00% | 1/1 100.00% | 8/9 88.89% | now publicly exercised by `font.getmetrics.vertical_vhea_only`; short/error path remains unproven |
| `pillow-rs-freetype/src/tt/vmtx.rs` | 28/50 56.00% | 3/8 37.50% | 1/2 50.00% | 44/65 67.69% | now publicly exercised by `font.getmetrics.vertical_vhea_only`; malformed/overflow paths remain unproven |

## Implementation differences or unproven behavior against Pillow 12.2.0

### 1. Successful libraqm shaping is intentionally not implemented

Pillow exposes `direction`, `features`, and `language` on `FreeTypeFont.getlength`, `getbbox`, `getmask`, and `getmask2`. Those successful shaping paths require libraqm.

Rust currently treats successful libraqm shaping as out of scope and uses a dedicated `PilError::UnsupportedLibraqm` internally. The parity harness now asserts every active `direction`/`features`/`language` row uses that exact internal variant before mapping the public payload to Pillow's no-libraqm `KeyError` category/message. This is correct only for the no-libraqm environment.

Decision: do not claim complete `PIL.ImageFont` parity while successful RAQM shaping is excluded.

### 2. `stroke_filled=true` is wired but not proven

Pillow `FreeTypeFont.getmask2` accepts `stroke_filled` through keyword arguments and passes it into the C render path. `_imagingft.c` chooses `FT_Glyph_StrokeBorder` when `stroke_filled=true`.

Rust now carries `stroke_filled` in `ImageFontTextOptions` and routes to `fontdone::ffi::FT_Outline_Glyph_StrokeBorder`. That removed the old explicit unsupported guard, but the lower `fontdone` stroke-border geometry for real glyph outlines is still incomplete. Commit `fd0bb7ccafd8968031e962c1f3e12c5102a5e5f0` makes `FT_Stroker_ParseOutline` follow the C contour/tag parser, but the maintained mixed-outline route remains pending because the delegated segment routes and border export are not yet general enough.

Decision: complete `FT_Stroker_ParseOutline`/border-export for real glyphs, then add successful `stroke_filled=true` fixture rows. Until then, this is a known parity gap. Do not add more glyph-specific shortcuts; the current normal-stroke path already has a DejaVu glyph-36 `A` shortcut, and a stroked `jQ` sweep row proved that Pillow succeeds while Rust fails before rendering.

### 3. Stroked extent clamping is suspect Rust-only logic

Rust clamps stroked `x_max`/`y_max` when actual bitmap extents exceed bbox-derived dimensions.

Pillow allocates the target from `bounding_box_and_anchors` and clips while writing pixels. The current evidence does not show Pillow mutating the computed extent the way Rust does.

Decision: treat this as a compatibility shim, not trusted parity. After lower stroker/bbox parity improves, remove it or prove it with an exact C-equivalent trace. The next stroke work should start in `pillow-rs-freetype/src/ffi/handles.rs` by replacing the glyph-36-specific `FT_Outline_Glyph_Stroke` shortcut with a general outline parse/export route.

### 4. BASIC layout is shared and mostly source-aligned

Pillow C lays out glyphs once and rendering consumes the resulting glyph info. Rust now builds a shared BASIC `GlyphRun` for length, bbox, mask, and stroke.

Remaining risk: fixtures need more independent kerning/no-kerning and missing-glyph transitions so shared-run parity is not only proven by duplicate easy rows.

### 5. Error mapping is now table-equivalent but not exhaustively reached

Rust maps FreeType 2.14.3 errors through a full table and returns `PilError::OsError`, matching Pillow's broad `OSError` behavior.

Remaining risk: rare FreeType errors are present as table data but not all are reachable through current public ImageFont fixtures. They should only be added if a real Pillow input can trigger them.

Resolved during the latest pass: the new `font.load_failure.missing_hmtx_table` row imports the maintained FreeType `missing-hmtx.ttf` fixture into the Font corpus. Pillow returns `OSError("horizontal metrics (hmtx) table missing")`; Rust previously returned `OSError("broken file")` because `fontdone::ffi::error_to_ft` mapped every `FontError::InvalidFont(_)` to `FT_Err_Invalid_File_Format`. The fix adds the specific `FT_Err_Hmtx_Table_Missing` mapping before the generic fallback. Coverage snapshot `b4872772-06c0-4585-acfd-e5917f1b91da` shows the new `convert.rs:203-204` branch is executed.

Also resolved during the metric pass: `font.getmetrics.fixed_width` and `font.getmetrics.hhea_zero_no_os2_fallback` import maintained FreeType metric fixtures into the Font corpus and verify Pillow/Rust exact `getmetrics()` payloads. Snapshot `33772692-59a3-46aa-9471-0c48db9437c0` showed this moved lower `pillow-rs-freetype/src/font.rs` coverage from 1260 to 1266 covered lines and from 1725 to 1735 covered regions.

Resolved during the mono BASIC pass: six input-only rows now prove public `mode="1"` parity for kerning and descender text without storing oracle outputs:

- `font.getlength.dejavusans20_av_mode_1`
- `font.getlength.dejavusans20_jq_mode_1`
- `font.getbbox.dejavusans20_av_mode_1`
- `font.getbbox.dejavusans20_jq_mode_1`
- `font.getmask.dejavusans20_jq_mode_1`
- `font.getmask2.dejavusans20_jq_mode_1`

These rows passed exact live Pillow 12.2.0 parity in `make -C pillow-rs font-tests` and Coverage MCP run `974f35c7-e61d-4dec-bc8a-16ba4e91978e`. Snapshot `06e0a61c-a56e-43e5-bfe7-a8b821be22f1` confirms `imagingft.rs` stayed at 1642/1666 lines, 246/254 branches, 163/174 functions, and 2547/2645 regions. The rows increased behavioral proof to 348 cases, but they do not move direct `imagingft.rs` region coverage; the remaining direct coverage-moving work is still stroker/stroke-border and rare reachable error paths.

Resolved during the IDEF overflow pass: `font.getlength.hinter_too_many_instruction_defs` imports the maintained FreeType `hinter-fpgm-idef-opcode-overflow.ttf` fixture into the Font corpus and proves public Pillow/Rust error parity for `OSError("too many instruction definitions")`. Rust previously classified positive IDEF opcode overflow as `InvalidOutline("bytecode: IDEF opcode out of range")`, which surfaced as Pillow-incompatible `OSError("invalid outline")`. Commit `2e45e4e4dec60bdfca5df2a7a17640f67a0037c7` now matches FreeType/Pillow's `Too_Many_Instruction_Defs` classification. Coverage snapshot `4e04ba48-488e-4798-87f6-7fc34d4ad4ab` shows lower `pillow-rs-freetype/src/tt/hinter/exec.rs` IDEF overflow handling is exercised; direct `imagingft.rs` table line `253` remains LLVM-uncovered because static table data is not attributed as executed.

Resolved during the hhea descender pass: `font.getbbox.hhea_descender_only_av` imports the maintained FreeType `hhea-descender-only.ttf` fixture into the Font corpus and proves public bbox parity for another metrics-table fallback shape.

Resolved during the lower metrics-table pass: three input-only rows now prove public ImageFont access to table paths that were previously 0% covered:

- `font.getlength.hdmx_observable_av`
- `font.getmetrics.mvar_vertical_metrics`
- `font.getmetrics.vertical_vhea_only`

Snapshot `06e0a61c-a56e-43e5-bfe7-a8b821be22f1` moves lower table coverage from 0% to: `hdmx.rs` 44/67 regions, `mvar.rs` 92/113 regions, `vhea.rs` 8/9 regions, and `vmtx.rs` 44/65 regions.

### 6. Bitmap and FreeType class shape is not 1:1

Pillow has `ImageFont.ImageFont` for bitmap fonts and `ImageFont.FreeTypeFont` for FreeType fonts. Rust currently uses `PilFont` for bitmap and `ImageFont` for FreeType.

Decision: decide whether public Rust naming should mirror Pillow more closely. The current split is testable but not class-shape parity.

### 7. Path/stream behavior is binding-owned, not core-owned

Pillow module functions accept paths and streams. Core Rust accepts bytes and options.

Decision: keep filesystem I/O outside core, but ensure binding crates remain thin and do not reimplement parsing/layout/rendering logic.

### 8. Embedded bitmap, vertical metrics, and device metrics are partially trusted

Coverage still shows weak coverage for `sbit`, `vhea`, `vmtx`, `hdmx`, and `mvar`; `vhea`, `vmtx`, `hdmx`, and `mvar` are no longer zero after the lower metrics-table pass. SBIT is now actively exercised through public `getmask`/`getmask2` rows for private-use embedded bitmap glyphs, but `sbit.rs` remains far below complete region coverage.

Resolved during the SBIT pass: previous SBIT rows used `"A"` and did not hit the embedded bitmap strikes in the generated fixtures. Commit `21086af6f5fff5921b554e3b6fe76d6613b5874d` changes those rows to private-use glyphs (`U+E000`, `U+E001`), fixes bitmap glyph layout cbox calculation in `imagingft.rs`, and expands SBIT `GRAY2`, `GRAY4`, and `BGRA` pixels to Pillow-compatible mask coverage. `make -C pillow-rs font-tests` passes with 352 rows. Coverage MCP snapshot `e67116f1-f510-46ba-80a0-23768d214d3a` confirms `tt/sbit.rs` moved to 254/814 lines and 375/1269 regions.

Boundary decision: SBIT table parsing, strike selection, glyph bitmap decoding,
compound bitmap composition, and malformed embedded-bitmap classification must
stay in `pillow-rs-freetype/src/tt/sbit.rs` and the lower FreeType-compatible
API. `pillow-rs/src/font/imagingft.rs` may only consume the resulting
FreeType-like glyph slot and apply Pillow `_imagingft` public adapter semantics:
layout bbox from bitmap glyph bounds, mask offsets, mode conversion, and final
coverage bytes. If future SBIT failures require table-format knowledge in
`imagingft.rs`, that is a layering bug; fix the lower `pillow-rs-freetype`
implementation instead.

Decision: keep the active SBIT rows as trusted public parity proof, then add further ImageFont oracle rows only for still-independent embedded bitmap formats, compound glyphs, malformed SBIT errors, vertical/TTB metrics if/when libraqm enters scope, horizontal device metrics, and variation metric deltas. If a feature is not in supported scope, record the explicit exclusion instead of leaving it ambiguous.

## Recommended action order

1. Finish `FT_Stroker_ParseOutline`/border-export support for real glyph outlines so `stroke_filled=true` can pass against Pillow.
2. Add minimal, independent oracle fixtures for:
   - successful `stroke_width + stroke_filled=true`;
   - stroked mode `"1"`;
   - height-side stroked clipping;
   - successful stroked kerning and no-kerning transitions after lower stroker support is generalized;
   - additional embedded bitmap glyph paths not covered by the current SBIT rows;
   - reachable FreeType table errors.
3. Re-run `make -C pillow-rs font-tests`.
4. Re-run Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`.
5. Update this document with the new run/snapshot and remove only gaps proven by live Pillow oracle rows.

## Current decision point

The current implementation is good enough to trust the active 352-row Font fixture corpus.

It is not yet good enough to declare full `PIL.ImageFont` parity across Pillow 12.2.0. The biggest action decision is whether to prioritize real `FT_Glyph_StrokeBorder`/stroker geometry first, because that is the clearest concrete mismatch between Pillow public behavior and Rust implementation.
