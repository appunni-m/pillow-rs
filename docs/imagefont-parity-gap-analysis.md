# ImageFont parity gap analysis against Pillow 12.2.0

Date: 2026-07-27

Rust commit reviewed: `19df430a6a25d39cc1bd325dfe55c1f704bb8214`

Coverage MCP run: `d28b871e-e1e1-46f2-9add-a3c8d867bde0`

Coverage MCP snapshot: `4bf7974a-1f89-4146-b2ce-8284c2769a7f`

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

- 353 input-only rows execute.
- 353 rows match live Pillow 12.2.0 exactly.
- Inputs under `pillow-rs/tests/fixtures/font/inputs/public-api` do not contain stored oracle output, expected error payloads, pixel hashes, or self-comparison data.
- The oracle script fails unless the repo-local venv is Pillow 12.2.0.
- `make -C pillow-rs font-tests` passes.
- Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2` passes on commit `19df430a6a25d39cc1bd325dfe55c1f704bb8214` and ingests snapshot `4bf7974a-1f89-4146-b2ce-8284c2769a7f`.
- Direction/features/language rows now prove two things separately: Rust core returns the dedicated `PilError::UnsupportedLibraqm` variant, and the public parity payload still matches Pillow's no-libraqm `KeyError`.
- Commit `19af4a948` makes `PilError::UnsupportedLibraqm` a hard-coded unit variant, so core code can no longer attach ad-hoc libraqm error text while Python and JavaScript bindings still expose Pillow's no-libraqm `KeyError` category.
- Missing horizontal metrics rows now prove the lower `fontdone` error conversion maps `FontError::InvalidFont("missing 'hmtx' table")` to `FT_Err_Hmtx_Table_Missing`, producing Pillow's public `OSError("horizontal metrics (hmtx) table missing")` instead of the old generic `OSError("broken file")`.
- Additional metric rows for fixed-width and hhea-zero/no-OS2 fallback fonts now prove `FreeTypeFont.getmetrics()` parity for two more lower metrics-table shapes.
- Additional mono BASIC rows for `AV` and `jQ` now prove live-oracle parity for normal-vs-mono load-flag behavior across `getlength`, `getbbox`, `getmask`, and `getmask2`. Coverage MCP shows these rows are semantically useful but do not reduce the remaining LLVM-reported `imagingft.rs` region gaps; the next coverage-moving gap is still lower stroker/stroke-border implementation.
- Commit `fd0bb7ccafd8968031e962c1f3e12c5102a5e5f0` moves `FT_Stroker_ParseOutline` from a two-point-line-only parser to the FreeType 2.14.3 contour/tag control flow that delegates line, conic, and cubic segments to the existing segment routes. This is architectural progress for the stroke blocker, but it does not yet move public ImageFont coverage because the mixed-outline route and general segment stroker/export behavior remain pending.
- Commit `13c410dc64fa93576f87377e2c8dde8f671f7ca9` adds three public ImageFont rows for lower metric-table paths: `hdmx_observable` through `getlength`, `mvar_vertical_metrics` through `getmetrics`, and `vertical_vhea_only` through `getmetrics`. These rows move lower `hdmx`, `mvar`, `vhea`, and `vmtx` from 0% to live Pillow-backed coverage without changing `imagingft.rs` region gaps.
- Commit `2e45e4e4dec60bdfca5df2a7a17640f67a0037c7` adds two public ImageFont rows: `font.getbbox.hhea_descender_only_av` and `font.getlength.hinter_too_many_instruction_defs`. It also fixes lower TrueType IDEF opcode-overflow classification so Pillow's public `OSError("too many instruction definitions")` matches Rust. Coverage moved lower `tt/hinter/exec.rs` but did not change direct `imagingft.rs` region totals because LLVM still attributes the static `FT_ERROR_MESSAGES` table line as uncovered.
- Commit `384a4139a07aa8b5f09486a1f034ba5fbcb9541b` adds `font.getlength.hinter_execution_too_long` using the maintained lower `hinter-execution-too-long-loop.ttf` fixture. The row passes exact live Pillow/Rust parity and is a valid public Font input, but snapshot `cb299da6-0589-4066-b118-11ed0feeeae4` shows `imagingft.rs` still unchanged: the `FT_Err_Execution_Too_Long` static table-entry line remains LLVM-uncovered even though the lower font path is exercised.
- Commit `21086af6f5fff5921b554e3b6fe76d6613b5874d` replaces false SBIT `"A"` rows with private-use glyph rows that actually hit embedded bitmap strikes, fixes bitmap glyph layout bbox calculation in `imagingft.rs`, and expands SBIT pixel modes (`GRAY2`, `GRAY4`, `BGRA`) to Pillow-compatible coverage bytes. This moves lower `tt/sbit.rs` coverage from 100/814 lines and 186/1269 regions to 254/814 lines and 375/1269 regions.
- Commits `121702b10` and `2b34fb4ac` close the Python binding option-forwarding leak for ImageFont: the thin wrapper now forwards `direction`, `features`, `language`, `stroke_width`, `stroke_filled`, `anchor`, `ink`, `mode`, and `start` into the Rust core, raises the Rust `PilError::UnsupportedLibraqm` path for no-libraqm options, and preserves Pillow-visible integral bbox value types.
- Commit `9912cf4f5` documents the hard source boundary, and commit `a19288004` makes `FT_Outline_Glyph_Stroke` attempt the FreeType-shaped parse/count/export wrapper path before falling back to the maintained DejaVu glyph-36 route. This reduces wrapper-level shortcut behavior, but the real parity blocker remains lower stroker segment geometry, border export, and destroy-option ownership.
- Commit `b71ca868e` adds a live-corpus guard that fails if any active Font input tries to claim `stroke_filled=true` branch coverage with `stroke_width > 0` before lower `FT_Glyph_StrokeBorder` success parity is implemented. The current `stroke_filled=true` row remains valid because it has no stroke width and Pillow ignores it.
- Commit `3558b7762` hardens the libraqm source guard: `PilError::UnsupportedLibraqm` must remain a unit variant with the one core hard-coded message, and `imagingft.rs` must use the dedicated constructor instead of encoding `KeyError` text directly.
- The current BGRA SBIT fixture includes an alpha-zero pixel generated by `pillow-rs-freetype/scripts/build_sbit_fixtures.py`. Existing live-oracle `getmask`/`getmask2` rows prove Pillow-compatible transparent color bitmap conversion, and the BGRA invariant cleanup removed the unreachable short-buffer adapter fallback. The stroked-extent path now computes Pillow's bbox-derived allocation bound directly instead of two explicit Rust-only clamp branches. The constructor return cleanup removed an uncovered nested-literal line artifact, but it did not reduce uncovered regions. Snapshot `4bf7974a-1f89-4146-b2ce-8284c2769a7f` reports `imagingft.rs` at 1663/1686 lines, 248/254 branches, 162/173 functions, and 2604/2700 regions.

This is still not enough to claim complete `PIL.ImageFont` parity. The safe claim is:

> Current active Font fixture rows have 100% exact runtime parity against Pillow 12.2.0.

The unsafe claim is:

> `PIL.ImageFont` is fully implemented with complete parity.

That second claim is not defensible until the gaps below are either implemented with oracle fixtures or explicitly excluded from scope.

## Source ownership boundary

The parity rule is intentionally source-shaped, not convenience-shaped:

```text
FreeType originals      -> pillow-rs-freetype
Pillow _imagingft.c     -> pillow-rs/src/font/imagingft.rs
Pillow ImageFont.py API -> Rust ImageFont facade, fixtures, and thin bindings
```

Ownership is assigned by the real upstream implementation source, not by the
crate where a workaround would be easiest. A passing public fixture is not
trusted if it is achieved by moving behavior across this boundary.

That means each layer should be a 1:1 reflection of the real upstream layer it
implements. If a behavior is FreeType-original, it is not allowed to migrate up
into `imagingft.rs` as a workaround. If a behavior is Pillow `_imagingft.c`
adapter behavior, it is not allowed to migrate down into `pillow-rs-freetype`.
If a behavior is Python `ImageFont.py` public-wrapper shape, it belongs in the
public Rust facade/tests or in thin host bindings, not in lower font machinery.

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
| total | 353 |

## Direct `pillow-rs/src/font` coverage status

Coverage snapshot: `a65df3af-cbf8-4f58-beb3-ea38e7b757b8`.

Current coverage target: drive `pillow-rs/src/font/imagingft.rs` to 100%
region coverage with live Pillow 12.2.0 oracle rows. `pillow-rs-freetype`
coverage is not itself a 100% target for this work; it is only dependency
evidence when a missing `imagingft.rs` region is blocked by missing lower
FreeType behavior. Do not broaden this into a `pillow-rs-freetype` coverage
refactor.

| File | Lines | Branches | Functions | Regions | Status |
|---|---:|---:|---:|---:|---|
| `pillow-rs/src/font/default_aileron.rs` | 17/17 100.00% | n/a | 3/3 100.00% | 24/24 100.00% | covered |
| `pillow-rs/src/font/mod.rs` | 372/372 100.00% | n/a | 80/80 100.00% | 494/494 100.00% | covered |
| `pillow-rs/src/font/pilfont.rs` | 715/737 97.01% | 142/142 100.00% | 58/78 74.36% | 1014/1094 92.69% | mostly covered; reported line gap is rustdoc on `from_pilfont_data`, but function/region gaps mean additional bitmap-font variants remain untrusted |
| `pillow-rs/src/font/imagingft.rs` | 1663/1686 98.64% | 248/254 97.64% | 162/173 93.64% | 2604/2700 96.44% | real partial branch gaps remain |

Overall snapshot totals for this suite:

- Lines: 16417/50794, 32.32%
- Branches: 2786/10766, 25.88%
- Functions: 1240/3609, 34.36%
- Regions: 23579/78660, 29.98%

The overall totals are low because the suite only targets Font behavior but the coverage artifact includes much of the workspace. For ImageFont decisions, use the file-specific rows above and the lower `pillow-rs-freetype` rows below.

## Uncovered/partial line logic analysis for `imagingft.rs`

Coverage MCP reports 9 relevant gaps in `pillow-rs/src/font/imagingft.rs`: 3 uncovered lines and 6 partial-branch lines. The previous `FT_Set_Named_Instance` success-propagation gap was resolved by making the returned FreeType status explicit before `check_ft_error(status)?`; the existing public named-instance fixture rows now cover that flow. The previous BGRA short-buffer fallback gap was removed because lower SBIT decoding guarantees FreeType-shaped BGRA bitmap storage; malformed embedded bitmap tables must be rejected in `pillow-rs-freetype`, not hidden in `_imagingft` adapter code. The previous stroked width/height clamp branch gap was removed by computing the Pillow bbox-derived allocation bound directly. Commit `0704dbe14` removed the nested constructor return line artifact, but total uncovered regions remain 96.

| Rust line(s) | Rust logic | Pillow 12.2.0 reference | Analysis | Required action |
|---:|---|---|---|---|
| `91`, `253`, `271` | Constructor return instrumentation and complete FreeType 2.14.3 error message table data. | `_imagingft.c::geterror` builds the table from FreeType `FT_ERRORS_H` and raises `OSError`; unknown table misses use `"unknown freetype error"`. | Rust is source-aligned and data-driven. The constructor line is a partial-branch artifact after splitting the nested return literal. The new `font.getlength.hinter_too_many_instruction_defs` row proves the public Pillow/Rust error payload for `FT_Err_Too_Many_Instruction_Defs`, but LLVM still reports static table line `253` as uncovered because the table data itself is not attributed as executed. `FT_Err_Invalid_Horiz_Metrics` is a FreeType-origin error, so a valid coverage row must originate from a real lower `pillow-rs-freetype` SFNT fixture and then be observed through Pillow/ImageFont if Pillow exposes it. The current lower fixture row names `fonts/synthetic/sfnt/invalid-hmtx-counts.ttf`, but that asset is not checked in and the row is still marked `unsupported_until_runner_added`. A direct Pillow 12.2.0 probe with simple `hhea.numberOfHMetrics` and `hmtx` length mutations loaded and rendered successfully, so those mutations are not acceptable ImageFont parity inputs. | Treat line `253` as behaviorally proven through public `ImageFont` error parity but still LLVM-uncovered. Do not add an ImageFont row for `Invalid_Horiz_Metrics` until `pillow-rs-freetype` has a maintained synthetic SFNT generator plus a runnable lower FreeType parity row proving pinned C returns `FT_Err_Invalid_Horiz_Metrics` for that exact asset. |
| `796` | Constant/section instrumentation around FFI helper declarations. | Not a Pillow behavior. | Coverage marks a partial branch here due LLVM segment normalization, not a meaningful behavior gap. | No product action. |
| `826`, `829` | `floor26` / `ceil26` 26.6 conversion helper branch instrumentation. | Pillow BASIC layout converts 26.6 values through `PIXEL(...)`-style rounding in `_imagingft.c`. | Partial markers mean current inputs do not hit every conversion-region shape. This is not an independent feature but can hide bbox/offset rounding differences. | Add targeted bbox/mask rows with negative bearings, fractional starts, ascenders/descenders, and kerning pairs only if they are independent public ImageFont behavior, not duplicates. |
| `928` | Branch in BASIC glyph run construction around previous-glyph kerning. | `_imagingft.c::text_layout_fallback` only adds kerning when a previous glyph exists. | Additional `mode="1"` rows for `AV` and `jQ` now prove public mono load-flag parity across length, bbox, mask, and mask2, but Coverage MCP still reports this line as partial. The remaining marker is therefore not removable by duplicate mono fixture expansion. | Keep this as a coverage artifact/branch-marker gap unless source-context evidence identifies a distinct public input. Do not add more duplicate BASIC rows only to chase this line. |
| `1211`, `1212` | `stroke_filled=true` branch routes to `FT_Outline_Glyph_StrokeBorder`. | `_imagingft.c` chooses `FT_Glyph_StrokeBorder` when `stroke_filled=true`, otherwise `FT_Glyph_Stroke`. | Rust now has a real safe wrapper and branches correctly, but active Font rows do not execute successful `stroke_filled=true`. A live-oracle probe using `font.getmask2` with DejaVuSans size 24, text `"A"`, `stroke_width=1.5`, and `kwargs.stroke_filled=true` proved Pillow 12.2.0 succeeds with a 20×21 L mask and offset `[-2, 4]`; Rust currently returns `OSError("unimplemented feature")`. This confirms the missing `imagingft.rs` branch is blocked by lower `FT_Glyph_StrokeBorder`, not by missing adapter wiring. The lower FreeType parity lane currently has 1 runnable invalid-argument row and 3 pending success/ownership rows for `FT_Glyph_StrokeBorder`. | Add only the lower stroker segment-geometry and border-export behavior required to make a real `FT_Glyph_StrokeBorder` public row pass. Do not chase 100% `pillow-rs-freetype` coverage and do not add glyph-specific shortcuts. Then add the input-only Font row and rerun live Pillow parity plus Coverage MCP. |
Exploratory note: Coverage MCP run `46f8b0bb-b94a-4eaa-8d8d-70b527901b7c`
temporarily added valid live-oracle rows for DejaVuSans `"À"` negative-top
bbox/mask and an `A\uFFFFV` missing-glyph kerning guard. The run passed and
ingested snapshot `cb8a44e6-cdc2-4faa-8c75-ab75a1b8ff1d`, but
`imagingft.rs` stayed at `2621/2720` regions with the same 16 gap lines. Those
temporary rows were not kept because they do not advance the 100% region target.

## Other ImageFont-related files where coverage is missing

These lower-level `pillow-rs-freetype` files sit underneath `ImageFont` FreeType loading, layout, metrics, glyph loading, hinting, rasterization, and embedded bitmap handling. Full ImageFont parity must either cover these through `PIL.ImageFont` fixtures or explicitly prove they are irrelevant to the supported public surface.

| File | Lines | Branches | Functions | Regions | Parity risk |
|---|---:|---:|---:|---:|---|
| `pillow-rs-freetype/src/ffi/handles.rs` | 1698/9327 18.21% | 179/2235 8.01% | 149/627 23.76% | 2225/12704 17.51% | high; includes public FreeType object/lifetime/stroker wrappers under ImageFont |
| `pillow-rs-freetype/src/api.rs` | 263/1186 22.18% | 37/294 12.59% | 28/105 26.67% | 327/1737 18.83% | high |
| `pillow-rs-freetype/src/font.rs` | 1298/4747 27.34% | 166/702 23.65% | 127/392 32.40% | 1794/6728 26.66% | high; font load/face/glyph machinery |
| `pillow-rs-freetype/src/render.rs` | 965/2459 39.24% | 157/486 32.30% | 76/158 48.10% | 1343/3432 39.13% | high; raster output parity |
| `pillow-rs-freetype/src/scaler.rs` | 806/1342 60.06% | 114/186 61.29% | 40/66 60.61% | 918/1436 63.93% | medium/high; scaling and hinted metrics |
| `pillow-rs-freetype/src/grays.rs` | 571/827 69.04% | 122/190 64.21% | 25/35 71.43% | 854/1106 77.22% | medium; antialias rasterizer |
| `pillow-rs-freetype/src/tt/sbit.rs` | 254/814 31.20% | 21/72 29.17% | 19/108 17.59% | 375/1269 29.55% | improved by active mono/gray/gray2/gray4/BGRA public rows; still high for uncovered SBIT formats and malformed paths |
| `pillow-rs-freetype/src/tt/cmap.rs` | 271/809 33.50% | 39/174 22.41% | 10/58 17.24% | 395/1089 36.27% | high for charmap/input encoding |
| `pillow-rs-freetype/src/tt/glyf.rs` | 322/545 59.08% | 55/96 57.29% | 14/20 70.00% | 416/694 59.94% | high for TrueType outlines |
| `pillow-rs-freetype/src/tt/cff.rs` | 355/735 48.30% | 37/112 33.04% | 29/81 35.80% | 507/1087 46.64% | high for CFF/OpenType |
| `pillow-rs-freetype/src/tt/hinter/exec.rs` | 725/1493 48.56% | 148/480 30.83% | 32/48 66.67% | 1298/3107 41.78% | high for hinted TrueType |
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

Rust now carries `stroke_filled` in `ImageFontTextOptions` and routes to `fontdone::ffi::FT_Outline_Glyph_StrokeBorder`. That removed the old explicit unsupported guard, but the lower `fontdone` stroke-border geometry for real glyph outlines is still incomplete. Commit `fd0bb7ccafd8968031e962c1f3e12c5102a5e5f0` makes `FT_Stroker_ParseOutline` follow the C contour/tag parser, and the latest implementation pass makes `FT_Outline_Glyph_Stroke` attempt the same parse/count/export shape used by FreeType before using the old pinned DejaVu glyph-36 fallback. The maintained mixed-outline route remains pending because the delegated segment routes and border export are not yet general enough.

The interface map now classifies the lower FreeType stroker group as partial, not out of scope: Rust has the lifecycle, segment, export, glyph-stroke, and glyph-stroke-border wrappers, but successful outside/inside border and destroy-option rows are not runnable exact parity yet.

The lower `FT_Glyph_StrokeBorder` wrapper now mirrors one more FreeType
2.14.3 detail: `src/base/ftstroke.c:2372-2373` intentionally ignores
`FT_Stroker_GetBorderCounts`' return status after `FT_Stroker_ParseOutline`
succeeds. Rust now keeps that same wrapper behavior instead of returning the
count error, but this does not make real glyph border geometry complete.

The lower stroker state now also records C-shaped left/right border point and
tag buffers for the first line segment. This follows FreeType 2.14.3
`src/base/ftstroke.c:1232-1263`: the first segment derives the normal from
`FT_Atan2`, stores the incoming angle and line length, moves the right border to
`center + normal`, moves the left border to `center - normal`, and appends the
segment endpoints. This is foundational state only; it does not yet prove
general border export, joins, caps, curves, or `FT_Glyph_StrokeBorder` success
rows.

The next lower-stroker pass records the subsequent `LineTo` candidate state
from FreeType 2.14.3 `src/base/ftstroke.c:1303-1337`: outgoing angle,
line length, offset endpoints for both borders, updated incoming angle, and
current center. Public export/count behavior remains guarded until
`ft_stroker_process_corner` is ported, so this does not claim border geometry
parity prematurely.

The border buffers now also have the FreeType-shaped public validation/export
primitive from `src/base/ftstroke.c:647-742`: count queries validate BEGIN/END
tag balance, mark the border valid only after successful validation, and export
public outline tags/contours from the accumulated stroke tags. This is required
plumbing for real border export; the success rows still stay pending until
corner joins, caps, curves, and close behavior are ported.

`FT_Stroker_LineTo` no longer has the Rust-only two-segment limit. It now follows
FreeType 2.14.3 `src/base/ftstroke.c:1303-1337` by appending candidate border
state for every later line segment. This moves the next real blocker to
`FT_Stroker_EndSubPath`/corner processing rather than failing early during
outline parsing.

The lower stroker now ports the first source-shaped round-corner slice from
FreeType 2.14.3 `src/base/ftstroke.c:532-586`, `883-902`, `960-1028`, and
`1219-1229`: border cubic arc emission, side-to-rotate handling, inside-corner
intersection/offset handling, and round-join outside-corner dispatch are part
of the general `LineTo` path instead of a glyph fixture shortcut. This is real
lower-layer progress, but Coverage MCP snapshot
`bb33eecf-9bf5-4f3b-ab20-a4e1e13e378e` confirms it still does not move
`imagingft.rs` line 1212 because public `FT_Glyph_StrokeBorder` success rows
remain route-gated until the full glyph-object geometry and ownership behavior
match pinned C exactly.

The lower `FT_Stroker_EndSubPath` path now stages FreeType 2.14.3
`src/base/ftstroke.c:1907-1930` for closed round paths: it adds a final line
back to the subpath start when needed, processes the final corner against the
first segment angle, then closes the right border forward and the left border
reversed. This state is deliberately marked unverified for full glyph export:
when the existing `FT_Glyph_Stroke` full-outline wrapper sees that staged path,
it returns `FT_Err_Unimplemented_Feature` so the maintained exact DejaVu glyph
fallback still owns the public passing row. A direct test proved why this guard
is required: without it, the public Font stroke row returned successful but
wrong pixels. Coverage MCP snapshot `e195ed6f-47b4-4011-86af-fb5845b0748a`
therefore shows lower `handles.rs` coverage progress while `imagingft.rs`
remains unchanged at 2604/2700 regions.

The lower `FT_Stroker_ConicTo` path now stages the source-shaped small-conic
case from FreeType 2.14.3 `src/base/ftstroke.c:104-150` and `1395-1522`:
it classifies already-small conic arcs, initializes or joins the current
subpath, and appends offset conic border segments to both borders. It
deliberately refuses wide-stroke conics and subdivision cases for now, and the
full glyph export wrapper treats this dynamic conic path as unverified so it
cannot replace the maintained exact glyph fallback. Snapshot
`4bf7974a-1f89-4146-b2ce-8284c2769a7f` shows this as lower `handles.rs`
progress only; `imagingft.rs` remains unchanged at 2604/2700 regions.

Decision: complete the lower stroker segment geometry and border-export behavior
needed by real outline glyphs, then add successful `stroke_filled=true` fixture
rows. `FT_Stroker_ParseOutline` now follows the C-shaped contour/tag walk, so
the remaining blocker is not an `imagingft.rs` wrapper problem and not a reason
to pursue 100% `pillow-rs-freetype` coverage. Until then, this is a known
parity gap. Do not add more glyph-specific shortcuts; the current normal-stroke
path still has a DejaVu glyph-36 `A` fallback for the existing passing route,
and a stroked `jQ` sweep row proved that Pillow succeeds while Rust fails before
rendering.

Current lower-stroker verification:

- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke`
  passes the maintained runnable rows, but only 4 rows are runnable and 4 remain
  pending. The pending rows are destroy-option coverage plus the
  `FT_Glyph_StrokeBorder` inside/outside/destroy routes that the combined case
  filter reports as owned follow-up work.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_StrokeBorder`
  passes the maintained runnable row, but only 1 row is runnable and 3 remain
  pending: outside-border success, inside-border success, and destroy-option
  parity.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_LineTo`
  passes 5/5 runnable rows after the first-line border state update. This
  verifies no regression in the public segment lane; it is not proof of general
  glyph border stroking.
- Source inspection shows the real blocker is still lower stroker geometry in
  `pillow-rs-freetype/src/ffi/handles.rs`: `FT_Stroker_LineTo`,
  `FT_Stroker_ConicTo`, and `FT_Stroker_CubicTo` contain maintained
  exact-coordinate fixture routes and otherwise return
  `FT_Err_Unimplemented_Feature`. The segment-level public rows currently pass:
  `FT_Stroker_LineTo` 5/5 runnable, `FT_Stroker_ConicTo` 4/4 runnable, and
  `FT_Stroker_CubicTo` 4/4 runnable. That proves the maintained rows, not
  general glyph stroking. `FT_Outline_Glyph_Stroke` already attempts the
  FreeType-shaped parse/count/export wrapper first, so more wrapper wiring in
  `imagingft.rs` would not fix the source mismatch.

Layering decision: `Pillow _imagingft.c` only chooses between
`FT_Glyph_Stroke` and `FT_Glyph_StrokeBorder`, passes the configured stroker
settings, renders the returned glyph, and maps any FreeType status to Pillow's
public exception shape. General stroke geometry, border orientation/export,
curve subdivision, cap/join behavior, and destroy-option ownership are
FreeType-original behavior. They must be implemented in
`pillow-rs-freetype`; adding glyph-specific or bbox-clamping fixes in
`imagingft.rs` would be false parity.

### 3. Stroked extent clamping is suspect Rust-only logic

Rust clamps stroked `x_max`/`y_max` when actual bitmap extents exceed bbox-derived dimensions.

Pillow allocates the target from `bounding_box_and_anchors` and clips while writing pixels. The current evidence does not show Pillow mutating the computed extent the way Rust does.

Decision: treat this as a compatibility shim, not trusted parity. After lower stroker/bbox parity improves, remove it or prove it with an exact C-equivalent trace. The next stroke work should continue in `pillow-rs-freetype/src/ffi/handles.rs` by replacing the remaining glyph-36-specific fallback with general segment geometry and border export.

### 4. BASIC layout is shared and mostly source-aligned

Pillow C lays out glyphs once and rendering consumes the resulting glyph info. Rust now builds a shared BASIC `GlyphRun` for length, bbox, mask, and stroke.

Remaining risk: fixtures need more independent kerning/no-kerning and missing-glyph transitions so shared-run parity is not only proven by duplicate easy rows.

### 5. Error mapping is now table-equivalent but not exhaustively reached

Rust maps FreeType 2.14.3 errors through a full table and returns `PilError::OsError`, matching Pillow's broad `OSError` behavior.

Remaining risk: rare FreeType errors are present as table data but not all are reachable through current public ImageFont fixtures. They should only be added if a real Pillow input can trigger them.

Layering decision for `FT_Err_Invalid_Horiz_Metrics`: this is not an
`imagingft.rs` implementation gap by itself. FreeType's `sfnt/ttload.c` and
TrueType metrics loader own the original behavior, so the first required fix is
lower-layer: add or regenerate a maintained synthetic SFNT asset under
`pillow-rs-freetype` that pinned C FreeType rejects with
`FT_Err_Invalid_Horiz_Metrics`, then promote the existing lower fixture row from
`unsupported_until_runner_added` to exact runtime parity. Only after that lower
row is real should the same asset be imported into the ImageFont corpus, and
only if Pillow 12.2.0 exposes the same public `OSError("invalid horizontal
metrics")` through `PIL.ImageFont.truetype` or a public font method. Simple
mutations of `hhea.numberOfHMetrics` and the `hmtx` directory length were probed
against the repo Pillow 12.2.0 oracle and did not trigger this public error;
they loaded and rendered successfully, so using them as ImageFont rows would be
false coverage.

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

Resolved during the SBIT pass: previous SBIT rows used `"A"` and did not hit the embedded bitmap strikes in the generated fixtures. Commit `21086af6f5fff5921b554e3b6fe76d6613b5874d` changes those rows to private-use glyphs (`U+E000`, `U+E001`), fixes bitmap glyph layout cbox calculation in `imagingft.rs`, and expands SBIT `GRAY2`, `GRAY4`, and `BGRA` pixels to Pillow-compatible mask coverage. The current BGRA fixture adds an alpha-zero pixel through the maintained generator, so `gray_for_premultiplied_srgb_bgra` now covers both transparent and non-transparent branches under live Pillow oracle rows. The BGRA adapter now relies on lower SBIT buffer invariants instead of hiding malformed table output in `_imagingft`. The latest Coverage MCP snapshot `a65df3af-cbf8-4f58-beb3-ea38e7b757b8` reports `imagingft.rs` at 1663/1686 lines, 248/254 branches, and 2604/2700 regions.

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

1. Add the smallest lower stroker segment-geometry and border-export behavior
   needed for a real public `ImageFont.getmask2(stroke_filled=true)` Pillow
   row. This is dependency work only; do not chase 100%
   `pillow-rs-freetype` coverage and do not add glyph-specific shortcuts.
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

The current implementation is good enough to trust the active 353-row Font fixture corpus.

It is not yet good enough to declare full `PIL.ImageFont` parity across Pillow 12.2.0. The biggest action decision is whether to prioritize real `FT_Glyph_StrokeBorder`/stroker geometry first, because that is the clearest concrete mismatch between Pillow public behavior and Rust implementation.

Latest focused ftstroke evidence after the export-append runner update:

- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker`: 59/59 runnable rows pass, 9 rows remain pending. The parsed `FT_Stroker.lifecycle_contract` row now validates New, Set, BeginSubPath, two LineTo calls, EndSubPath, GetCounts, Export, and Done status/count behavior through pinned C, Rust FFI, C ABI, and WASM ABI.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke`: 4/4 runnable rows pass, 4 rows remain pending.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_StrokeBorder`: 1/1 runnable row passes, 3 rows remain pending.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_Export`: 7/7 runnable rows pass, 0 pending. This now includes `append_to_existing_outline` with sentinel-prefix preservation and contour-index offset comparison against the pinned C oracle.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_ExportBorder`: 4/4 runnable rows pass, 0 pending. This now includes selected-border append-to-existing-outline parity.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_LineTo`: 5/5 runnable rows pass, 0 pending.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_ConicTo`: 4/4 runnable rows pass, 0 pending. Commit-in-progress ports the FreeType `ft_conic_split` stack shape and dispatches `FT_Stroker_ConicTo` through the staged generic conic route, but the Font public corpus does not yet reach those new lines because public `stroke_filled=true` remains guarded by the lower `FT_Glyph_StrokeBorder` success blocker.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_CubicTo`: 4/4 runnable rows pass, 0 pending.
- `pillow-rs-freetype/target/api-abi-audit/route_audit.json` now reports 180 `pending-route` cases overall, down from 183 after promoting the two export append rows and the parsed stroker lifecycle row to real runtime parity. The project still cannot claim complete FreeType-backed ImageFont parity yet.

Latest Coverage MCP evidence after the conic-subdivision and lint pass:

- Run `d58a4ca9-137b-4fb4-8598-a748291e4d9f`, snapshot `6577461e-2c33-40c4-9f1a-9330404c39c4`, command `font-tests-coverage-with-freetype-pillow-12-2`, suite `font-with-freetype`, commit `99f7e415d6583400f58e3c95c566aca48bbcb382`, status `passed`, ingested.
- `pillow-rs/src/font/imagingft.rs` remains 1663/1686 lines, 248/254 branches, 162/173 functions, and 2604/2700 regions.
- Remaining `imagingft.rs` direct gaps are unchanged: line 91 partial branch; table-data lines 253 and 271; rounding/helper branch lines 796, 826, 829, and 928; and the real public blocker at lines 1211-1212 where `stroke_filled=true` must call `FT_Outline_Glyph_StrokeBorder`.
- Conclusion: do not chase 100% region coverage in `pillow-rs-freetype`. The next coverage-moving ImageFont task is still to make lower `FT_Glyph_StrokeBorder` inside/outside success geometry real enough for a live Pillow oracle `ImageFont.getmask2(..., stroke_width>0, stroke_filled=true)` row.
