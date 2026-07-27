# ImageFont parity gap analysis against Pillow 12.2.0

Date: 2026-07-27

Rust commit reviewed: `275d941f`

Coverage MCP run: `ea51012f-1da6-4e2e-b60a-1768e7fa6f87`

Coverage MCP snapshot: `facad7de-822e-45d5-961b-7534bbdc3b3b`

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

- 356 input-only rows execute.
- 356 rows match live Pillow 12.2.0 exactly.
- Inputs under `pillow-rs/tests/fixtures/font/inputs/public-api` do not contain stored oracle output, expected error payloads, pixel hashes, or self-comparison data.
- The oracle script fails unless the repo-local venv is Pillow 12.2.0.
- `make -C pillow-rs font-tests` passes.
- Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2` passes and ingests snapshot `facad7de-822e-45d5-961b-7534bbdc3b3b`.
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
- Current audit at `275d941f` confirms the libraqm contract is enforced in `pillow-rs/tests/font_public_api.rs`: direction/features/language rows must return the dedicated core `PilError::UnsupportedLibraqm`, core must contain the hard-coded no-libraqm message exactly once, `imagingft.rs` must call `PilError::unsupported_libraqm()` exactly once, and host bindings must map the variant to Pillow-compatible `KeyError`. `layout_engine="RAQM"` remains separate because Pillow 12.2.0 without libraqm accepts that constructor option and falls back to BASIC; it is not a successful libraqm shaping path.
- Coverage MCP run `ea51012f-1da6-4e2e-b60a-1768e7fa6f87` at commit `275d941fcb5a73319022986069a09b3fb6e1e58b` passed and ingested snapshot `facad7de-822e-45d5-961b-7534bbdc3b3b`. Direct `imagingft.rs` coverage remains 1664/1686 lines, 249/254 branches, 162/173 functions, and 2608/2700 regions; the lower fallback cleanup intentionally does not claim new active ImageFont coverage.
- The BGRA SBIT fixture includes an alpha-zero pixel generated by `pillow-rs-freetype/scripts/build_sbit_fixtures.py`. Existing live-oracle `getmask`/`getmask2` rows prove Pillow-compatible transparent color bitmap conversion, and the BGRA invariant cleanup removed the unreachable short-buffer adapter fallback. The stroked-extent path computes Pillow's bbox-derived allocation bound directly instead of two explicit Rust-only clamp branches. The constructor return cleanup removed an uncovered nested-literal line artifact, but it did not reduce uncovered regions. Historical snapshot `4bf7974a-1f89-4146-b2ce-8284c2769a7f` reported `imagingft.rs` at 1663/1686 lines, 248/254 branches, 162/173 functions, and 2604/2700 regions before the later stroke-filled row covered line 1212.

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
| `ImageFont.FreeTypeFont` | `getname`, `getmetrics`, `getlength`, `getbbox`, `getmask`, `getmask2`, `font_variant`, `get_variation_names`, `set_variation_by_name`, `get_variation_axes`, `set_variation_by_axes` | Mostly modeled through Rust `ImageFont`. BASIC layout paths are oracle-tested. Successful libraqm shaping is out of scope. The public `getmask2(..., stroke_width=1.5, stroke_filled=True)` route is proven for the maintained DejaVuSans glyph-36 outside-border path; broader inside-border and destroy-option stroke parity remain incomplete. |
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
| `font.getlength.json` | 23 |
| `font.getmask.json` | 38 |
| `font.getmask2.json` | 46 |
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
| total | 356 |

## Direct `pillow-rs/src/font` coverage status

Coverage snapshot: `23fab2f2-78d7-4910-9a32-14d72c712804`
from Coverage MCP run `7376f5b8-9f4f-4a83-aa15-7a94efc926d2`
at commit `8a6cd50ef6631b2e90d8d703bbdc1179b0435e8e`.

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
| `pillow-rs/src/font/imagingft.rs` | 1664/1686 98.70% | 249/254 98.03% | 162/173 93.64% | 2608/2700 96.59% | `stroke_filled=true` branch is now covered; remaining markers are static data or LLVM region artifacts |

Overall snapshot totals for this suite:

- Lines: 17092/51970, 32.89%
- Branches: 2900/10934, 26.52%
- Functions: 1302/3651, 35.66%
- Regions: 24483/79905, 30.64%

The overall totals are low because the suite only targets Font behavior but the coverage artifact includes much of the workspace. For ImageFont decisions, use the file-specific rows above and the lower `pillow-rs-freetype` rows below.

## Uncovered/partial line logic analysis for `imagingft.rs`

Coverage MCP reports 7 relevant gaps in `pillow-rs/src/font/imagingft.rs`: 2 uncovered static-data lines and 5 partial-branch lines. The previous `FT_Set_Named_Instance` success-propagation gap was resolved by making the returned FreeType status explicit before `check_ft_error(status)?`; the existing public named-instance fixture rows now cover that flow. The previous BGRA short-buffer fallback gap was removed because lower SBIT decoding guarantees FreeType-shaped BGRA bitmap storage; malformed embedded bitmap tables must be rejected in `pillow-rs-freetype`, not hidden in `_imagingft` adapter code. The previous stroked width/height clamp branch gap was removed by computing the Pillow bbox-derived allocation bound directly. The previous `stroke_filled=true` branch gap was resolved by the maintained lower `FT_Glyph_StrokeBorder.outside_border_success` route plus the live Pillow `font.getmask2.dejavusans24_a_stroke_1_5_filled_l` row.

| Rust line(s) | Rust logic | Pillow 12.2.0 reference | Analysis | Required action |
|---:|---|---|---|---|
| `91`, `253`, `271` | Constructor return instrumentation and complete FreeType 2.14.3 error message table data. | `_imagingft.c::geterror` builds the table from FreeType `FT_ERRORS_H` and raises `OSError`; unknown table misses use `"unknown freetype error"`. | Rust is source-aligned and data-driven. The constructor line is a partial-branch artifact after splitting the nested return literal. The new `font.getlength.hinter_too_many_instruction_defs` row proves the public Pillow/Rust error payload for `FT_Err_Too_Many_Instruction_Defs`, but LLVM still reports static table line `253` as uncovered because the table data itself is not attributed as executed. `FT_Err_Invalid_Horiz_Metrics` is a FreeType-origin error, so a valid coverage row must originate from a real lower `pillow-rs-freetype` SFNT fixture and then be observed through Pillow/ImageFont if Pillow exposes it. The current lower fixture row names `fonts/synthetic/sfnt/invalid-hmtx-counts.ttf`, but that asset is not checked in and the row is still marked `unsupported_until_runner_added`. A direct Pillow 12.2.0 probe with simple `hhea.numberOfHMetrics` and `hmtx` length mutations loaded and rendered successfully, so those mutations are not acceptable ImageFont parity inputs. | Treat line `253` as behaviorally proven through public `ImageFont` error parity but still LLVM-uncovered. Do not add an ImageFont row for `Invalid_Horiz_Metrics` until `pillow-rs-freetype` has a maintained synthetic SFNT generator plus a runnable lower FreeType parity row proving pinned C returns `FT_Err_Invalid_Horiz_Metrics` for that exact asset. |
| `796` | LLVM segment metadata around FFI helper declarations. | Not a Pillow behavior. | Coverage MCP reports the nearby helper lines as heavily executed (`gid`, `kern_26dot6`, `basic_layout_kern`, `pixel`, `floor26`, `ceil26` all have hits). The partial marker sits on the section/comment boundary, not on an executable Pillow branch. | No product action. |
| `826`, `829` | LLVM function-boundary/brace instrumentation around `floor26`; executable rounding body is line `828`. | Pillow BASIC layout converts 26.6 values through floor/ceil-style bbox math in `_imagingft.c::bounding_box_and_anchors`. | Coverage MCP reports line `828` hit 164624 times and `ceil26` lines `831-833` hit 164624 times. The remaining partial markers are attached to the function boundary/closing brace, not to an unhit rounding expression. Temporary negative-top and missing-glyph kerning rows also passed live Pillow oracle but did not move these markers. | Do not add duplicate bbox/mask rows solely for these markers. Add new rows only when they cover an independent public ImageFont behavior. |
| `928` | LLVM function-signature instrumentation for `bbox_from_run_with_flags`; executable body is lines `932-933`. | `_imagingft.c::font_getbbox` delegates to the shared BASIC layout and bbox computation. | Coverage MCP reports line `928` hit 2991 times with one synthetic missing branch, while the actual body lines `932-933` are covered. Additional kerning/mono rows did not move the marker. | Treat as a coverage artifact unless future source-context evidence identifies a real unhit branch. |
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

### 2. `stroke_filled=true` is proven for the maintained outside-border row only

Pillow `FreeTypeFont.getmask2` accepts `stroke_filled` through keyword arguments and passes it into the C render path. `_imagingft.c` chooses `FT_Glyph_StrokeBorder` when `stroke_filled=true`.

Rust carries `stroke_filled` in `ImageFontTextOptions` and routes to `fontdone::ffi::FT_Outline_Glyph_StrokeBorder`. Commit `fc233cfb7` adds the maintained lower `FT_Glyph_StrokeBorder.outside_border_success` route and the live Pillow `font.getmask2.dejavusans24_a_stroke_1_5_filled_l` row. That row proves the public DejaVuSans glyph-36 outside-border path across Pillow 12.2.0, Rust FFI, C ABI, and WASM ABI.

This is not general stroke-border parity. The lower `fontdone` stroke-border geometry for broader real glyph outlines is still incomplete. Commit `fd0bb7ccafd8968031e962c1f3e12c5102a5e5f0` makes `FT_Stroker_ParseOutline` follow the C contour/tag parser, and the latest implementation pass makes `FT_Outline_Glyph_Stroke` attempt the same parse/count/export shape used by FreeType before using the old pinned DejaVu glyph-36 fallback. The maintained mixed-outline route remains pending because the delegated segment routes and border export are not yet general enough.

The interface map classifies the lower FreeType stroker group as partial, not out of scope: Rust has the lifecycle, segment, export, glyph-stroke, and glyph-stroke-border wrappers, and the maintained outside-border row is runnable exact parity. Inside-border and destroy-option rows are still pending, and general glyph stroking remains guarded.

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

Decision: keep the successful `stroke_filled=true` Font row because it is backed
by a real lower outside-border C oracle route. Continue lower stroker segment
geometry and border-export work before adding broader inside-border,
destroy-option, or additional glyph-shape stroke fixtures. `FT_Stroker_ParseOutline`
now follows the C-shaped contour/tag walk, so the remaining general-stroke
blocker is not an `imagingft.rs` wrapper problem and not a reason to pursue
100% `pillow-rs-freetype` coverage. Do not add more glyph-specific shortcuts;
the current normal-stroke path still has a DejaVu glyph-36 `A` fallback for the
existing passing route, and a stroked `jQ` sweep row proved that Pillow succeeds
while Rust fails before rendering.

Latest Font-corpus sweep: two active input-only rows now cover height-side
stroked clipping through live Pillow 12.2.0 oracle parity:

- `font.getmask.dejavusans24_a_stroke_start_negative_y_clips`
- `font.getmask2.dejavusans24_a_stroke_start_negative_y_clips`

The attempted independent `stroke_width=1.5, mode="1"` rows were not kept
active because they exposed a real lower stroke-outline blocker. Direct Pillow
12.2.0 reports `mode="1"` stroked glyph 36 as a `19x21` L mask with the
mono-target stroked outline bytes, while current Rust produces the normal
stroked outline bytes for that row. This must be fixed in the lower
`pillow-rs-freetype` stroke implementation by making the real stroked outline
depend on the loaded outline, not by adding a new glyph-specific shortcut or
weakening the Font oracle comparison.

Current lower-stroker verification:

- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke`
  passes the maintained runnable rows, but the active
  `outline_glyph_stroked_success` route loads glyph 36 with
  `FT_LOAD_NO_BITMAP`, not the public ImageFont blocker shape
  `FT_LOAD_TARGET_MONO`. Only 4 rows are runnable and 4 remain pending. The
  pending rows are destroy-option coverage plus the `FT_Glyph_StrokeBorder`
  inside/outside/destroy routes that the combined case filter reports as owned
  follow-up work.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_StrokeBorder`
  passes 2/2 runnable rows. The remaining pending rows are inside-border
  success and destroy-option parity.
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

### 3. Stroked extent clipping is Pillow adapter behavior

Rust bounds stroked `x_max`/`y_max` to Pillow's bbox-derived target when actual bitmap extents exceed those dimensions.

Pillow allocates the target from `bounding_box_and_anchors` and clips while writing pixels. This is adapter behavior in `_imagingft.c`, not lower FreeType geometry.

Decision: keep the adapter bound because it matches Pillow's public allocation
contract. After the outside-border route update, removing the bound was tested
directly against the live Pillow Font corpus. `make -C pillow-rs font-tests`
failed on `font.getmask.dejavusans24_a_stroke_1_5_l`: Pillow returned a
`20x21` L mask, while unbounded Rust returned `20x22` with an extra top row.

A direct pinned C FreeType 2.14.3 diagnostic then proved lower FreeType is not
the source of the one-row public difference. For DejaVuSans glyph 36 at
`FT_Set_Char_Size(..., 1536, 72, 72)`, `FT_LOAD_NO_BITMAP`, stroker radius 96,
round cap, round join, and `FT_RENDER_MODE_NORMAL`, C FreeType reports:

- original outline cbox: `xMin=12 yMin=0 xMax=1038 yMax=1152`
- after `FT_Glyph_Stroke`: cbox `xMin=-89 yMin=-96 xMax=1139 yMax=1248`
- after `FT_Glyph_Stroke` + `FT_Glyph_To_Bitmap`: bitmap `20x22`, `left=-2`, `top=20`
- after `FT_Glyph_StrokeBorder(..., inside=0)` + `FT_Glyph_To_Bitmap`: bitmap `20x22`, `left=-2`, `top=20`

That means the lower C oracle and the lower Rust route both naturally produce a
`20x22` stroked glyph bitmap. Pillow's public `20x21` result comes from
`_imagingft.c::font_render_impl` allocating from `bounding_box_and_anchors` and
clipping writes to that target. This bound belongs in `imagingft.rs`; moving it
down into `pillow-rs-freetype` would make the lower FreeType layer wrong.

The next lower stroke work should still continue in
`pillow-rs-freetype/src/ffi/handles.rs` by replacing remaining glyph-36-specific
fallbacks with general segment geometry and border export, but that work is not
needed to explain the `20x21` vs `20x22` public ImageFont extent.

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

Resolved during the SBIT pass: previous SBIT rows used `"A"` and did not hit the embedded bitmap strikes in the generated fixtures. Commit `21086af6f5fff5921b554e3b6fe76d6613b5874d` changes those rows to private-use glyphs (`U+E000`, `U+E001`), fixes bitmap glyph layout cbox calculation in `imagingft.rs`, and expands SBIT `GRAY2`, `GRAY4`, and `BGRA` pixels to Pillow-compatible mask coverage. The current BGRA fixture adds an alpha-zero pixel through the maintained generator, so `gray_for_premultiplied_srgb_bgra` now covers both transparent and non-transparent branches under live Pillow oracle rows. The BGRA adapter now relies on lower SBIT buffer invariants instead of hiding malformed table output in `_imagingft`. Historical Coverage MCP snapshot `a65df3af-cbf8-4f58-beb3-ea38e7b757b8` reported `imagingft.rs` at 1663/1686 lines, 248/254 branches, and 2604/2700 regions before the later stroke-filled row covered line 1212.

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

1. Add minimal, independent oracle fixtures for:
   - stroked mode `"1"` after lower stroke-outline parity handles mono-target
     stroked outlines without glyph-specific shortcuts;
   - height-side stroked clipping: covered for DejaVuSans glyph 36 by the
     current negative-Y `getmask` and `getmask2` rows;
   - successful stroked kerning and no-kerning transitions after lower stroker support is generalized;
   - additional embedded bitmap glyph paths not covered by the current SBIT rows;
   - reachable FreeType table errors.
2. Re-run `make -C pillow-rs font-tests`.
3. Re-run Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`.
4. Update this document with the new run/snapshot and remove only gaps proven by live Pillow oracle rows.

## Current decision point

The current implementation is good enough to trust the active 356-row Font fixture corpus.

It is not yet good enough to declare full `PIL.ImageFont` parity across Pillow 12.2.0. The biggest action decision is whether to prioritize real `FT_Glyph_StrokeBorder`/stroker geometry first, because broader stroke-border/destroy/general glyph support is still incomplete even though the active public extent behavior is now explained by `_imagingft.c` clipping.

Latest focused ftstroke evidence after the outside-border route update:

- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker`: 59/59 runnable rows pass, 9 rows remain pending. The parsed `FT_Stroker.lifecycle_contract` row now validates New, Set, BeginSubPath, two LineTo calls, EndSubPath, GetCounts, Export, and Done status/count behavior through pinned C, Rust FFI, C ABI, and WASM ABI.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke`: 4/4 runnable rows pass, 4 rows remain pending.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_StrokeBorder`: 2/2 runnable rows pass, 2 rows remain pending. The newly maintained `outside_border_success` route compares selected border, replacement outline points/tags/contours, CBox, status sequence, and preserve-original ownership against pinned C, Rust FFI, C ABI, and WASM ABI.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_Export`: 7/7 runnable rows pass, 0 pending. This now includes `append_to_existing_outline` with sentinel-prefix preservation and contour-index offset comparison against the pinned C oracle.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_ExportBorder`: 4/4 runnable rows pass, 0 pending. This now includes selected-border append-to-existing-outline parity.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_LineTo`: 5/5 runnable rows pass, 0 pending.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_ConicTo`: 4/4 runnable rows pass, 0 pending. Commit `99f7e415d` ports the FreeType `ft_conic_split` stack shape and dispatches `FT_Stroker_ConicTo` through the staged generic conic route. Public `stroke_filled=true` now reaches the maintained outside-border glyph row, but general closed round-path stroker geometry remains guarded for broader glyph shapes.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_CubicTo`: 4/4 runnable rows pass, 0 pending.
- `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap`: 9/9 runnable rows pass, 2 rows remain pending. The pending row `pending_stroked_mono_target_outline_to_bitmap` names the exact lower sequence blocking public `mode="1"` stroked ImageFont parity: `FT_LOAD_TARGET_MONO` outline, `FT_Glyph_Stroke`, then `FT_Glyph_To_Bitmap` with `FT_RENDER_MODE_NORMAL`, comparing bitmap placement and coverage bytes against pinned C. The native C oracle command and Rust/C/WASM runner branches now execute this exact sequence; temporarily promoting the row to real parity exposes a `/bitmap/buffer_hex` mismatch, while bitmap shape/placement has advanced past the previous wrong-oracle `/advance/x` artifact.
- `pillow-rs-freetype/target/api-abi-audit/route_audit.json` now reports 180 `pending-route` cases overall and 4842 `real-parity` cases after promoting `FT_Glyph_StrokeBorder.outside_border_success` and adding the explicit pending `FT_Glyph_To_Bitmap` blocker route. The project still cannot claim complete FreeType-backed ImageFont parity yet because `inside_border_success`, destroy-option ownership, stroked mono-target bitmap conversion, and general glyph stroke geometry remain incomplete.

Latest Coverage MCP evidence after the outside-border, Font `stroke_filled`, and height-side stroked clipping rows:

- Run `b4d8d9bc-4468-4127-bf57-f635104ac5ee`, snapshot `ad94bdb5-c232-4f4a-9d8f-5f2172f15f65`, command `font-tests-coverage-with-freetype-pillow-12-2`, suite `font-with-freetype`, status `passed`, ingested.
- Refreshed run after this blocker classification: `b728484a-5ef8-4a5e-bff6-0ced6f559172`, snapshot `3c27125a-5b8c-4406-b19e-0a640f80d7d5`, commit `4d44a5ace53c32d054ae7d6f11b13cc216d68893`, command `font-tests-coverage-with-freetype-pillow-12-2`, suite `font-with-freetype`, status `passed`, ingested.
- Refreshed run after adding the explicit lower pending `FT_Glyph_To_Bitmap` blocker route: `bbc5c38e-ddf1-4908-9669-b7aed2ed69b5`, snapshot `e7264a1c-7c4a-4c35-8a11-dd700af601ef`, commit `162de125d1e36cb88b5c50006513d028db9df6c6`, command `font-tests-coverage-with-freetype-pillow-12-2`, suite `font-with-freetype`, status `passed`, ingested.
- Refreshed run after wiring the exact native C oracle and Rust/C/WASM runner route for the stroked mono-target glyph-to-bitmap sequence: `eff0e9cf-09c0-479c-b86e-63865875b6ef`, snapshot `4e2e6bc3-c28f-4453-8600-f21dfc7885bd`, commit `41b2b74451a906938d481935c962e11e25109cd9`, command `font-tests-coverage-with-freetype-pillow-12-2`, suite `font-with-freetype`, status `passed`, ingested.
- The new input-only Font row `font.getmask2.dejavusans24_a_stroke_1_5_filled_l` passes exact live Pillow 12.2.0 oracle parity and reaches `imagingft.rs:1212`.
- The new input-only Font rows `font.getmask.dejavusans24_a_stroke_start_negative_y_clips` and `font.getmask2.dejavusans24_a_stroke_start_negative_y_clips` pass exact live Pillow 12.2.0 oracle parity. They increase active corpus proof but do not move direct `imagingft.rs` coverage metrics.
- `pillow-rs/src/font/imagingft.rs` is now 1664/1686 lines, 249/254 branches, 162/173 functions, and 2608/2700 regions.
- The prior real public blocker at lines 1211-1212 is resolved: line 1211 has both branches covered and line 1212 has one hit.
- Remaining direct gaps are line 91 partial branch; static FreeType error-table data lines 253 and 271; and LLVM partial-branch artifacts around helper/comment or bit-rounding lines 796, 826, 829, and 928. These are not currently known public ImageFont behavior mismatches.
- Conclusion: do not chase 100% region coverage in `pillow-rs-freetype`. For `imagingft.rs`, the remaining direct gaps are currently classified as static-data or LLVM segment artifacts, not known public Pillow behavior misses. Add new Font rows only when they exercise independent ImageFont behavior, not to force these markers.

Current request classification for `imagingft.rs` region coverage:

- `imagingft.rs` has no known remaining adapter-owned implementation branch
  that should be filled by moving FreeType logic upward. The adapter already
  follows Pillow's `_imagingft.c` shape for stroked text: load the glyph using
  the public BASIC load flags, call `FT_Get_Glyph`, call `FT_Glyph_Stroke` or
  `FT_Glyph_StrokeBorder`, convert the stroked outline to a normal gray bitmap,
  then clip/paste into the Pillow-sized mask.
- The rejected public `stroke_width=1.5, mode="1"` rows are valid missing
  behavior, but the first divergence is lower than `imagingft.rs`. A focused
  diagnostic temporarily added
  `font.getmask.dejavusans24_a_stroke_1_5_mode_1_probe`; the live Pillow oracle
  and Rust both returned a `19x21` L mask, but coverage bytes differed from the
  first non-zero pixel. That rules out the `_imagingft` allocation/offset path
  for this case.
- The Font runner and root API plumbing are not the cause: the row contains
  both `mode` and `stroke_width`, so it routes through
  `ImageFontTextOptions` and `imagefont_getmask_with_options`, and
  `imagingft.rs::text_load_flags` maps `mode="1"` to `FT_LOAD_TARGET_MONO`.
- Lower `FT_Glyph_Stroke` outline/cbox parity is also not enough to close this
  row. A temporary diagnostic changed the maintained
  `ftstroke.FT_Glyph_Stroke.outline_glyph_stroked_success` row to
  `FT_LOAD_TARGET_MONO`; it still passed. A second temporary diagnostic made the
  lower harness request `_imagingft`-style char width and height
  (`size_26_6, size_26_6`) instead of width `0`; it still passed. Therefore the
  next first-divergence target is the combined lower sequence:
  `FT_LOAD_TARGET_MONO` outline -> `FT_Glyph_Stroke` ->
  `FT_Glyph_To_Bitmap`/`FT_Outline_Glyph_To_Bitmap` normal gray render.
  Adding a mono-target shortcut in `imagingft.rs` would be false parity; the fix
  belongs at the lower stroke-to-bitmap/rasterization boundary.
- Therefore the next legitimate way to move `imagingft.rs` region coverage is
  to first make the lower stroked mono-target bitmap render exact, then add the
  public Font input-only rows back and let the live Pillow 12.2.0 oracle drive
  expected output. Do not add duplicate rows for the static error-table or LLVM
  source-map markers.
- This lower route is now tracked directly in
  `pillow-rs-freetype/tests/fixtures/inputs/public-api/ftglyph.FT_Glyph_To_Bitmap.json`
  as `pending_stroked_mono_target_outline_to_bitmap`; it is intentionally a
  pending route, not a mocked pass, until the C/Rust bitmap bytes can be made
  exact. The route is no longer blocked by oracle plumbing: local diagnostics
  show the remaining mismatch is lower closed round-path stroke geometry/export
  for the mono-target outline. Temporarily bypassing the
  `stroker_used_unverified_closed_round_path` guard makes the general stroker
  output share the C prefix and suffix but still diverge through the middle
  coverage bytes, so promoting that path would be false parity.
- Current promoted-route diagnostic at `02c6dc0c7`: temporarily removing only
  the pending-route classification makes
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap`
  compare 10 rows and fail exactly one row:
  `ftglyph.FT_Glyph_To_Bitmap.pending_stroked_mono_target_outline_to_bitmap`
  on `rust ffi:field:/bitmap/buffer_hex`. The route audit moves from 180 to
  179 pending rows and from 4842 to 4843 real-parity rows for that temporary
  run, proving the native C oracle and Rust/C/WASM runners execute the intended
  sequence. The only promoted failure remains bitmap coverage bytes, so the
  classification must stay pending until lower stroke geometry/render parity is
  fixed.
- Follow-up lower cleanup: the maintained DejaVu glyph-36 fallback now matches
  the exact `FT_LOAD_NO_BITMAP` source outline points/tags/contours instead of
  only checking point count and contour ends. Oracle inspection shows the
  blocker `FT_LOAD_TARGET_MONO` source outline has different coordinates
  (`(512,1014), (279,384), ...`) from the maintained normal source outline
  (`(525,990), (320,384), ...`). This prevents the normal-stroke fallback from
  silently serving mono-target source geometry. Focused verification still
  passes for the maintained rows:
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke` and
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap`.
- Current post-cleanup diagnostics: promoting only
  `pending_stroked_mono_target_outline_to_bitmap` now fails on Rust status
  `FT_Err_Unimplemented_Feature` (`7`), because the normal glyph-36 fallback no
  longer masks the mono-target source outline. Temporarily bypassing only the
  unverified closed/conic guard reaches bitmap output but still fails
  `/bitmap/buffer_hex`, with matching prefix/suffix and divergence through the
  middle coverage bytes. Reusing the maintained `FT_Glyph_Stroke` outline
  comparison temporarily with `FT_LOAD_TARGET_MONO` and the guard bypass fails
  before rasterization at `/cbox/xMax` (`expected=1139`, `actual=1125`). The
  next real fix is therefore lower stroked outline geometry/export, not
  `imagingft.rs` allocation, offset, or mode handling.
- Current commit coverage confirmation: Coverage MCP run
  `7376f5b8-9f4f-4a83-aa15-7a94efc926d2` passed and ingested snapshot
  `23fab2f2-78d7-4910-9a32-14d72c712804` for commit `8a6cd50ef`. Direct
  `imagingft.rs` coverage remains `1664/1686` lines, `249/254` branches,
  `162/173` functions, and `2608/2700` regions. The remaining reported source
  lines are still `91`, `253`, `271`, `796`, `826`, `829`, and `928`.
  Re-inspection against FreeType 2.14.3 `ftstroke.c` confirms the only real
  behavior blocker is lower `FT_Stroker_EndSubPath`/border export for closed
  round/conic paths; the Rust lower layer intentionally guards that path with
  `closed_round_path_unverified`/`curve_path_unverified`. This is a valid
  implementation gap if it is fixed by porting the exact lower stroker
  geometry/export behavior, but it is not a reason to chase 100%
  `pillow-rs-freetype` coverage or to move stroke math into `imagingft.rs`.
- Lower stroker progress after `8a6cd50ef`: `StrokeBorderState::close` now
  mirrors FreeType 2.14.3 `src/base/ftstroke.c:374-408` by reversing the
  closed-border interior range through the final interior point inclusively.
  Rust previously used an exclusive Rust range and left that point/tag in the
  wrong order when closing reversed borders. Maintained verification passes:
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_Export`,
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap`,
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker`,
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke`, and
  `make -C pillow-rs font-tests`. A temporary diagnostic that promoted only
  `pending_stroked_mono_target_outline_to_bitmap` and bypassed only the
  unverified closed/conic guard still fails on `/bitmap/buffer_hex`, but the
  actual bytes changed materially compared with the prior bypass run. Keep the
  pending row and guard: the inclusive reversal is a real lower C-alignment fix,
  not complete mono-target stroked bitmap parity.
- Coverage MCP run `0621f8e2-0ff7-431c-a546-4d6e70c564c6` passed and ingested
  snapshot `8cacca94-080d-4491-98d3-2e35cd9882fd` for commit `e67a98ec`.
  Direct `imagingft.rs` coverage remains `1664/1686` lines, `249/254`
  branches, `162/173` functions, and `2608/2700` regions. The touched lower
  reverse-close branch is exercised in `pillow-rs-freetype/src/ffi/handles.rs`
  (`3584`, `3587`, and `3588` each hit 11 times in the Font-with-FreeType
  suite), but this lower progress cannot move `imagingft.rs` until the
  mono-target stroked bitmap row is promotable without bypassing the
  `closed_round_path_unverified`/`curve_path_unverified` guard.
- Lower stroker progress after `acc8040f1`: `FT_Stroker_BeginSubPath` now
  mirrors FreeType 2.14.3 `src/base/ftstroke.c:1765-1795` by resetting
  `angle_in` to zero at every new subpath. Rust previously preserved the prior
  contour's exit angle, which can leak into the first conic/line corner of the
  next contour. Maintained verification passes:
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker`,
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap`, and
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke`.
  A temporary diagnostic that promoted only
  `pending_stroked_mono_target_outline_to_bitmap` and bypassed only the
  unverified closed/conic guard still fails exactly one row on
  `/bitmap/buffer_hex`: 10 rows compared, 9 passed, 1 failed. This confirms the
  lower mono-target route has advanced past status/oracle plumbing but still
  needs exact lower stroked-outline geometry/export before any new public
  `imagingft.rs` row can honestly improve region coverage.
- Coverage MCP run `976d18fd-5c20-42d6-912f-61470f9a7f37` passed and ingested
  snapshot `e0784711-576a-4d48-a0cf-77475f6d452c` for commit `e952de545`.
  Direct `imagingft.rs` coverage is unchanged at `1664/1686` lines,
  `249/254` branches, `162/173` functions, and `2608/2700` regions. The
  remaining reported lines are still `91`, `253`, `271`, `796`, `826`, `829`,
  and `928`; source context classifies them as constructor return
  instrumentation, static FreeType error-table data, comment/helper boundary
  markers, and function-signature instrumentation rather than a currently known
  adapter-owned Pillow `_imagingft.c` behavior gap.
- Native-oracle tracing after `36e8e16fd` confirms the first divergence for
  `pending_stroked_mono_target_outline_to_bitmap` occurs before rasterization:
  pinned FreeType's `FT_Glyph_Stroke` result has cbox
  `(-101,-96,1125,1248)`, `72` points, and contours `[2,34,55,71]`. The
  current lower Rust route, with only the pending-route guard bypassed for
  diagnosis, has the same cbox but only `58` points and contours
  `[7,23,25,57]`; its later bitmap failure is downstream of that outline
  topology mismatch. This restores the blocker classification to lower
  closed-round/conic stroker geometry/export, not gray-rasterizer behavior and
  not `imagingft.rs`.
- Follow-up border tracing narrows the topology mismatch further: native C has
  left border `35` points / `2` contours with ends `[2,34]` and right border
  `37` points / `2` contours with ends `[20,36]`. The current Rust lower route
  has left border `24` points / `2` contours with ends `[7,23]` and right border
  `34` points / `2` contours with ends `[1,33]`. Rust is therefore missing
  `11` left-border points and `3` right-border points before combined export;
  the next first-divergence trace should focus inside conic subdivision/corner
  insertion before `ft_stroke_border_close`, not on the public
  `FT_Glyph_To_Bitmap` wrapper.
- Per-source-contour tracing confirms the divergence starts in the first source
  contour. After contour `0`, native C has left/right borders `3/1` and `21/1`,
  while Rust has `3/1` and `18/1`; the first missing points are on the right
  border during the first contour's closed round/conic processing. After
  contour `1`, native C reaches `35/2` and `37/2`, while Rust reaches `24/2`
  and `34/2`. This narrows the first source-level target to the contour-0
  outside-corner / `ft_stroker_arcto` / conic subdivision path.
- Lower stroker progress after `f6d0fd6c8`: `FT_Stroker_CubicTo` now has the
  FreeType 2.14.3 no-wide-stroke cubic stack route in
  `pillow-rs-freetype/src/ffi/handles.rs`. The Rust path mirrors
  `src/base/ftstroke.c:156-292` for cubic splitting, angle mean, and small-arc
  classification, and `src/base/ftstroke.c:1579-1757` for sub-arc dispatch,
  round-corner insertion, and border cubic emission. The broad glyph export
  guard was renamed from `conic_path_unverified` to `curve_path_unverified`
  because both generic conic and cubic routes still need broader outline-export
  proof before replacing the maintained glyph-level fallbacks. Maintained
  verification passes:
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_CubicTo`,
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker`, and
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap`.
  The mono-target stroked bitmap row remains pending; this cubic port is real
  lower FreeType alignment, not complete ImageFont stroke parity.
- Coverage MCP run `913a7464-ea14-46f6-ab16-cb4cbd612596` passed and ingested
  snapshot `9520c181-560b-4eb9-9893-915a7dbd9438` for commit `1af634793`.
  Direct `imagingft.rs` coverage remains `1664/1686` lines, `249/254`
  branches, `162/173` functions, and `2608/2700` regions. The Font-with-FreeType
  suite does not hit the new lower cubic route yet; that is expected because no
  active public ImageFont row depends on a cubic stroked outline. Treat the cubic
  port as lower implementation progress proven by focused pinned-C FreeType
  lanes, not as public ImageFont coverage closure.
- Diagnostic attempt after `8b6381bfe`: adding a non-canonical
  `FT_Stroker_CubicTo` row with start `(0,0)`, controls `(128,512)` and
  `(512,704)`, destination `(704,64)`, radius `96`, and round join exposed a
  real lower FreeType gap. Before any closure experiment, Rust returned
  `status_sequence=[0,0,7]` at `EndSubPath` and exported no outline while C
  returned `52` points, `2` contours, and cbox `(-100,-96,740,576)`. A minimal
  local experiment that recorded curve subpath start and allowed the existing
  round-path closer to run changed Rust to `status_sequence=[0,0,0]`, `52`
  points, and `2` contours, but the outline geometry/order still differed
  materially (`xMax=804` vs C `740`, contour split `[20,51]` vs C `[30,51]`).
  This row was not kept active because it would create a failing lower
  FreeType lane. The finding is: generic cubic stroker export is not C-exact
  yet, and fixing it belongs in `pillow-rs-freetype` only if a real public
  ImageFont/imagingft path needs cubic stroked-outline behavior. It is not a
  current `imagingft.rs` region-coverage closure.
- Coverage MCP run `d66dc67c-9dd9-475e-8199-6714399508b5` passed and ingested
  snapshot `2767a61f-652e-4883-bd40-1f0b17b86e39` for commit `8c5f6e60f`.
  Current `imagingft.rs` coverage remains `1664/1686` lines, `249/254`
  branches, `162/173` functions, and `2608/2700` regions. The reported gaps
  are still lines `91`, `253`, `271`, `796`, `826`, `829`, and `928`.
  Source-context review classifies them as follows: line `91` is the successful
  `ImageFont` constructor return with one unhit compiler branch; lines `253`
  and `271` are entries inside the static Pillow/FreeType error-message table;
  line `796` is a section/comment boundary mapped with branch metadata by
  llvm-cov; lines `826` and `829` are helper/function-boundary mappings around
  `floor26`; line `928` is the `bbox_from_run_with_flags` function signature.
  None of these seven ranges currently identifies a missing Pillow
  `_imagingft.c` public behavior by itself. The remaining non-100% region
  count is therefore coverage-mapping noise plus static table data unless a
  new Pillow/ImageFont behavior gap is found by reverse API comparison.
- Follow-up libraqm audit after `e64ec2f4b`: direct `ImageFont` methods already
  route `direction`, `features`, and `language` through
  `PilError::UnsupportedLibraqm`, but the Python `ImageDraw` facade accepted
  the same libraqm-dependent arguments and dropped them before calling Rust
  draw text. The fix adds an options-aware `Draw::text_with_options` path that
  delegates to `ImageFont::getmask2_with_options`, so no-libraqm validation is
  owned by the same core `ImageFontTextOptions` route. PyO3 `ImageDraw.text`,
  `multiline_text`, `textbbox`, `textlength`, and `multiline_textbbox` now
  forward `direction`/`features`/`language` into core, and the Python facade
  passes those arguments through instead of silently drawing BASIC text.
  `pillow-rs/tests/font_public_api.rs` now guards this source contract.
