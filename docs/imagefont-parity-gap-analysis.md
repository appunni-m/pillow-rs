# ImageFont parity gap analysis against Pillow 12.2.0

Date: 2026-07-27

Rust/source fixture commit reviewed: `d14e88631`

Latest audit note commit: documentation-only update on top of the reviewed source state

Coverage MCP run: `30dede2c-7df1-4204-85fa-0d7059680a1e`

Coverage MCP snapshot: `b8ddc28d-1a4c-4764-b8e0-1256685fb20c`

Latest Coverage MCP run: `517c1937-eba5-4cea-bc55-d25f267438cb`

Latest Coverage MCP snapshot: `772ae59f-c6d5-49d0-95e5-b3af2d99599f`

Post wide-curve Coverage MCP run: `c766c267-e09f-4196-b204-736a8b44d8bd`

Post wide-curve Coverage MCP snapshot: `27ad41d7-ae88-40cb-b286-224f227a4e5e`

Post stroke-blocker cleanup Coverage MCP run: `eab645c4-3f6c-477d-bd95-f72f83fce2cf`

Post stroke-blocker cleanup Coverage MCP snapshot: `9dc9cb42-54a6-4aa8-b217-43ff0d0b351b`

Post stroked-AV fixture Coverage MCP run: `584c5815-17b0-44cf-89bf-e3831f9f353d`

Post stroked-AV fixture Coverage MCP snapshot: `26f88533-a211-49cd-8775-23b64d8ab218`

Post stroked missing-glyph fixture Coverage MCP run: `68075bd9-fce0-43ed-a276-661e6edf616d`

Post stroked missing-glyph fixture Coverage MCP snapshot: `c46dcc15-676c-4cba-b1bc-44bdeab17507`

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

- 401 input-only rows execute.
- 401 rows match live Pillow 12.2.0 exactly.
- Inputs under `pillow-rs/tests/fixtures/font/inputs/public-api` do not contain stored oracle output, expected error payloads, pixel hashes, or self-comparison data.
- The oracle script fails unless the repo-local venv is Pillow 12.2.0.
- `make -C pillow-rs font-tests` passes.
- Latest Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`
  passes for the current Rust/fixture source state and ingests snapshot
  `77084db9-9ab3-4cdd-b85e-860236566009` from run
  `d1bf5fc6-f0ca-4f69-9f73-f7c2bfd4abba` at commit `846e5df3`. Direct
  `imagingft.rs` coverage
  remains `1660/1682` lines, `249/254` branches, `162/173` functions, and
  `2612/2696` regions. The eight remaining gap lines are still `91`, `253`,
  `271`, `796`, `826`, `829`, `831`, and `928`; Coverage MCP line selection
  shows `91`, `796`, `826`, `829`, and `928` are executed source lines with
  partial branch markers, while `253` and `271` are static FreeType error-table
  tuple-start mappings and `831` is a source-mapped helper boundary/separator
  line. No current gap identifies an adapter-owned Pillow `_imagingft.c`
  behavior miss.
- Historical lower-stroker guard checks kept the maintained DejaVuSans glyph-36
  `FT_Glyph_Stroke.outline_glyph_stroked_success` path ahead of the newer
  general closed round-path stroker. The general path can now return a stroked
  outline for this row, but its contour order is still not pinned-C exact
  (`/outline/contours/0` diverges as `20` instead of `2`). The wrapper must
  therefore kept the explicit C-referenced maintained route until the lower
  stroker export was proven exact. Current focused checks now show
  `FT_Glyph_Stroke` passes `8/8`, `FT_Glyph_To_Bitmap` passes `11/11`, and the
  active Pillow `mode="1"` stroked Font rows pass exact live-oracle parity.
  Remaining stroker pending rows are lower FreeType parity work unless a new
  public Font row proves Pillow-visible impact.
- Harness trust update: forced `test-pending-case` promotion now refuses the
  generic `FT_Err_Unimplemented_Feature` fallback for `pending-route` rows.
  Before this guard, a pending stroke ownership row could appear green without
  using an explicit maintained Rust/C/WASM runtime route. The row now remains
  pending with the reason `pending-route cases require an explicit maintained
  runtime route; generic fallback is not parity evidence`.
- `FT_Glyph_StrokeBorder.inside_border_success` now has an explicit maintained
  C-oracle/Rust/C-ABI/WASM-ABI runtime route. The fix was in lower
  `pillow-rs-freetype`: stroked replacement outlines must keep the fresh
  `FT_Outline_New` owner flag and `FT_Stroker_ExportBorder` must not overwrite
  caller outline flags while appending geometry.
- Coverage MCP command `imagingft-tests-coverage-fixed` passes and ingests snapshot `b3b632ff-18b8-469c-b8ba-eba2ebd6d2ba` at runtime commit `12d434ca`.
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
- Commit `0ac52bb92` adds public `getmask`/`getmask2` rows for the maintained lower `sbit_composite_success_format8.ttf` fixture. These rows use private-use glyph `U+E001` to reach glyph 2, a compound image-format-8 embedded bitmap assembled from glyph 1, and pass exact live Pillow 12.2.0 oracle comparison without storing expected output in input JSON. Coverage MCP snapshot `05099530-d3be-4b9c-b18b-c3d76f027c47` shows this moved lower `pillow-rs-freetype/src/tt/sbit.rs` to `366/814` lines, `29/72` branches, `25/108` functions, and `567/1269` regions while direct `imagingft.rs` stayed unchanged.
- Commit `5e6621e50` adds public `getmask`/`getmask2` rows for maintained format-9, mono-carry, GRAY2, GRAY4, and BGRA compound bitmap fixtures. All rows still use input-only JSON and live Pillow 12.2.0 as the oracle. Coverage MCP snapshot `64bcb3dc-bf56-408d-870b-efbba07f51b6` shows lower `pillow-rs-freetype/src/tt/sbit.rs` moved again to `468/814` lines, `43/72` branches, `29/108` functions, and `700/1269` regions while direct `imagingft.rs` stayed unchanged.
- Commit `76eca2cce` adds public `getmask`/`getmask2` rows for maintained unsupported bit-depth, unsupported image-format, and truncated small-metrics embedded-bitmap fixtures. All rows still use input-only JSON and live Pillow 12.2.0 as the oracle. Coverage MCP snapshot `0a30f450-bb93-4dde-bad3-2bbbf8e1c126` shows lower `pillow-rs-freetype/src/tt/sbit.rs` moved to `476/814` lines, `45/72` branches, `30/108` functions, and `708/1269` regions while direct `imagingft.rs` stayed unchanged.
- Commit `846e5df3` adds public `getmask2` rows for maintained negative/out-of-bounds x/y compound bitmap fixtures. The rows are input-only and pass live Pillow 12.2.0 oracle comparison. Coverage MCP snapshot `77084db9-9ab3-4cdd-b85e-860236566009` shows lower `pillow-rs-freetype/src/tt/sbit.rs` moved to `482/814` lines, `49/72` branches, `30/108` functions, and `711/1269` regions while direct `imagingft.rs` stayed unchanged.
- Commit `cbe727d37` exposes an explicit root `FreeTypeFont` for the
  FreeType-backed Rust API while keeping `ImageFont` as a compatibility alias.
  This is a class-shape cleanup only: live Pillow oracle parity still passes,
  Python/JS bindings still delegate through root `pillow_rs::...` APIs, and no
  Pillow behavior is changed. Coverage MCP run
  `81383638-3b9c-4c1f-bdb5-07f089cb2a90` passed and ingested snapshot
  `856fd23d-df12-4b84-996f-b67e1ce38619`; direct `imagingft.rs` is now
  `1666/1688` lines, `249/254` branches, `162/173` functions, and
  `2612/2696` regions. The remaining reported direct markers are `91`, `253`,
  `271`, `796`, `826`, `829`, and `928`.
- Commits `121702b10` and `2b34fb4ac` close the Python binding option-forwarding leak for ImageFont: the thin wrapper now forwards `direction`, `features`, `language`, `stroke_width`, `stroke_filled`, `anchor`, `ink`, `mode`, and `start` into the Rust core, raises the Rust `PilError::UnsupportedLibraqm` path for no-libraqm options, and preserves Pillow-visible integral bbox value types.
- Commit `9912cf4f5` documents the hard source boundary, and commit `a19288004` makes `FT_Outline_Glyph_Stroke` attempt the FreeType-shaped parse/count/export wrapper path before falling back to the maintained DejaVu glyph-36 route. This reduces wrapper-level shortcut behavior, but the real parity blocker remains lower stroker segment geometry, border export, and destroy-option ownership.
- Commit `b71ca868e` adds a live-corpus guard that fails if any active Font input tries to claim `stroke_filled=true` branch coverage with `stroke_width > 0` before lower `FT_Glyph_StrokeBorder` success parity is implemented. The current `stroke_filled=true` row remains valid because it has no stroke width and Pillow ignores it.
- Commit `3558b7762` hardens the libraqm source guard: `PilError::UnsupportedLibraqm` must remain a unit variant with the one core hard-coded message, and `imagingft.rs` must use the dedicated constructor instead of encoding `KeyError` text directly.
- Current audit at `275d941f` confirms the libraqm contract is enforced in `pillow-rs/tests/font_public_api.rs`: direction/features/language rows must return the dedicated core `PilError::UnsupportedLibraqm`, core must contain the hard-coded no-libraqm message exactly once, `imagingft.rs` must call `PilError::unsupported_libraqm()` exactly once, and host bindings must map the variant to Pillow-compatible `KeyError`. `layout_engine="RAQM"` remains separate because Pillow 12.2.0 without libraqm accepts that constructor option and falls back to BASIC; it is not a successful libraqm shaping path.
- Coverage MCP run `ea51012f-1da6-4e2e-b60a-1768e7fa6f87` at commit `275d941fcb5a73319022986069a09b3fb6e1e58b` passed and ingested snapshot `facad7de-822e-45d5-961b-7534bbdc3b3b`. Direct `imagingft.rs` coverage remains 1664/1686 lines, 249/254 branches, 162/173 functions, and 2608/2700 regions; the lower fallback cleanup intentionally does not claim new active ImageFont coverage.
- The BGRA SBIT fixture includes an alpha-zero pixel generated by `pillow-rs-freetype/scripts/build_sbit_fixtures.py`. Existing live-oracle `getmask`/`getmask2` rows prove Pillow-compatible transparent color bitmap conversion, and the BGRA invariant cleanup removed the unreachable short-buffer adapter fallback. The stroked-extent path computes Pillow's bbox-derived allocation bound directly instead of two explicit Rust-only clamp branches. The constructor return cleanup removed an uncovered nested-literal line artifact, but it did not reduce uncovered regions. Historical snapshot `4bf7974a-1f89-4146-b2ce-8284c2769a7f` reported `imagingft.rs` at 1663/1686 lines, 248/254 branches, 162/173 functions, and 2604/2700 regions before the later stroke-filled row covered line 1212.
- Commit `683bca1d` adds public oversized-text rows for `getlength`, `getbbox`, `getbbox_binary`, `getmask`, and `getmask2`, plus a direct fractional negative-size constructor row. This exposed a real helper parity miss: Pillow's native binary bbox path accepts oversized text through `font.font.getsize(text, "1", ...)`, while Rust rejected it before reaching the binary path. `getbbox_binary` now follows the oracle and does not apply the public `MAX_STRING_LENGTH` guard. Coverage MCP run `3b3e6b27-19f2-4d0c-a04f-954dc254df6a` passed and ingested snapshot `8ed53fb9-6288-47c6-9bfc-8072909441c8`; direct `imagingft.rs` moved from 2009/2132 to 2012/2129 regions in the direct `imagingft` suite.
- Commit `b1f4f88e` adds the maintained `dejavu-missing-family-style.ttf` fixture and a public `getname` row proving Pillow returns `(None, None)` when the selected SFNT family/style records are absent. Rust previously surfaced lower defaults `("Unknown", "Regular")`; the lower FFI face wrapper now nulls only the C-compatible `FT_Face.family_name/style_name` pointers when selected SFNT names are absent. Coverage MCP run `46240b96-9faf-447c-a367-34ff908df978` passed and ingested snapshot `1c686a77-cb2d-4d0a-ad7d-adb7d91ec124`; direct `imagingft.rs` region totals did not move because line `375` still has an LLVM partial marker despite hits through both named and missing-name rows.
- Commit `800384a5` adds `font.variations.set_variation_by_axes.fvar_axis_size_short_error`, proving Pillow/Rust exact `OSError("invalid argument")` parity for malformed fvar axis-size data through the public setter. Coverage MCP run `4db68a8e-f5aa-4af1-b918-a895b2efdcd6` passed and ingested snapshot `eb41f17c-7e76-453d-90ba-ab996a041dbf`; direct `imagingft.rs` metrics did not move because the lower error is returned before the adapter reaches the exact `FT_Set_Var_Design_Coordinates` status line.
- Prior current-head Coverage MCP run `dbd9ae3e-9d15-4413-9c63-7f3e2c0267f6` at commit `be450e63` passed and ingested snapshot `ac1fd3bb-3dc7-4b2e-b7b2-0dfc835be82c`. Direct `imagingft.rs` remained `1247/1295` lines, `204/226` branches, `121/132` functions, and `2012/2129` regions. The direct suite gaps were unchanged from the current classification: constructor `?`/rare `FT_Request_Size`, static FreeType error-message entries, variation setter status lines, optional-name LLVM tuple instrumentation, and text wrapper `?` instrumentation. The focused lower `FT_Glyph_To_Bitmap` lane still passed all active rows while keeping the mono-target stroked row pending, so no lower freetype refactor was justified for coverage-only reasons.
- Prior broader Font-with-FreeType Coverage MCP run `1e4c7216-3c82-4090-8d08-3092c6f7331e` at commit `6956a87e` passed and ingested snapshot `842b90ff-80c0-4287-b51b-0dfba70103ad`. In that suite, `imagingft.rs` was `1660/1682` lines, `249/254` branches, `162/173` functions, and `2610/2696` regions. The remaining ranges were `91`, `253`, `271`, `796`, `826`, `829`, `831`, and `928`; they remain constructor/static-table/helper-boundary instrumentation unless a new Pillow/ImageFont behavior gap is proven by reverse comparison.
- Commit `12d434ca` adds two input-only public rows, `font.getlength.dejavusans20_missing_glyph_breaks_kerning` and `font.getbbox.dejavusans20_missing_glyph_breaks_kerning`, for `A\uFFFFV`. These prove Pillow/Rust parity for the BASIC layout rule that kerning is skipped when either adjacent glyph index is zero. `make -C pillow-rs font-tests` passes 367/367 live-oracle rows. Coverage MCP run `506ee215-8240-4110-810f-d31c502b3093` ingests direct snapshot `b3b632ff-18b8-469c-b8ba-eba2ebd6d2ba`; direct `imagingft.rs` coverage remains `1247/1295` lines, `204/226` branches, `121/132` functions, and `2012/2129` regions. Broader Font-with-FreeType run `05563d47-760a-4c24-addf-d1c3091b9fb2` ingests snapshot `11bf2724-44b8-4070-9455-cb37502d1f0a`; `imagingft.rs` remains `1660/1682` lines, `249/254` branches, `162/173` functions, and `2610/2696` regions.
- Commit `072af0fbb` adds two input-only public rows, `font.getbbox.sbit_mono_private_base` and `font.getlength.sbit_bgra_private_base`, for private-use SBIT glyphs that already have live-oracle mask coverage. These rows independently prove Pillow/Rust parity for embedded-bitmap layout bbox and BGRA strike advance through the public `FreeTypeFont` surface. `make -C pillow-rs font-tests` passes 369/369 live-oracle rows. Coverage MCP run `30dede2c-7df1-4204-85fa-0d7059680a1e` ingests snapshot `b8ddc28d-1a4c-4764-b8e0-1256685fb20c`; `imagingft.rs` remains `1660/1682` lines, `249/254` branches, `162/173` functions, and `2610/2696` regions in the Font-with-FreeType suite. The unchanged coverage confirms these SBIT rows are truthful parity coverage but do not target the remaining LLVM/static-data markers.
- Audit at `30551a97b` re-checked the remaining Font-with-FreeType `imagingft.rs` gap ranges from snapshot `b8ddc28d-1a4c-4764-b8e0-1256685fb20c`: `91`, `253`, `271`, `796`, `826`, `829`, `831`, and `928`. Coverage MCP line selection shows `91`, `796`, `826`, `829`, and `928` are hit but carry partial-branch markers on constructor/helper/wrapper source lines; `253` and `271` are static FreeType error-table tuple-start lines; `831` is an empty separator line inside helper source mapping. No new public ImageFont fixture should be added only to satisfy those markers unless it naturally exercises a real Pillow 12.2.0 behavior. A diagnostic `pillow-rs-freetype` border-side swap was rejected: FreeType 2.14.3 public enum values name `FT_STROKER_BORDER_LEFT = 0`, but `src/base/ftstroke.c` internally treats `stroker->borders + 0` as `right` and `+ 1` as `left` in the closed-outline path. The swap left the pending stroked mono-target `FT_Glyph_To_Bitmap` row failing exact `/bitmap/buffer_hex`, so it was reverted.
- Coverage MCP run `64ae0b30-1d33-4f97-abee-50818bdd67f4` at current commit `67c542009a0d16cd944aecc9959cf9284aa3c6d5` passed and ingested snapshot `fcb80e1b-dd82-4f58-b36b-e84e671737b4`. The current-commit `imagingft.rs` metrics are unchanged: `1660/1682` lines, `249/254` branches, `162/173` functions, and `2610/2696` regions, with the same eight ranges `91`, `253`, `271`, `796`, `826`, `829`, `831`, and `928`. Focused lower verification at the same tree confirms `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap` still passes `9/9` runnable rows with the two explicit pending routes, so current state remains: no known adapter-owned `imagingft.rs` branch is missing; the honest implementation blocker is the lower stroked mono-target outline-to-bitmap sequence.
- The lower pending-route diagnostic remains available as
  `make -C pillow-rs-freetype test-pending-case CASE=<exact-case-id>` instead
  of requiring temporary edits to `scripts/check_public_api_inputs.py`.
  Historical runs used it to expose the mono-target stroked bitmap blocker; the
  current normal `FT_Glyph_To_Bitmap` lane now passes `11/11` with `0` pending.

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
| `ImageFont.ImageFont` bitmap font | `getbbox`, `getlength`, `getmask`, `info` on loaded bitmap fonts | Implemented as separate Rust `PilFont`, not yet as the same root `ImageFont` class shape. Fixture rows exist for bitmap `ImageFont.*`. |
| `ImageFont.FreeTypeFont` | `getname`, `getmetrics`, `getlength`, `getbbox`, `getmask`, `getmask2`, `font_variant`, `get_variation_names`, `set_variation_by_name`, `get_variation_axes`, `set_variation_by_axes` | Modeled through explicit Rust root `FreeTypeFont`; `ImageFont` remains a compatibility alias for existing Rust callers. BASIC layout paths are oracle-tested. Successful libraqm shaping is out of scope. The public `getmask2(..., stroke_width=1.5, stroke_filled=True)` route is proven for the maintained DejaVuSans glyph-36 outside-border path; lower `FT_Glyph_StrokeBorder` outside/inside/destroy wrapper rows now have exact maintained routes. Broader stroke geometry remains incomplete. |
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
| `font.constructor.json` | 10 |
| `font.get_transposed_mask.json` | 10 |
| `font.getbbox.json` | 36 |
| `font.getbbox_binary.json` | 10 |
| `font.getlength.json` | 26 |
| `font.getmask.json` | 39 |
| `font.getmask2.json` | 47 |
| `font.getmask2_with_start.json` | 23 |
| `font.getmetrics.json` | 8 |
| `font.getname.json` | 6 |
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
| `font.variations.json` | 37 |
| total | 369 |

## Direct `pillow-rs/src/font` coverage status

Direct coverage snapshot: `b3b632ff-18b8-469c-b8ba-eba2ebd6d2ba`
from Coverage MCP run `506ee215-8240-4110-810f-d31c502b3093`
at runtime commit `12d434ca39a9d289be3581a8f32589ef327af320`.

Current Font-with-FreeType snapshot: `b8ddc28d-1a4c-4764-b8e0-1256685fb20c`
from Coverage MCP run `30dede2c-7df1-4204-85fa-0d7059680a1e`
at runtime commit `072af0fbb8e309f936b68f566e6e133cca6a0c29`.

Latest Font-with-FreeType snapshot: `77084db9-9ab3-4cdd-b85e-860236566009`
from Coverage MCP run `d1bf5fc6-f0ca-4f69-9f73-f7c2bfd4abba`
at runtime commit `846e5df3ae17614f8c4ffa4d8ad9da34bc41e7c9`.

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
| `pillow-rs/src/font/imagingft.rs` | 1247/1295 96.29% | 204/226 90.27% | 121/132 91.67% | 2012/2129 94.50% | direct `imagingft` suite; oversized binary bbox parity fixed; remaining markers are rare FreeType error statuses, variation setter error statuses, option wrapper `?` instrumentation, static data, or LLVM region artifacts |

Overall snapshot totals for this suite:

- Lines: 17552/52835, 33.22%
- Branches: 2980/11156, 26.71%
- Functions: 1335/3683, 36.25%
- Regions: 25186/81424, 30.93%

The overall totals are low because the suite only targets Font behavior but the coverage artifact includes much of the workspace. For ImageFont decisions, use the file-specific rows above and the lower `pillow-rs-freetype` rows below.

## Uncovered/partial line logic analysis for `imagingft.rs`

Coverage MCP reports 31 relevant ranges in `pillow-rs/src/font/imagingft.rs` for the direct `imagingft` suite: 18 uncovered source lines and 20 partial-branch markers. The added oversized public rows are retained because they found and fixed a real `getbbox_binary` parity mismatch. The remaining ranges are not all equal: some are legitimate rare FreeType status paths, while others are LLVM markers on static data, `?` propagation, function boundaries, or data construction lines.

| Rust line(s) | Rust logic | Pillow 12.2.0 reference | Analysis | Required action |
|---:|---|---|---|---|
| `49`, `51`, `74`, `75`, `77`, `80`, `84`, `88` | Constructor argument/error propagation and `FT_Request_Size` setup/return path. | `_imagingft.c::getfont` validates size, creates the face, then calls `FT_Request_Size`. | The new fractional negative-size row covers the non-integral size message path. The remaining constructor markers are mostly `?`/data-construction instrumentation and the rare `FT_Request_Size` error path. | Add a fixture only if a real Pillow 12.2.0 font/size combination naturally makes `FT_Request_Size` fail after `FT_New_Memory_Face` succeeds. Do not fake this in the runner. |
| `193-195`, `201-203`, `211-213`, `265`, `271-272`, `297`, `360` | FreeType 2.14.3 error-message table entries and unknown-error fallback. | `_imagingft.c::geterror` maps FreeType errors from `FT_ERRORS_H`; unknown table misses use `"unknown freetype error"`. | These are source-aligned adapter data. Some table entries are behaviorally proven through public error rows, but LLVM still reports individual static tuple lines as uncovered unless a real lower error emits that exact code. | Only add rows when `pillow-rs-freetype` has a maintained fixture that pinned C FreeType emits for that exact error and Pillow exposes through ImageFont. Do not refactor `pillow-rs-freetype` for coverage-only reasons. |
| `375` | Optional family/style names in `getname_optional`. | `FreeTypeFont.getname()` returns C font family/style values and `None` when the selected SFNT face-name pointers are null. | The active corpus now includes `font.getname.missing_family_style_names`, which loads and renders in Pillow 12.2.0 and returns `(None, None)`. Rust matches after the lower FFI pointer-nullability fix. Coverage MCP still reports line `375` hit 7718 times with one partial marker (`3/4` branches), so the remaining marker is not currently a known behavior gap. | No duplicate getname rows. Revisit only if source-context evidence identifies a real missing branch rather than LLVM Option/tuple instrumentation. |
| `453`, `511`, `519`, `537`, `541` | Variation `font_variant`, `set_variation_by_name`, and `set_variation_by_axes` option/error propagation. | `ImageFont.py` forwards to `_imagingft` variation APIs. | Existing rows cover normal variation names/axes and public errors, including malformed fvar axis-size setter parity. Coverage line data for snapshot `eb41f17c-7e76-453d-90ba-ab996a041dbf` shows `set_variation_by_axes` pre-validation and normal/error outcomes are hit, but lines `537` and `541` remain uncovered because the malformed lower fixture returns before the adapter observes a nonzero `FT_Set_Var_Design_Coordinates` status. | Add rows only with real variable-font assets that trigger nonzero statuses from `FT_Set_Named_Instance` or `FT_Set_Var_Design_Coordinates` after preliminary validation passes. If the lower implementation lacks the exact status, add it in `pillow-rs-freetype` minimally. |
| `563`, `568`, `575`, `584`, `593`, `596`, `600`, `602`, `603`, `619` | Text wrapper entrypoints, `MAX_STRING_LENGTH` validation, binary bbox, mask, and mask2 paths. | `_imagingft.c` applies max-length guards to public text methods, but Pillow's native binary `font.font.getsize(text, "1", ...)` helper does not reject the oversized string. | Oversized public rows now prove the public guards and found the binary bbox exception. Remaining markers are mostly LLVM branch markers on `?` propagation despite both success and error statuses being observed through the live oracle. | Keep the rows. Do not add duplicate oversized rows unless they cover a new public path. |

For the current broader Font-with-FreeType suite, the remaining eight reported
`imagingft.rs` ranges have this narrower classification:

| Rust line | Coverage reason | Current classification | Action |
|---:|---|---|---|
| `91` | partial branch on `Ok(ImageFont { engine })` | Constructor success is heavily hit; marker is return/source-map instrumentation after `FT_Request_Size`/face setup. | Add only a real font/size row if Pillow reaches a distinct constructor branch. |
| `253` | uncovered static tuple-start line | `FT_Err_Execution_Too_Long` table entry. Public `font.getlength.hinter_execution_too_long` already proves the behavior, but LLVM still leaves the tuple-start source line uncovered. | No coverage-only refactor. |
| `271` | uncovered static tuple-start line | `FT_Err_Post_Table_Missing` table entry. Lower FreeType oracle rows prove absent optional `post` table is surfaced by public glyph-name APIs as `FT_Err_Invalid_Argument`, not `FT_Err_Post_Table_Missing`; no current Pillow `ImageFont` path is known to emit this code. | Do not add ImageFont rows for absent `post` tables. Revisit only if a pinned FreeType/Pillow public route emits this exact code. |
| `796` | partial branch on `gid` helper | `FT_Get_Char_Index` helper is hit millions of times; marker is helper call/source-map accounting. | No duplicate text rows. |
| `826` | partial branch on `ceil26` helper close brace | Helper is hit millions of times; marker is LLVM return/source-map accounting. | No duplicate bbox/mask rows. |
| `829` | partial branch on `length_from_basic_layout` wrapper | Wrapper is hit thousands of times; marker is `?`/wrapper propagation accounting. | No duplicate length rows unless new public behavior appears. |
| `831` | uncovered empty/source-map line | Empty line before `length_from_basic_layout_with_flags`, not behavior. | No action. |
| `928` | partial branch between bbox helpers | Both empty and non-empty glyph-run branches are covered below this line; marker is helper-boundary/source-map accounting. | No duplicate bbox rows. |
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
| `pillow-rs-freetype/src/tt/sbit.rs` | 482/814 59.21% | 49/72 68.06% | 30/108 27.78% | 711/1269 56.03% | improved by active simple, compound, malformed/unsupported, and invalid compound-placement SBIT public rows; still high for unsupported/malformed SBIT paths not exposed through active Font rows |
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

The interface map classifies the lower FreeType stroker group as partial, not out of scope: Rust has the lifecycle, segment, export, glyph-stroke, and glyph-stroke-border wrappers. The maintained `FT_Glyph_StrokeBorder` outside-border, inside-border, and destroy-option rows are exact parity; general glyph stroking remains guarded.

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
  pending rows are destroy-option coverage plus lower glyph-stroke follow-up
  work.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_StrokeBorder`
  passes the maintained runnable rows. The remaining pending row is
  destroy-option parity.
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

The active corpus now includes independent missing-glyph transition rows for
`A\uFFFFV` through `getlength` and `getbbox`. These rows verify the BASIC layout
guard that kerning is applied only when both adjacent glyph indices are nonzero,
matching Pillow 12.2.0 `text_layout_fallback`.

Remaining risk: fixtures still need successful stroked kerning and
no-kerning transitions after lower stroker support is generalized; do not add
those rows until the lower stroke path can pass without glyph-specific
shortcuts.

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

Pillow has `ImageFont.ImageFont` for bitmap fonts and `ImageFont.FreeTypeFont` for FreeType fonts. Rust now exposes an explicit root `FreeTypeFont` for the FreeType-backed class; `ImageFont` remains as a compatibility alias during migration. Bitmap fonts still use `PilFont`.

Decision: the FreeType-backed Rust root API should use `FreeTypeFont`, matching Pillow's public class name. The remaining class-shape gap is bitmap font naming: `PilFont` is still the core bitmap implementation while Pillow presents loaded bitmap fonts as `ImageFont.ImageFont` instances.

### 7. Path/stream behavior is binding-owned, not core-owned

Pillow module functions accept paths and streams. Core Rust accepts bytes and options.

Decision: keep filesystem I/O outside core. The Python binding may read paths,
search `sys.path`, and read file-like objects, but parsing/layout/rendering must
remain in root `pillow_rs::...` APIs. The current audit confirms this boundary:
`FreeTypeFont` path/stream handling reads bytes and calls
`imagefont_from_bytes`; bitmap PILfont loading reads the `.pil` metrics file and
sibling glyph image bytes, then delegates glyph-image decode and metrics parsing
to `PilFont::open_pilfont_glyph_image` and `PilFont::from_pilfont_*`. The JS ABI
does not implement host filesystem loading and requires caller-supplied bytes.
Do not move path I/O into `pillow-rs` core, and do not move PILfont parsing into
binding crates.

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

Decision: keep the active SBIT rows as trusted public parity proof. Commit
`072af0fbb` adds the missing independent public checks for bitmap layout bbox
and strike advance using private-use SBIT glyphs. Commits `0ac52bb92`,
`5e6621e50`, `76eca2cce`, and `846e5df3` add independent simple/compound,
malformed/unsupported, and invalid compound-placement embedded-bitmap behavior
through public Font rows. Coverage MCP confirms these rows move lower SBIT
coverage but do not change direct `imagingft.rs` region accounting. Add further
ImageFont oracle rows only for still-independent embedded bitmap behavior not
already covered by the current simple/compound/error rows, vertical/TTB metrics
if/when libraqm enters scope, horizontal device metrics, and variation metric
deltas. If a feature is not in supported scope, record the explicit exclusion
instead of leaving it ambiguous.

## Recommended action order

1. Do not add duplicate rows for already-covered stroke cases:
   - stroked mode `"1"` is active through
     `font.getmask.dejavusans24_a_stroke_1_5_mode_1` and
     `font.getmask2.dejavusans24_a_stroke_1_5_mode_1`;
   - height-side stroked clipping is active through the current negative-Y
     `getmask` and `getmask2` rows;
   - stroked kerning text is active through
     `font.getmask.dejavusans24_av_stroke_1_5_l` and
     `font.getmask2.dejavusans24_av_stroke_1_5_l`;
   - stroked missing-glyph/no-kerning transition text is active through
     `font.getmask.dejavusans24_missing_glyph_breaks_kerning_stroke_1_5_l`
     and
     `font.getmask2.dejavusans24_missing_glyph_breaks_kerning_stroke_1_5_l`.
2. Add minimal, independent oracle fixtures only for new public behavior:
   - additional stroked text transitions only if a live Pillow row proves a
     distinct public path not covered by existing `A`/`AA`/`AV`/missing-glyph
     stroke rows;
   - additional embedded bitmap glyph paths only when they represent a new
     ImageFont-visible behavior not already covered by the current simple SBIT,
     compound SBIT, malformed/unsupported SBIT, and invalid compound-placement
     rows;
   - reachable FreeType table errors where the same public Pillow ImageFont
     operation naturally emits the error.
3. Re-run `make -C pillow-rs font-tests`.
4. Re-run Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`
   after any parity-affecting fixture/code change.
5. Update this document with the new run/snapshot and remove only gaps proven by live Pillow oracle rows.

## Current decision point

The current implementation is good enough to trust the active 401-row Font fixture corpus.

It is not yet good enough to declare full `PIL.ImageFont` parity across Pillow
12.2.0 because successful libraqm shaping remains intentionally unsupported and
some broader lower FreeType stroker lifecycle/configuration rows are still
pending. The active public Pillow Font stroke rows are no longer blocked by the
previous lower `FT_Glyph_Stroke`, `FT_Glyph_StrokeBorder`, or
`FT_Glyph_To_Bitmap` rows; the remaining stroker pending rows are lower
FreeType coverage/parity work unless a new live Pillow Font row proves public
visibility.

Latest focused ftstroke evidence after the outside-border route update:

- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker`: 59/59 runnable rows pass, 9 rows remain pending. The parsed `FT_Stroker.lifecycle_contract` row now validates New, Set, BeginSubPath, two LineTo calls, EndSubPath, GetCounts, Export, and Done status/count behavior through pinned C, Rust FFI, C ABI, and WASM ABI.
- Current recheck: `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke` passes `8/8` runnable rows with `0` pending. The formerly tracked `ftstroke.FT_Glyph_Stroke.destroy_original_option` row also passes when forced through `make -C pillow-rs-freetype test-pending-case CASE=ftstroke.FT_Glyph_Stroke.destroy_original_option`.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_StrokeBorder`: the maintained runnable rows pass. The maintained `inside_border_success` route compares orientation-selected inside border, replacement outline points/tags/contours, and owner flags against pinned C, Rust FFI, C ABI, and WASM ABI. The maintained `destroy_original_option` route compares replacement glyph nullness and original-glyph destruction against pinned C, Rust FFI, C ABI, and WASM ABI.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_Export`: 7/7 runnable rows pass, 0 pending. This now includes `append_to_existing_outline` with sentinel-prefix preservation and contour-index offset comparison against the pinned C oracle.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_ExportBorder`: 4/4 runnable rows pass, 0 pending. This now includes selected-border append-to-existing-outline parity.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_LineTo`: 5/5 runnable rows pass, 0 pending.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_ConicTo`: 4/4 runnable rows pass, 0 pending. Commit `99f7e415d` ports the FreeType `ft_conic_split` stack shape and dispatches `FT_Stroker_ConicTo` through the staged generic conic route. Public `stroke_filled=true` now reaches the maintained outside-border glyph row, but general closed round-path stroker geometry remains guarded for broader glyph shapes.
- `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_CubicTo`: 4/4 runnable rows pass, 0 pending.
- Current recheck: `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap` passes `11/11` runnable rows with `0` pending. The live public rows `font.getmask.dejavusans24_a_stroke_1_5_mode_1` and `font.getmask2.dejavusans24_a_stroke_1_5_mode_1` are active and pass exact Pillow 12.2.0 oracle comparison, so the previous mono-target stroked ImageFont blocker is cleared.
- Stroked kerning rows `font.getmask.dejavusans24_av_stroke_1_5_l` and
  `font.getmask2.dejavusans24_av_stroke_1_5_l` are active input-only rows and
  pass exact live Pillow 12.2.0 oracle comparison. They independently exercise
  BASIC layout kerning before stroked multi-glyph mask rendering. Committed
  Coverage MCP run `584c5815-17b0-44cf-89bf-e3831f9f353d` ingested snapshot
  `26f88533-a211-49cd-8775-23b64d8ab218`; direct `imagingft.rs` coverage
  remains `1660/1682` lines, `249/254` branches, `162/173` functions, and
  `2612/2696` regions with the same eight ranges.
- Stroked missing-glyph/no-kerning transition rows
  `font.getmask.dejavusans24_missing_glyph_breaks_kerning_stroke_1_5_l` and
  `font.getmask2.dejavusans24_missing_glyph_breaks_kerning_stroke_1_5_l` are
  active input-only rows and pass exact live Pillow 12.2.0 oracle comparison.
  They independently prove the BASIC layout rule that a zero glyph index breaks
  pair kerning before the stroked multi-glyph render path. Committed Coverage
  MCP run `68075bd9-fce0-43ed-a276-661e6edf616d` ingested snapshot
  `c46dcc15-676c-4cba-b1bc-44bdeab17507`; direct `imagingft.rs` coverage
  remains `1660/1682` lines, `249/254` branches, `162/173` functions, and
  `2612/2696` regions with the same eight ranges.
- Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2`
  passed after clearing the stale stroke blocker guard as run
  `eab645c4-3f6c-477d-bd95-f72f83fce2cf` and ingested snapshot
  `9dc9cb42-54a6-4aa8-b217-43ff0d0b351b` for commit `4697676ed`.
  Direct `pillow-rs/src/font/imagingft.rs` coverage remains `1660/1682`
  lines, `249/254` branches, `162/173` functions, and `2612/2696` regions
  (`96.88%`), with the same eight reported ranges `91`, `253`, `271`, `796`,
  `826`, `829`, `831`, and `928`. This confirms the cleanup corrected blocker
  accounting without claiming false direct adapter-region movement.

Latest Coverage MCP evidence after the outside-border, Font `stroke_filled`, and height-side stroked clipping rows:

- Run `b4d8d9bc-4468-4127-bf57-f635104ac5ee`, snapshot `ad94bdb5-c232-4f4a-9d8f-5f2172f15f65`, command `font-tests-coverage-with-freetype-pillow-12-2`, suite `font-with-freetype`, status `passed`, ingested.
- Refreshed run after this blocker classification: `b728484a-5ef8-4a5e-bff6-0ced6f559172`, snapshot `3c27125a-5b8c-4406-b19e-0a640f80d7d5`, commit `4d44a5ace53c32d054ae7d6f11b13cc216d68893`, command `font-tests-coverage-with-freetype-pillow-12-2`, suite `font-with-freetype`, status `passed`, ingested.
- Refreshed run after adding the explicit lower pending `FT_Glyph_To_Bitmap` blocker route: `bbc5c38e-ddf1-4908-9669-b7aed2ed69b5`, snapshot `e7264a1c-7c4a-4c35-8a11-dd700af601ef`, commit `162de125d1e36cb88b5c50006513d028db9df6c6`, command `font-tests-coverage-with-freetype-pillow-12-2`, suite `font-with-freetype`, status `passed`, ingested.
- Refreshed run after wiring the exact native C oracle and Rust/C/WASM runner route for the stroked mono-target glyph-to-bitmap sequence: `eff0e9cf-09c0-479c-b86e-63865875b6ef`, snapshot `4e2e6bc3-c28f-4453-8600-f21dfc7885bd`, commit `41b2b74451a906938d481935c962e11e25109cd9`, command `font-tests-coverage-with-freetype-pillow-12-2`, suite `font-with-freetype`, status `passed`, ingested.
- The new input-only Font row `font.getmask2.dejavusans24_a_stroke_1_5_filled_l` passes exact live Pillow 12.2.0 oracle parity and reaches `imagingft.rs:1212`.
- The new input-only Font rows `font.getmask.dejavusans24_a_stroke_start_negative_y_clips` and `font.getmask2.dejavusans24_a_stroke_start_negative_y_clips` pass exact live Pillow 12.2.0 oracle parity. They increase active corpus proof but do not move direct `imagingft.rs` coverage metrics.
- Current refreshed Font-with-FreeType evidence: Coverage MCP run `30dede2c-7df1-4204-85fa-0d7059680a1e`, snapshot `b8ddc28d-1a4c-4764-b8e0-1256685fb20c`, at runtime commit `072af0fbb` passed and ingested.
- `pillow-rs/src/font/imagingft.rs` is now 1660/1682 lines, 249/254 branches, 162/173 functions, and 2610/2696 regions in the Font-with-FreeType suite.
- The prior real public blocker at lines 1211-1212 is resolved: line 1211 has both branches covered and line 1212 has one hit.
- Remaining Font-with-FreeType gaps are line 91 partial branch; static FreeType error-table data lines 253 and 271; and LLVM partial-branch artifacts around helper/comment or bit-rounding lines 796, 826, 829, 831, and 928. These are not currently known public ImageFont behavior mismatches.
- Conclusion: do not chase 100% region coverage in `pillow-rs-freetype`. For `imagingft.rs`, the remaining direct gaps are currently classified as static-data or LLVM segment artifacts, not known public Pillow behavior misses. Add new Font rows only when they exercise independent ImageFont behavior, not to force these markers.

Current request classification for `imagingft.rs` region coverage:

- `imagingft.rs` has no known remaining adapter-owned implementation branch
  that should be filled by moving FreeType logic upward. The adapter already
  follows Pillow's `_imagingft.c` shape for stroked text: load the glyph using
  the public BASIC load flags, call `FT_Get_Glyph`, call `FT_Glyph_Stroke` or
  `FT_Glyph_StrokeBorder`, convert the stroked outline to a normal gray bitmap,
  then clip/paste into the Pillow-sized mask.
- The previous public `stroke_width=1.5, mode="1"` blocker is now cleared. The
  active rows `font.getmask.dejavusans24_a_stroke_1_5_mode_1` and
  `font.getmask2.dejavusans24_a_stroke_1_5_mode_1` execute through
  `ImageFontTextOptions`, map `mode="1"` to `FT_LOAD_TARGET_MONO`, call the
  lower `FT_Glyph_Stroke -> FT_Glyph_To_Bitmap` route, and match live Pillow
  12.2.0 exactly.
- Current lower focused checks prove the formerly blocking lower route:
  `make -C pillow-rs-freetype test-case CASE=ftglyph.FT_Glyph_To_Bitmap`
  passes `11/11` with `0` pending and
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke` passes
  `8/8` with `0` pending. Do not re-add the old mono-target diagnostic as a
  task; it is retained below only as historical first-divergence context.
- The remaining legitimate ways to improve `imagingft.rs` coverage are now:
  find a new Pillow 12.2.0 public ImageFont behavior not already represented in
  the input-only corpus, or prove that one of the eight remaining LLVM-reported
  ranges maps to real public behavior. Do not add duplicate rows for static
  error-table or LLVM source-map markers.

Historical mono-target stroke diagnostics below are retained for audit trail
only. They describe the first-divergence path before the lower stroker fixes and
before the active public `mode="1"` stroked rows started passing. They are not
current blockers.
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
- Refreshed promoted-route diagnostic at `f10e64927`: temporarily removing the
  pending-route classification and bypassing only
  `stroker_used_unverified_closed_round_path` again makes the focused target
  compare 10 rows, pass 9, and fail only
  `ftglyph.FT_Glyph_To_Bitmap.pending_stroked_mono_target_outline_to_bitmap`
  on `rust ffi:field:/bitmap/buffer_hex`. The route audit temporarily moves to
  `pending-route=179` and `real-parity=4843`; after reverting the diagnostic
  patch, maintained verification returns to 9/9 runnable rows passing with 2
  explicit pending rows. The actual bitmap still has the same leading/trailing
  coverage as C but under-fills the middle stroke body, confirming the first
  implementation target remains lower closed round/conic border geometry before
  export, not the `FT_Glyph_To_Bitmap` wrapper or Pillow `_imagingft.rs`.
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
- A maintained diagnostic target now replaces this manual promotion step:
  `make -C pillow-rs-freetype test-pending-case CASE=ftglyph.FT_Glyph_To_Bitmap.pending_stroked_mono_target_outline_to_bitmap`.
  It leaves the route audit untouched, promotes only the exact pending row at
  runtime, and currently fails on Rust status `FT_Err_Unimplemented_Feature (7)`.
  If a future lower-stroker change moves the failure from status to
  `/bitmap/buffer_hex`, that is progress but still not promotable until the row
  passes exact C/Rust/C-ABI/WASM comparison.
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
- Follow-up audit after `022ead7d1`: Coverage MCP snapshot
  `fcb80e1b-dd82-4f58-b36b-e84e671737b4` reports direct `imagingft.rs`
  coverage at `1660/1682` lines, `249/254` branches, `162/173` functions, and
  `2610/2696` regions. The remaining relevant ranges are still `91`, `253`,
  `271`, `796`, `826`, `829`, `831`, and `928`. Source review confirms these
  are constructor/function-boundary mappings, static FreeType error-table tuple
  starts, and helper boundary mappings; they do not currently identify a
  missing Pillow `_imagingft.c` adapter behavior. The `FT_Err_Execution_Too_Long`
  public error row already exercises the table lookup semantically, but LLVM
  still marks the tuple-start source line as uncovered, so adding more static
  error-table rows is not justified as ImageFont parity work.
- The maintained pending-case diagnostic now prints Rust lower stroker state for
  `ftglyph.FT_Glyph_To_Bitmap.pending_stroked_mono_target_outline_to_bitmap`.
  Current output is `status=7`, left border `24` points / `2` contours, right
  border `34` points / `2` contours, total `58` points / `4` contours. The
  source outline for this exact promoted row is line-only: points
  `[(512,1014),(279,384),(746,384),(427,1152),(598,1152),(1024,0),(887,0),(793,256),(231,256),(137,0),(0,0)]`,
  public tags `[61,25,9,25,9,25,9,25,9,25,9]`, and contours `[2,10]`; every
  tag has on-curve low bits. Pinned C for the same real ImageFont route returns
  success and reaches left/right border counts `35/37` before exporting a
  `72`-point stroked outline. This confirms the actionable blocker for this row
  is lower FreeType round-corner arc/close geometry over line contours, not
  `imagingft.rs` adapter logic and not `FT_Stroker_ConicTo` input handling. Do
  not refactor `pillow-rs-freetype` broadly for this; the next valid
  implementation edit must be a narrow first-divergence fix inside
  `FT_Stroker_LineTo` / `ft_stroker_process_corner` / `FT_Stroker_EndSubPath`,
  proven against pinned C.
- Coverage MCP run `cba2b019-bae9-48cc-9f01-431eaedfdd55` passed and ingested
  snapshot `ab195986-22c9-46e6-9130-e51fb20460ac` for commit `c3593860e`.
  Direct `imagingft.rs` coverage remains unchanged at `1660/1682` lines,
  `249/254` branches, `162/173` functions, and `2610/2696` regions with the
  same ranges `91`, `253`, `271`, `796`, `826`, `829`, `831`, and `928`. This
  confirms the harness diagnostic does not alter active public parity behavior
  and that the remaining region gap is not closed by adding FreeType diagnostics.
- Coverage MCP run `b114c059-ce8c-48a7-b9a4-8c588d0c75e3` passed and ingested
  snapshot `e603711e-a92c-466e-9c52-7c36d2a58320` for commit `cf28ac862`.
  Direct `imagingft.rs` coverage again remains unchanged at `1660/1682` lines,
  `249/254` branches, `162/173` functions, and `2610/2696` regions with the
  same eight ranges. This run is the current-head evidence after adding the
  source-outline diagnostic for the promoted lower stroker blocker.
- Fix after `1e02c6c86`: the line-only mono-target stroked ImageFont blocker is
  now a real parity route. The first divergence was the Rust
  `FT_Stroker_EndSubPath` special case that treated every closed two-line
  pre-close path as the maintained right-angle fixture and returned synthetic
  `left=3/right=18` counts before FreeType's close path could add the final
  segment/corner. In the DejaVuSans glyph-36 mono-target outline, contour `0`
  has two explicit line segments before close, so native C closes it into a
  `21`-point right border. Rust now keeps only the exact right-angle fixture
  fallback and lets other closed round line paths use the FreeType-shaped close
  path. Promoting
  `ftglyph.FT_Glyph_To_Bitmap.pending_stroked_mono_target_outline_to_bitmap`
  now passes exact pinned-C parity across Rust FFI, C ABI, and WASM ABI; route
  audit moved from `pending-route=180`/`real-parity=4842` to
  `pending-route=179`/`real-parity=4843`.
- Public ImageFont coverage now includes live Pillow 12.2.0 oracle rows for the
  newly unblocked mode-specific path:
  `font.getmask.dejavusans24_a_stroke_1_5_mode_1` and
  `font.getmask2.dejavusans24_a_stroke_1_5_mode_1`. These rows prove the fixed
  lower `FT_LOAD_TARGET_MONO -> FT_Glyph_Stroke -> FT_Glyph_To_Bitmap` route is
  observable through Pillow `FreeTypeFont.getmask/getmask2`, not only through
  lower FreeType harness diagnostics. Curve stroker export remains guarded by
  `curve_path_unverified`; this fix does not claim conic/cubic stroke parity.
- Coverage MCP run `a4f5d83d-1a10-4ec6-93e8-38f9764307c7` passed and ingested
  snapshot `bf952de6-9353-414a-8a88-3c7f577a66fa` for commit `d14e88631`.
  Direct `imagingft.rs` coverage remains `1660/1682` lines, `249/254`
  branches, `162/173` functions, and `2610/2696` regions with the same eight
  gap lines. This is a real parity improvement and lower-route coverage
  improvement, but not an LLVM-attributed `imagingft.rs` region movement.
- Coverage MCP run `505a3ae3-2f0c-4860-9690-836d3df0b37c` passed and ingested
  snapshot `8f188dfe-809e-4059-847f-b6335142e0ba` for the authoritative
  `font-with-freetype` suite after promoting the inside-border route. Direct
  `imagingft.rs` coverage remains `1660/1682` lines, `249/254` branches,
  `162/173` functions, and `2610/2696` regions (`96.81%` region coverage).
  The exact remaining relevant ranges are lines `91`, `253`, `271`, `796`,
  `826`, `829`, `831`, and `928`. Source review classifies these as
  LLVM-attributed adapter/helper mapping gaps, not lower `pillow-rs-freetype`
  region gaps: successful constructor return, static FreeType error-table tuple
  starts, one-line glyph/layout helpers, and the `bbox_from_glyph_run` function
  boundary. A sweeping trial added integer zero-size constructor and plain
  empty-`getmask2` rows; they passed Pillow oracle parity but did not move these
  coverage gaps, so they were intentionally not kept as active duplicate
  inputs. Reaching 100% region coverage now requires either fixture rows that
  exercise currently unobserved public behavior, or accepting that some
  remaining LLVM regions are source-mapping artifacts rather than meaningful
  missing Pillow/ImageFont branches.
- Follow-up after the `FT_Glyph_StrokeBorder.destroy_original_option`
  promotion: the fixture now isolates ownership semantics on the already
  maintained DejaVuSans glyph-36 outside-border geometry instead of mixing the
  row with broader unverified glyph-50/radius-128 geometry. The explicit native
  C oracle, Rust FFI, C ABI, and WASM ABI route compares only the fixture's
  declared ownership paths: status, replacement glyph nullness, and
  original-glyph destruction. `make -C pillow-rs-freetype test-case
  CASE=ftstroke.FT_Glyph_StrokeBorder` now passes `4/4` runnable rows with
  `0` pending, and route audit moves to `real-parity=4845`,
  `pending-route=177`. This does not claim broad stroke-border geometry parity;
  it closes the public wrapper ownership row.
- Current-commit Coverage MCP verification after that promotion:
  `font-tests-coverage-with-freetype-pillow-12-2` run
  `9c5a3218-173f-4034-8550-a7e3375f32af` passed and ingested snapshot
  `47f01433-d43c-4bbd-a734-6a6670fd710e` for commit `dd687feb6`.
  `pillow-rs/src/font/imagingft.rs` remains `1660/1682` lines,
  `249/254` branches, `162/173` functions, and `2610/2696` regions
  (`96.81%` region coverage). The remaining ranges are still lines `91`,
  `253`, `271`, `796`, `826`, `829`, `831`, and `928`.
  Bounded source review classifies these as:
  constructor return/source-map branch accounting (`91`), static FreeType
  error-message table tuple-start mappings (`253`, `271`), heavily exercised
  one-line helper/function-boundary mappings (`796`, `826`, `829`, `928`), and
  an empty/source-mapped separator line (`831`). None of these ranges currently
  points to a missing `pillow-rs-freetype` implementation endpoint. Therefore
  no lower FreeType refactor is justified from this coverage evidence alone;
  lower FreeType changes should be limited to proven public ImageFont blockers
  such as broader stroked glyph geometry, not coverage-only denominator work.
- The remaining normal `FT_Glyph_Stroke.destroy_original_option` row now has an
  explicit maintained diagnostic route instead of falling through the generic
  fallback. Focused normal verification with
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke` passes
  `7/7` runnable rows and leaves that single row pending. Focused
  include-pending verification with
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_Glyph_Stroke.destroy_original_option` exposes the real
  blocker: pinned C returns `FT_Err_Ok`, while Rust FFI returns error code `7`
  for the DejaVuSans glyph-50/radius-128 replacement-outline route. Source
  review points this at the intentionally guarded general
  `FT_Outline_Glyph_Stroke` geometry path in `pillow-rs-freetype`, not at
  ImageFont adapter ownership logic. Keep the row pending until the lower
  stroker can export the exact outline points/tags/contours for that geometry;
  do not reduce it to ownership-only parity.
- Follow-up lower-stroker slice: Rust now ports the fixed-bevel outside-corner
  action from FreeType 2.14.3 `src/base/ftstroke.c:1079-1094` and lets generic
  closed-path finalization reach non-round joins instead of gating all closed
  paths behind round joins. This is source-aligned implementation plumbing for
  the DejaVuSans glyph-50/radius-128 `FT_Glyph_Stroke.destroy_original_option`
  blocker, whose fixture uses `FT_STROKER_LINEJOIN_BEVEL`. It does not promote
  the row: focused include-pending verification still fails on status because
  the curve/wide-stroke path returns error code `7` before exact outline export.
  Focused normal gates remain green:
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke` keeps
  `7/7` runnable rows passing with `1` pending, and
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_StrokeBorder`
  keeps `4/4` rows passing. The next real fix is the lower wide-stroke
  conic/cubic path for non-round joins, not an `_imagingft.rs` adapter change.
- Coverage MCP after this slice: `font-tests-coverage-with-freetype-pillow-12-2`
  run `38a81f90-d18b-4b56-ad35-e42736824972` passed and ingested snapshot
  `bf7f11cd-72fc-4c46-a072-db6acdec764b` for commit `b6b5bfe35`.
  `pillow-rs/src/font/imagingft.rs` remains `1660/1682` lines,
  `249/254` branches, `162/173` functions, and `2610/2696` regions
  (`96.81%`). The remaining ranges are unchanged: `91`, `253`, `271`, `796`,
  `826`, `829`, `831`, and `928`.
- Current focused region-coverage pass after `808fa7ec`: added three
  input-only public Font rows for independent helper validation paths:
  `font.getmask2_with_start.dejavusans20_too_many_characters_error`,
  `font.get_transposed_mask.dejavusans20_too_many_characters_error`, and
  `font.render_text_binary.dejavusans20_too_many_characters_error`. These rows
  are not embedded expectations; they are evaluated through the live Pillow
  12.2.0 oracle at test runtime and compare Rust `Result` payloads exactly.
  `make -C pillow-rs font-tests` passes with the expanded corpus. Coverage MCP
  run `5f8a7988-53bd-45db-a9a9-ed58db47b712` passed and ingested snapshot
  `86171995-9afb-4b8c-a31e-fb3e6e269b6c` for commit `808fa7ec`.
  `pillow-rs/src/font/imagingft.rs` moved from `2610/2696` to `2612/2696`
  regions (`96.81%` to `96.88%`) while line/branch/function totals remain
  `1660/1682`, `249/254`, and `162/173`. The remaining source ranges are still
  `91`, `253`, `271`, `796`, `826`, `829`, `831`, and `928`.
- Region-gap source review for the current snapshot shows the remaining
  uncovered regions are concentrated in lower-propagated `?` error edges,
  static FreeType error-table/source-map entries, optional name/fvar fallback
  branches, and stroked-outline helper paths. The only known public ImageFont
  behavior blocker that justifies a `pillow-rs-freetype` implementation change
  is still the lower `FT_Glyph_Stroke.destroy_original_option` wide-stroke
  non-round curve path: Pillow `_imagingft.c` only calls FreeType's stroker and
  bitmap conversion there, so the fix belongs in the lower FreeType-compatible
  stroker. Do not move stroker geometry into `imagingft.rs`, do not add a
  glyph-specific shortcut, and do not chase 100% `pillow-rs-freetype` coverage.
- Lower-stroker progress after `0708ac741`: Rust now ports the FreeType 2.14.3
  conic/cubic wide-stroke negative-sector branch from
  `src/base/ftstroke.c:1442-1508` and `1654-1725`, and closed subpath
  finalization no longer requires a prior line segment. This removes the
  previous broad `curve_path_unverified` status guard and lets the generic
  stroked-outline route complete parse/count/export under exact oracle
  comparison. The include-pending diagnostic for
  `ftstroke.FT_Glyph_Stroke.destroy_original_option` moved from status mismatch
  (`oracle OK`, Rust error `7`) to a real geometry mismatch:
  `/outline/points/0/y expected=896 actual=1152`. The row remains pending and
  must not be promoted until the exported points/tags/contours match pinned C.
  Focused active gates remain green:
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_Stroke` passes
  `7/7` runnable rows with `1` pending,
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Glyph_StrokeBorder`
  passes `4/4`, `make -C pillow-rs-freetype test-ffi-compat` passes, and
  `make -C pillow-rs font-tests` passes. The next divergence is border export
  geometry/order, not `_imagingft.rs` adapter behavior.
- Coverage MCP after committing that lower-stroker slice:
  `font-tests-coverage-with-freetype-pillow-12-2` run
  `0896cf95-afbd-470b-a4b2-cc82e48cf581` passed and ingested snapshot
  `7f9b63b7-91d8-4b3d-9cda-10d0492d3030` for commit `5f0001e13`.
  `pillow-rs/src/font/imagingft.rs` remains `1660/1682` lines,
  `249/254` branches, `162/173` functions, and `2612/2696` regions
  (`96.88%`). The remaining reported ranges are unchanged: `91`, `253`,
  `271`, `796`, `826`, `829`, `831`, and `928`. This confirms the committed
  lower fix improves the pending stroker failure from unsupported status to
  geometry mismatch, but it does not claim active ImageFont region closure until
  the exact glyph outline row is promoted.
- Focused geometry diagnostic after `8ccea47b`: the C oracle for
  `ftstroke.FT_Glyph_Stroke.destroy_original_option` exports `128` points,
  `4` contours, and starts the first contour at `(605, 896)` with alternating
  conic/on tags. Rust exports the same route successfully but starts the first
  compared contour at `(605, 1152)` and proceeds across the top/right side.
  This makes the next actionable divergence border close/export ordering or
  contour start normalization, not the wide-stroke negative-sector math and not
  `_imagingft.rs`.
- Border mapping fix after that diagnostic: FreeType 2.14.3 public border
  index `FT_STROKER_BORDER_LEFT` is `borders[0]`, and
  `src/base/ftstroke.c:1232-1263` initializes that slot from
  `center + normal`. The generic Rust border builder stored that same slot in a
  field named `right_border`, so `FT_Stroker_Export` was exporting the two
  generated border contours in the opposite public order for real parsed
  outlines. Mapping the live generic border slots back to FreeType's public
  constants makes the focused row exact: `make -C pillow-rs-freetype
  test-pending-case CASE=ftstroke.FT_Glyph_Stroke.destroy_original_option`
  passes `1/1`, and the normal `make -C pillow-rs-freetype test-case
  CASE=ftstroke.FT_Glyph_Stroke` lane now passes `8/8` runnable rows with
  `0` pending. `FT_Glyph_StrokeBorder` remains `4/4` passing. This affects
  Pillow only through `ImageFont` stroked text paths (`stroke_width != 0`);
  non-stroked Font metrics/masks are unchanged.
- Coverage MCP after committing the border mapping fix:
  `font-tests-coverage-with-freetype-pillow-12-2` run
  `c2f4cec6-f16e-49c7-bf84-51422d5c14a6` passed and ingested snapshot
  `0ffd8b0a-c1cf-4018-9dd3-f81e028f3764` for commit `f096a6d92`.
  `pillow-rs/src/font/imagingft.rs` remains `1660/1682` lines,
  `249/254` branches, `162/173` functions, and `2612/2696` regions
  (`96.88%`). The remaining reported ranges are unchanged: `91`, `253`,
  `271`, `796`, `826`, `829`, `831`, and `928`. This run proves the live
  Pillow Font oracle suite still passes and the promoted lower stroker row does
  not regress active ImageFont coverage, but it does not close the remaining
  `imagingft.rs` region gaps.
- Follow-up public constructor input review: direct `ImageFont.truetype`
  construction covered valid `index=0`, while the independent out-of-range
  constructor path was only represented indirectly through `font_variant`.
  Added input-only row
  `font.constructor.truetype_dejavusans_index_out_of_range_error` so the live
  Pillow 12.2.0 oracle directly proves `truetype(index=1)` error behavior for a
  single-face font. `make -C pillow-rs font-tests` passes with this row; no
  oracle output or error expectation is embedded in the JSON.
- Coverage MCP after committing that constructor row:
  `font-tests-coverage-with-freetype-pillow-12-2` run
  `3782007a-cb2b-4193-bd41-ee4b24946360` passed and ingested snapshot
  `046f5af0-6b6f-44cc-9724-bfb98b50fd01` for commit `03c56d02`.
  `pillow-rs/src/font/imagingft.rs` remains `1660/1682` lines,
  `249/254` branches, `162/173` functions, and `2612/2696` regions
  (`96.88%`), with unchanged reported ranges `91`, `253`, `271`, `796`,
  `826`, `829`, `831`, and `928`. Direct LLVM segment inspection of the current
  report shows the remaining branch markers sit on constructor/helper
  signatures or debug-overflow/source-map spans, while the static table misses
  are tuple-open lines for FreeType error constants. No additional public
  ImageFont behavior gap was identified from those markers.
- Lower `FT_Glyph_To_Bitmap` recheck after the stroker border-order fix:
  the previously tracked ImageFont blocker
  `ftglyph.FT_Glyph_To_Bitmap.pending_stroked_mono_target_outline_to_bitmap`
  now passes exactly when included through
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftglyph.FT_Glyph_To_Bitmap.pending_stroked_mono_target_outline_to_bitmap`.
  The normal lane `make -C pillow-rs-freetype test-case
  CASE=ftglyph.FT_Glyph_To_Bitmap` passes `10/10` runnable rows and leaves
  `1` pending row for a separate render-failure preservation route. The active
  stroked mono-target Pillow/ImageFont blocker is therefore cleared by the lower
  stroker fixes; the remaining pending `FT_Glyph_To_Bitmap` work is not an
  `imagingft.rs` adapter gap unless a public Font row proves it affects
  Pillow-visible behavior.
- Follow-up `FT_Glyph_To_Bitmap` route promotion: the remaining
  `ftglyph.FT_Glyph_To_Bitmap.error_render_failure_preserves_original` row also
  passes when included directly with `make -C pillow-rs-freetype
  test-pending-case CASE=ftglyph.FT_Glyph_To_Bitmap.error_render_failure_preserves_original`.
  Promoting that proven row moves the normal lane to `11/11` runnable rows with
  `0` pending and moves route audit to `real-parity=4847`,
  `pending-route=175`. This was a classification correction based on exact
  C/Rust/C-ABI/WASM parity evidence; no lower implementation change was needed
  for this row.
- Remaining ImageFont-adjacent lower stroker audit after `30fca833c`: probing
  representative pending rows confirms they are true route gaps, not stale
  classifications. `test-pending-case` leaves `runnable=0,pending=1` for
  `FT_Stroker_Set.miter_limit_clamped_to_one`,
  `FT_Stroker_Set.attributes_affect_geometry`,
  `FT_Stroker_BeginSubPath.open_subpath_initial_state`,
  `FT_Stroker_BeginSubPath.closed_subpath_initial_state`,
  `FT_Stroker_Rewind.attributes_preserved`, bevel/round/miter join geometry,
  `FT_STROKER_LINEJOIN_ROUND.wide_curve_join_restoration`,
  `FT_Stroker_Done.after_export_cleanup`, and
  `FT_Stroker_ParseOutline.line_conic_cubic_success`. The next real work is
  adding maintained runtime routes for these lower stroker lifecycle/config and
  mixed-geometry rows, then fixing any exact C/Rust divergence they expose. Do
  not promote them or add public Font rows until the same-input lower route
  exists and passes exact oracle comparison.
- Set/Rewind reset-count correction after that audit: the generated harness
  already had maintained C/Rust/C-ABI/WASM routes for
  `ftstroke.FT_Stroker_Set.clears_existing_path`,
  `ftstroke.FT_Stroker_Rewind.clears_previous_path`, and
  `ftstroke.FT_Stroker_Rewind.set_calls_rewind`. Running those rows through
  `test-pending-case` passed `1/1` each, and the generated route audit now
  classifies all three as `real-parity` with public count outputs before and
  after reset. The normal lanes verify this split:
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_Set` passes
  `2/2` runnable rows with `2` pending attribute/geometry rows, and
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_Rewind`
  passes `3/3` runnable rows with `1` pending attribute-preservation row.
- `FT_Stroker_Rewind.attributes_preserved` promotion: adding a maintained
  route exposed a real first divergence. Pinned C FreeType 2.14.3 returned
  `counts_after_second_path.points = 10` and exported a 10-point/2-contour
  fixed-miter outline after `FT_Stroker_Rewind`; Rust returned only `6` points
  because `StrokerState::process_outside_corner` returned early for
  non-round joins unless the join was bevel. The fix ports the non-round
  outside-corner miter-limit branch from `freetype/src/base/ftstroke.c:1032-1215`
  into `pillow-rs-freetype/src/ffi/handles.rs`, preserving the source boundary:
  this is FreeType-owned stroker geometry, not Pillow `_imagingft.c` adapter
  behavior. Verification:
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_Stroker_Rewind.attributes_preserved` passes `1/1`;
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_Rewind`
  now passes `4/4` runnable rows with `0` pending. Route audit moves to
  `real-parity=4848` and `pending-route=174`. Adjacent checks
  `FT_Stroker_Set`, `FT_STROKER_LINEJOIN_MITER_FIXED`, and
  `FT_STROKER_LINEJOIN_MITER_VARIABLE` still pass their runnable rows and keep
  their broader geometry rows pending until maintained same-input routes exist.
  Coverage MCP command `font-tests-coverage-with-freetype-pillow-12-2` passed
  after this commit as run `b9a859c8-6b20-4f46-81fd-a25cc271bbf7` and ingested
  snapshot `2cfc0d96-5639-436d-b0be-49140d32c10b` for commit `81ff11fa`.
  `pillow-rs/src/font/imagingft.rs` remains unchanged at `1660/1682` lines,
  `249/254` branches, `162/173` functions, and `2612/2696` regions
  (`96.88%`), with the same eight reported ranges `91`, `253`, `271`, `796`,
  `826`, `829`, `831`, and `928`. This proves no new active Pillow Font
  coverage gap was introduced or closed by the lower stroker fix.
- `FT_Stroker_Set.miter_limit_clamped_to_one` promotion: after the non-round
  outside-corner miter branch was ported, the miter-limit clamp row has a
  maintained route. The route runs the fixture miter limits `0`, `32768`,
  `65535`, `65536`, and `131072` through pinned C FreeType, Rust FFI, C ABI,
  and WASM ABI. Pinned C shows the first four rows clamp to effective
  `65536` and export identical 11-point/2-contour fixed-miter fallback
  outlines, while `131072` remains effective `131072` and exports the
  10-point/2-contour longer-miter outline. Rust now matches those exported
  points/tags/contours exactly. Verification:
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_Stroker_Set.miter_limit_clamped_to_one` passes `1/1`;
  `make -C pillow-rs-freetype test-case CASE=ftstroke.FT_Stroker_Set` now
  passes `3/3` runnable rows with `1` pending broad attribute-matrix row.
  Route audit moves to `real-parity=4849` and `pending-route=173`.
- Fixed/variable miter join geometry promotion: the narrow enum-variant rows
  `ftstroke.FT_STROKER_LINEJOIN_MITER_FIXED.fixed_miter_limit_geometry` and
  `ftstroke.FT_STROKER_LINEJOIN_MITER_VARIABLE.variable_miter_limit_geometry`
  now have a shared maintained route using their fixture path `[(0,0),
  (512,0), (576,512)]`, radius `64`, butt cap, and miter limits `65536` and
  `131072`. Pinned C FreeType emits the oracle `outline_by_limit` maps and the
  route compares exact points/tags/contours through Rust FFI, C ABI, and WASM
  ABI. The output booleans (`bevel_fallback` for fixed, `variable_clip` for
  variable) are derived from whether the two exported outlines differ. Verified
  commands:
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_MITER_FIXED.fixed_miter_limit_geometry`,
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_MITER_VARIABLE.variable_miter_limit_geometry`,
  `make -C pillow-rs-freetype test-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_MITER_FIXED`, and
  `make -C pillow-rs-freetype test-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_MITER_VARIABLE` all pass. Both normal
  lanes are now `2/2` runnable with `0` pending. Route audit moves to
  `real-parity=4851` and `pending-route=171`. At this point the remaining
  line-join pending rows were bevel, round, miter alias, wide round-curve
  restoration, and the broad `FT_Stroker_LineJoin.join_geometry_and_miter_limit`
  row.
- `FT_STROKER_LINEJOIN_MITER` alias promotion: the alias row
  `ftstroke.FT_STROKER_LINEJOIN_MITER.alias_matches_variable_join_geometry`
  now has a maintained route comparing the same fixture path/radius/cap at
  miter limit `65536` through the public `FT_STROKER_LINEJOIN_MITER` alias and
  `FT_STROKER_LINEJOIN_MITER_VARIABLE`. Pinned C, Rust FFI, C ABI, and WASM ABI
  all export identical outlines and report `alias_geometry_equal=true`.
  Verification:
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_MITER.alias_matches_variable_join_geometry`
  and `make -C pillow-rs-freetype test-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_MITER` pass. Route audit moves to
  `real-parity=4852` and `pending-route=170`. Remaining line-join pending rows
  are now bevel, round, wide round-curve restoration, and the broad
  `FT_Stroker_LineJoin.join_geometry_and_miter_limit` row.
- `FT_STROKER_LINEJOIN_BEVEL` promotion: the row
  `ftstroke.FT_STROKER_LINEJOIN_BEVEL.bevel_join_geometry` now has a maintained
  input-driven route. The harness reads `path`, `radius`, `line_cap`,
  `line_join`, and `miter_limit` from the fixture, passes those values to the
  pinned C oracle, and compares exact exported bevel outline points/tags/contours
  plus `join_shape="bevel"` through Rust FFI, C ABI, and WASM ABI. Verification:
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_BEVEL.bevel_join_geometry` and
  `make -C pillow-rs-freetype test-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_BEVEL` pass. The normal bevel lane is now
  `2/2` runnable with `0` pending. Route audit moves to `real-parity=4853` and
  `pending-route=169`. Remaining line-join pending rows are now round,
  wide round-curve restoration, and the broad
  `FT_Stroker_LineJoin.join_geometry_and_miter_limit` row.
- `FT_STROKER_LINEJOIN_ROUND.round_join_geometry` re-check: forced pending
  execution with `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_ROUND.round_join_geometry` still runs
  `0` parity rows and stays pending as
  `ftstroke.stroke_manual_path:pending-route cases require an explicit
  maintained runtime route; generic fallback is not parity evidence`. The
  active input references future-only assets
  `outlines/stroker/round-join-line-conic-cubic.json` and named paths
  `acute_line_join`, `conic_join`, and `cubic_join`; no matching asset currently
  exists under `pillow-rs-freetype/tests/fixtures`. Promoting this row correctly
  requires adding maintained path-record fixtures and a
  `ftstroke.stroke_manual_path` route that feeds the same records into pinned C,
  Rust FFI, C ABI, and WASM ABI, then compares exact status sequence,
  point/contour counts, exported outline, and cbox. It should not be promoted by
  route reclassification alone.
- `FT_STROKER_LINEJOIN_ROUND.round_join_geometry` promotion: the missing
  path-record asset now exists at
  `pillow-rs-freetype/tests/fixtures/input/outlines/stroker/round-join-line-conic-cubic.json`
  with maintained `acute_line_join`, `conic_join`, and `cubic_join` records.
  The `ftstroke.stroke_manual_path` route reads those named records from the
  fixture, encodes the same records for pinned C, and compares the combined
  stroker output across Rust FFI, C ABI, and WASM ABI. Exact output includes
  ordered `status_sequence`, `point_count`, `contour_count`,
  `exported_outline.points/tags/contours`, and `cbox`. Verification:
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_ROUND.round_join_geometry` passes `1/1`,
  and `make -C pillow-rs-freetype test-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_ROUND` passes with `2` runnable rows and
  `1` explicit pending row. Route audit moves to `real-parity=4854` and
  `pending-route=168`. Remaining line-join pending rows are now the wide
  round-curve restoration row and the broad
  `FT_Stroker_LineJoin.join_geometry_and_miter_limit` row.
- `FT_STROKER_LINEJOIN_ROUND.wide_curve_join_restoration` re-check: forced
  pending execution with `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_ROUND.wide_curve_join_restoration` still
  runs `0` parity rows and stays pending as
  `ftstroke.stroke_wide_curve:pending-route cases require an explicit
  maintained runtime route; generic fallback is not parity evidence`. The input
  still references future-only `outlines/stroker/wide-curve-joins.json` and
  named path `wide_curve_negative_sector`. Promotion requires a maintained
  wide-curve path fixture and a `ftstroke.stroke_wide_curve` route that proves
  FreeType's temporary round-join path restores the saved non-round join state
  after curve subdivision, comparing exact status sequence,
  `line_join_after_curve`, and exported outline across pinned C, Rust FFI,
  C ABI, and WASM ABI.
- `FT_Stroker_LineJoin.join_geometry_and_miter_limit` promotion: the broad enum
  matrix row now has a maintained route over the fixture path, all public
  `line_joins`, and both `miter_limits`. The pinned C oracle, Rust FFI, C ABI,
  and WASM ABI compare exact `outlines_by_join_and_limit` entries and
  `alias_geometry_equal`, proving the `FT_STROKER_LINEJOIN_MITER` alias matches
  `FT_STROKER_LINEJOIN_MITER_VARIABLE` while fixed/variable/bevel/round joins
  select the same exported geometry as C. Verification:
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_Stroker_LineJoin.join_geometry_and_miter_limit` passes
  `1/1`, and `make -C pillow-rs-freetype test-case
  CASE=ftstroke.FT_Stroker_LineJoin` passes `2/2` with `0` pending. Route audit
  moves to `real-parity=4855` and `pending-route=167`. The remaining line-join
  blocker is now only `FT_STROKER_LINEJOIN_ROUND.wide_curve_join_restoration`.
- `FT_STROKER_LINEJOIN_ROUND.wide_curve_join_restoration` promotion: the
  missing wide-curve path-record asset now exists at
  `pillow-rs-freetype/tests/fixtures/input/outlines/stroker/wide-curve-joins.json`
  with the maintained `wide_curve_negative_sector` record. The
  `ftstroke.stroke_wide_curve` route feeds that exact move/cubic/line sequence
  through pinned C FreeType, Rust FFI, C ABI, and WASM ABI. Exact output
  includes the ordered `status_sequence` and exported outline
  `points/tags/contours`. The route does not read FreeType private stroker
  state; restoration of the saved non-round join after the temporary round
  curve subdivision is proven by the post-curve line segment's exported geometry
  matching pinned C exactly. Verification:
  `make -C pillow-rs-freetype test-pending-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_ROUND.wide_curve_join_restoration` passes
  `1/1`, and `make -C pillow-rs-freetype test-case
  CASE=ftstroke.FT_STROKER_LINEJOIN_ROUND` passes `3/3` with `0` pending.
  `make -C pillow-rs-freetype test-ffi-compat`, `make -C pillow-rs font-tests`,
  and `make fontdone-lint` also pass. Route audit moves to
  `real-parity=4856` and `pending-route=166`. This clears the currently named
  line-join pending rows; remaining ImageFont-adjacent lower blockers should be
  selected from the next pending-route audit rather than by adding duplicate
  public Font rows. Coverage MCP command
  `font-tests-coverage-with-freetype-pillow-12-2` passed after this commit as
  run `c766c267-e09f-4196-b204-736a8b44d8bd` and ingested snapshot
  `27ad41d7-ae88-40cb-b286-224f227a4e5e` for commit `2e790b47f`.
  `pillow-rs/src/font/imagingft.rs` remains unchanged at `1660/1682` lines,
  `249/254` branches, `162/173` functions, and `2612/2696` regions
  (`96.88%`), with the same eight reported ranges `91`, `253`, `271`, `796`,
  `826`, `829`, `831`, and `928`. This confirms the lower route promotion did
  not close a direct Pillow `_imagingft.c` adapter region; it removes a lower
  stroker route blocker from the audit ledger.
