# PIL.ImageFont 100% Region Coverage Plan

Last updated: 2026-07-27 (Asia/Kolkata)

## Current target clarification: PIL.ImageFont, not FreeType-only

This plan is now scoped to the full public `PIL.ImageFont` module behavior.
`ImageFont.FreeTypeFont`, `_imagingft.c`, and `pillow-rs-freetype` are
implementation routes inside that target, not the target itself.

The only explicit out-of-scope area is successful libraqm shaping. Inputs that
use `direction`, `features`, or `language` without libraqm remain in scope and
must match Pillow's runtime error behavior exactly.

Coverage work must therefore be tied back to one of these:

- an active input-only fixture row under
  `pillow-rs/tests/fixtures/font/inputs/public-api`;
- a manifest-enforced `PIL.ImageFont` public operation/parameter in
  `pillow-rs/tests/fixtures/font/font_manifest.yaml`;
- a documented lower-level blocker that prevents a specific public
  `PIL.ImageFont` row from passing.

Do not use `_imagingft` or `pillow-rs-freetype` coverage alone to claim ImageFont
completion. Those measurements are useful only when they explain or unlock the
public `PIL.ImageFont` behavior.

## Current measured checkpoint: 2026-07-27

- Command: `font-tests-coverage-with-freetype`
- Run: `150b677f-c229-4846-8c18-4705aa8d4bcd`
- Snapshot: `2a895073-ef7f-474a-ae79-f4fdc34c81b4`
- Commit: `665da57df87b79b316ea86cee8ccfb59c6a39392`
- Result: passed, ingested
- `pillow-rs/src/font/default_aileron.rs`: regions `24/24` (`100.00%`)
- `pillow-rs/src/font/mod.rs`: regions `487/487` (`100.00%`)
- `pillow-rs/src/font/pilfont.rs`: regions `1014/1094` (`92.69%`), with
  complete branch coverage and one reported doc-comment line gap in the current
  LLVM source map.
- `pillow-rs/src/font/imagingft.rs`: regions `2591/2687` (`96.43%`).

The remaining region gap is not manifest drift: the active manifest/test
contract already targets `PIL.ImageFont` and verifies the public operation and
parameter map. The remaining measured regions are adapter/lower-level FreeType
paths that need either a real public `PIL.ImageFont` oracle row or a lower-level
implementation fix before they can be honestly covered.

Current manifest status: every live `PIL.ImageFont` public method parameter is
classified and covered. The last public-parameter blockers were bitmap
`ImageFont`/`TransposedFont` `*args` and `**kwargs`; active input-only rows now
pass those extras into the live Pillow oracle and verify Rust's ignored-extra
behavior exactly.

## Execution status update: 2026-07-26

Latest measured checkpoint after public-signature edge sweep and private
coverage-noise simplification:

- Commit: `45a654881`
- Coverage MCP run: `f7c271cc-063f-452a-8f1f-b7f851e1c6f0`
- Snapshot: `7da02b92-d1bf-436d-b0b2-a137e4f11539`
- Command: `imagingft-tests-coverage-fixed`
- Result: passed, ingested
- `pillow-rs/src/font/imagingft.rs`:
  - lines: `955/1040` (`91.83%`)
  - branches: `185/238` (`77.73%`)
  - functions: `95/110` (`86.36%`)
  - regions: `1594/1746` (`91.29%`)
- `pillow-rs/src/font/mod.rs`:
  - lines: `190/214` (`88.79%`)
  - functions: `44/51` (`86.27%`)
  - regions: `230/272` (`84.56%`)

Latest manifest movement:

- Covered `getmask.ink` with both integer-accepted and JSON-list error rows.
- Covered `getmask2.ink`, `getmask2.args`, and `getmask2.kwargs` with
  live-oracle rows that pass real positional args and extra kwargs into
  Pillow's public `FreeTypeFont.getmask2`.
- Fixed and covered `font_variant(layout_engine=...)` no-raqm behavior:
  Pillow accepts RAQM and unknown strings and falls back to BASIC rather than
  raising an error.
- Added input-only `start` clipping/error rows for `getmask/getmask2`. These
  preserve independent Pillow edge cases, though LLVM region totals did not
  move because the relevant branches were already represented or normalized.
- Removed two private non-behavioral coverage-noise branches: a one-use
  advance-limit constant and an unreachable bitmap-pitch conversion failure
  path on the supported target.
- Added exact Pillow parity rows for additional SBIT mask formats
  (`sbit-gray2-format1.ttf`, `sbit-gray4-format1.ttf`,
  `sbit-bgra-format1.ttf`). Coverage MCP run
  `6ef4cedf-4fff-4e0f-8461-f7e4fd998cb8` ingested snapshot
  `62ff34a2-15f0-4821-adb3-2fa73c1c9593` at commit `aa64f706b`; the
  `imagingft.rs` metrics did not move (`1594/1746` regions), so equivalent
  SBIT-format fixture expansion is not expected to close the remaining
  `imagingft.rs` gaps.
- Remaining blocked public parameters are now only:
  - `getmask`: `stroke_width`
  - `getmask2`: `stroke_width`

Current blocker to literal 100%:

- Stroked `getmask/getmask2` is still the public implementation blocker.
  Input-only rows cannot be honestly added for `stroke_width != 0` until Rust
  can render Pillow-equivalent stroked glyph masks instead of returning
  `NotImplementedError`.
- Remaining uncovered regions outside the public stroke blocker are defensive
  FreeType/bitmap/fallback paths that still need real oracle-driving assets or
  code simplification after proving they are unreachable.

Latest measured checkpoint:

- Commit: `1b5701b1c`
- Coverage MCP run: `5f04cbf2-17aa-4486-b861-1f1d12d4d6aa`
- Snapshot: `9bf3e928-980b-4c44-9e92-8fb637b25ad3`
- Command: `imagingft-tests-coverage-fixed`
- Result: passed, ingested
- `pillow-rs/src/font/imagingft.rs`:
  - lines: `1021/1112` (`91.82%`)
  - branches: `194/250` (`77.60%`)
  - functions: `103/119` (`86.55%`)
  - regions: `1687/1849` (`91.24%`)
- `pillow-rs/src/font/mod.rs`:
  - lines: `172/196` (`87.76%`)
  - functions: `39/46` (`84.78%`)
  - regions: `209/251` (`83.27%`)

Latest manifest hardening:

- `font_manifest.yaml` now includes `public_method_parameters`.
- `font_public_api.rs` queries live Pillow `FreeTypeFont` signatures through
  the repo-local oracle and fails unless every public parameter is classified
  as either covered or blocked.
- Covered parameters must be exercised by at least one active input-only row.
- Currently blocked public parameters:
  - `font_variant`: `font`, `index`, `encoding`, `layout_engine`
  - `getmask`: `stroke_width`, `ink`
  - `getmask2`: `stroke_width`, `ink`, `args`, `kwargs`

Still required for completion:

- Implement stroked `getmask/getmask2` rendering by integrating real FreeType
  stroker support into the Font mask path. Current `pillow-rs-freetype`
  `FT_Stroker_ParseOutline` does not yet support real glyph contours, so this
  depends on lower-level stroker geometry completion before honest Pillow
  pixel parity rows can be added.
- Implement or explicitly redesign unsupported `font_variant` parameters.
- Continue reducing/removing unreachable defensive branches only when proven by
  oracle-backed evidence.

- Commit: `a32242993`
- Coverage MCP run: `e3d92d05-0f5b-4a5b-b931-3ac0143469c3`
- Snapshot: `33c1bfe0-3025-4996-9348-90a7682386b0`
- Command: `imagingft-tests-coverage-fixed`
- Result: passed, ingested
- `pillow-rs/src/font/imagingft.rs`:
  - lines: `1020/1112` (`91.73%`)
  - branches: `194/250` (`77.60%`)
  - functions: `103/119` (`86.55%`)
  - regions: `1684/1849` (`91.08%`)
- `pillow-rs/src/font/mod.rs`:
  - lines: `172/196` (`87.76%`)
  - functions: `39/46` (`84.78%`)
  - regions: `209/251` (`83.27%`)

Latest added cases/fixes:

- Routed the ImageFont parity runner through explicit root `pillow_rs::imagefont_*`
  functions where available, matching the single-root-public-API boundary.
- Added anchor rows for `lt`, `la`, `ls`, and `lb`.
- Added `getmask2` option-path `start` row.
- Added empty `features` row for `getlength`; this found a real mismatch and
  Rust now matches Pillow by treating any supplied `features` list, including
  `[]`, as libraqm-gated.

Current blocker to literal 100%:

- Stroked `getmask/getmask2` is missing. Pillow supports
  `stroke_width != 0`; Rust currently returns `NotImplementedError`. The next
  implementation task is to integrate the pure-Rust FreeType stroker path into
  `pillow-rs/src/font/imagingft.rs`, then add input-only `getmask/getmask2`
  stroked rows and rerun Coverage MCP.
- Remaining non-stroke uncovered regions are defensive FreeType/bitmap/error
  branches that must be driven by real Pillow oracle assets or removed as dead
  code if proven unreachable.

Latest checkpoint after Pillow Font comparison review:

- `15e039a29` is the current measured commit.
- Coverage MCP run `200762a0-9e2e-4c9d-93ec-8cb7a8d4519e` passed and ingested
  snapshot `2010d398-5db4-479a-b747-91439a5d2160`.
- Current target coverage:
  - `pillow-rs/src/font/imagingft.rs`: regions `1660/1829` (`90.76%`)
  - `pillow-rs/src/font/mod.rs`: regions `194/232` (`83.62%`)
- Newly implemented/covered Pillow option paths:
  - `getbbox` anchor, bad-anchor, integer/fractional stroke width, ignored
    `mode`, and libraqm-required error rows.
  - `getlength` libraqm-required error rows for `features` and `language`.
  - `getmask2` anchor offset, `mode="RGBA"` TypeError, and libraqm-required
    error row.
- Current high-confidence blockers:
  - Stroked mask pixel parity needs a real outline-stroking implementation.
    Do not add a `stroke_width` `getmask/getmask2` row until Rust renders the
    same pixels as Pillow.
  - `getmask` still needs the same parameterized option path as `getmask2`.
  - `font_variant` still lacks alternate font source, index, encoding, and
    layout-engine override parity.
  - Remaining FreeType error/fallback/bitmap-storage branches need real
    oracle-driving font assets.

Current committed checkpoint:

- `e7db8f817` — added Pillow `FreeTypeFont` variation public APIs to the
  root Rust API plus thin Python/JS delegations and manifest-driven parity rows
  for `font_variant`, `get_variation_axes`, `get_variation_names`,
  `set_variation_by_axes`, and `set_variation_by_name`.
- `060d763c6` — expanded variation mutation rows and fixed public style-name
  refresh after `set_variation_by_name`; Pillow reports the selected named
  instance as the public style name.

Latest Coverage MCP snapshot:

- Run: `8f07704f-98e8-4677-ba61-d523d946203a`
- Snapshot: `48f1c0ae-b25a-4c55-bc08-017de9b90a1e`
- Commit: `060d763c65d86528be7a245f70ef3d124e2a50f2`
- `pillow-rs/src/font/imagingft.rs`:
  - lines: `925/1012` (`91.40316206%`)
  - branches: `182/236` (`77.11864407%`)
  - functions: `97/113` (`85.84070796%`)
  - regions: `1556/1717` (`90.62317997%`)
- `pillow-rs/src/font/mod.rs`:
  - lines: `131/146` (`89.72602740%`)
  - functions: `32/36` (`88.88888889%`)
  - regions: `170/202` (`84.15841584%`)
- Current corpus: `154` input-only rows across `20` files.
- The manifest now includes every inspected Pillow `FreeTypeFont` public method
  from the repo-local oracle (`font_variant`, variation getters/setters,
  metrics/name/layout/mask methods) plus classified helper/consumer operations
  needed for constructor, transposed, binary, draw, and Result/error parity.
- Literal 100% region coverage is still not achieved.

Implemented and committed during execution:

- `77c64bb22` — added variable-font and SBIT oracle rows.
- `f0d02db78` — added invalid-`maxp` runtime error rows; changed
  imagingft mask/render/draw test-facing paths to preserve `Result` errors
  instead of silently returning empty masks; mapped
  `FT_Err_Too_Many_Instruction_Defs` to Pillow
  `OSError("too many instruction definitions")`.
- `a460c1e27` — added generated hinter fixtures for `code overflow` and
  `nested DEFS`; fixed `fontdone` FDEF-index overflow classification to
  return `FT_Err_Code_Overflow`, and preserved `FT_Err_Nested_DEFS`.
- `550787522` — preserved missing SFNT face names as `None` through the
  imagingft oracle runner while keeping existing binding `getname()` defaults
  stable.

Implemented in the current worktree:

- Refactored `pillow-rs/src/font::Font` from a `TrueType | Bitmap` enum into a
  FreeType-only Pillow `FreeTypeFont` handle.
- Removed all `Font::Bitmap` branches and `shift_bitmap_mask` from
  `pillow-rs/src/font/imagingft.rs`.
- Deleted the orphan `pillow-rs/src/bitmap_font.rs` atlas and its data file.
- Removed Python/JS `font_default_*` exports that exposed the orphan bitmap
  atlas instead of a real Pillow public surface.
- Kept legacy PIL bitmap font behavior explicit in `pillow-rs/src/font/pilfont.rs`
  for `load`, `load_path`, and `load_default_imagefont`.

Verified gates:

- `make -C pillow-rs font-tests` passes after each committed batch.
- Coverage MCP command `imagingft-tests-coverage-fixed compatibility registration` passed and ingested
  after each committed batch.

Coverage snapshots:

| Commit | Snapshot | `font/imagingft.rs` regions | Branches | Notes |
|---|---|---:|---:|---|
| `88504ef6e` | `e77930c8-41c2-4eb2-b943-4ce8ca2066cf` | `1873/2338` (`80.11%`) | `172/258` | pre-error-row baseline used for this execution |
| `f0d02db78` | `b32d6e80-3e98-4e85-ade4-28b416790e97` | `1851/2294` (`80.69%`) | `174/260` | Result propagation changed denominator |
| `a460c1e27` | `f1c881aa-ebc5-40ea-a8ba-371a8418f355` | `1800/2243` (`80.25%`) | `178/262` | bytecode error rows/fixes |
| `550787522` | `d8402aae-43ae-469d-8efd-c01666ce5af8` | `1875/2330` (`80.47%`) | `181/266` | missing-name row/fix |
| `9d0aad7e` + dirty worktree | `37d797ad-be62-4dc1-b1e3-46fac6547b03` | `1649/2009` (`82.08%`) | `160/224` | removed Rust-only bitmap path from imagingft ownership |
| `978b6b403` + dirty worktree | `e3c79419-67ff-4b76-ac15-17cf0822a908` | `687/762` (`90.16%`) | `86/108` | active suite moved to Font public API; imagingft-named harness/fixtures deprecated |

Current blocker to literal 100% region coverage:

- The previous `Font::Bitmap`/`shift_bitmap_mask` blocker has been removed.
- Remaining gaps are lower-level FreeType-backed implementation routes inside
  the public `PIL.ImageFont` target: load/request-size error mapping,
  glyph-load/render fallback, uncommon bitmap coverage modes, clipping guard
  branches, and source-map wrapper lines. They are not a separate FreeType-only
  target for completion. Each one still requires a real public Pillow
  `PIL.ImageFont` oracle input, a documented public ImageFont blocker, or
  implementation simplification; they must not be covered with mocks or Rust
  self-comparison.
- A direct `PIL.ImageFont.FreeTypeFont.getmask/getmask2` probe with
  DejaVuSans `"A"`, `stroke_width=1.5`, and `mode="1"` found another valid
  public blocked row. Pillow returns an antialiased `L` mask from a
  mono-targeted stroked outline. Rust currently produces binary coverage on
  the mono stroked path; forcing normal stroked rendering only reproduces the
  existing `mode="L"` row. Keep this row out of the active corpus until the
  lower-level mono-targeted glyph load plus stroker behavior matches Pillow.

Rejected during execution:

- `name_table_bad_storage.ttf` was not kept in the font public corpus.
  Live Pillow `_imagingft` loaded it, but the lower-level freetype manifest
  classifies the same asset as an `FT_Err_Name_Table_Missing` lane. Changing
  `fontdone` to accept it inside this task would risk weakening FreeType
  parity, so only `name_table_missing.ttf` was retained for the Pillow
  `(None, None)` `getname()` behavior.

## Purpose

This document is the working map to drive `pillow-rs/src/font/imagingft.rs`
from the current measured state to 100% region coverage without weakening
truth-level Pillow parity.

The rule for every added case is strict:

- input JSON remains input-only;
- output and error expectations come from the repo-local live
  `PIL.ImageFont` oracle at runtime; `_imagingft` is asserted only for
  `FreeTypeFont`-backed rows where Pillow itself routes through that native
  extension;
- no Rust test may compare Rust to itself;
- no expected pixel hex, hashes, status, or error text may be stored in the
  input corpus;
- synthetic cases are allowed only when the synthetic object/path is also
  executed against an equivalent C/Pillow oracle and the comparison remains
  exact.

## Current evidence baseline

- Fixture suite: `make -C pillow-rs font-tests`
- Coverage MCP command: `imagingft-tests-coverage-fixed` compatibility registration, currently executing `font-tests`
- Latest measured source snapshot: `e3c79419-67ff-4b76-ac15-17cf0822a908`
- Measured commit for source/fixtures: `978b6b4039aa72ebbf68e00ce016fd0533fec21c`
- Current corpus: `105` rows across `17` input files
- Current `pillow-rs/src/font/imagingft.rs` metrics:
  - lines: `394/432` (`91.20370370%`)
  - branches: `86/108` (`79.62962963%`)
  - functions: `38/43` (`88.37209302%`)
  - regions: `687/762` (`90.15748031%`)

The current corpus already proves exact Pillow parity for these public
operations:

| Operation | Rows |
|---|---:|
| `draw_text` | 6 |
| `get_transposed_mask` | 10 |
| `getbbox` | 9 |
| `getbbox_binary` | 7 |
| `getlength` | 6 |
| `getmask` | 8 |
| `getmask2` | 7 |
| `getmask2_with_start` | 19 |
| `getmetrics` | 3 |
| `getname` | 8 |
| `has_variations` | 3 |
| `render_text_binary` | 6 |
| `transposed_bbox` | 7 |
| `unsupported_magic` | 1 |
| `validate_transposed_length` | 5 |

## Proven exact paths today

These are paths already exercised by oracle-backed rows and matching exactly:

| Path family | Evidence rows / assets | Coverage value |
|---|---|---|
| Standard TrueType scalar APIs | `DejaVuSans.ttf`, `dejavu-coverage.ttf` | name, metrics, length, bbox, binary bbox |
| Standard TrueType grayscale masks | `Hello`, `AV`, `jQ`, punctuation rows | pixel-byte exact `L` masks |
| Standard TrueType binary masks | `getbbox_binary`, `render_text_binary` rows | exact `fontmode="1"` bbox and RGBA alpha |
| Empty and space text | `empty_suite`, `space`, `space_zero_height` | zero-size and zero-height public behavior |
| Positive/fractional starts | `baseline`, `integer_start`, `fractional_start`, `positive_fractional_start` | Pillow `getmask2(..., start=...)` origin behavior |
| Negative start clipping | small, moderate, and heavy negative x/y start rows | real Pillow clipping behavior in `_imagingft.c::font_render_impl` |
| Bad image size errors | huge negative x/y starts | exact `ValueError: bad image size` through `Result` |
| Transposed masks/bboxes/length errors | all Pillow transpose constants plus invalid string | exact success/error behavior |
| CFF scalar/bbox | `pure-cff-cubic.otf` | exact name/metrics/length/bbox/binary-bbox/variation false |
| Embedded-strike TTF | `embedded-strike-color-or-sbit.ttf` | exact scalar/mask/draw behavior for the chosen rows |
| Load failures | missing font, invalid sizes, huge sizes, tiny ppem | exact load-time error classification |

Rejected proof attempt:

- `pure-cff-cubic.otf` rendering rows were intentionally not kept.
  `getmask.pure_cff_a` produced same mode and size but different antialias
  bytes. That is a real parity gap, not a usable coverage row.

## Coverage gap taxonomy

Coverage MCP now reports 68 remaining gap groups in `font/imagingft.rs`.
The line numbers below are from snapshot `37d797ad-be62-4dc1-b1e3-46fac6547b03`.

| Gap bucket | Lines | Why uncovered | Route to cover |
|---|---:|---|---|
| Load success/error sub-branches | 35-36, 49, 51 | specific FreeType request-size and face-load error codes not yet produced by current inputs | add malformed/pathological font or size rows only when live Pillow emits the same public error |
| `ft_error_to_pil` fallback mapping | 74-83 | glyph-load/render errors after successful face load are not currently reachable | need a font/text pair that loads the face but fails during `FT_Load_Glyph`, or a C-backed synthetic glyph-load fixture |
| Transpose source-map artifacts | 129, 145 | already has all public orientations; line-level artifacts remain | verify with exact line records; likely no new input needed |
| Removed Rust-only bitmap path | former `Font::Bitmap` arms and `shift_bitmap_mask` | not a Pillow `_imagingft` surface | removed from this target; legacy bitmap parity belongs under `pilfont`, not `_imagingft` |
| `get_transposed_mask` no-orientation return arm | 145 | possible source mapping because `orientation: null` and missing orientation rows exist | verify via line-level detailed coverage before adding rows |
| Layout kerning/glyph failure branches | 359, 373-374, 441, 444, 456 | no current font triggers load failure or advance overflow in layout | probe freetype malformed and unsupported glyph fixtures |
| `mask_from_run_with_start` render fallback | 497, 502-503, 509, 512, 514-515, 544, 547, 555-578 | no current font makes render-load fail then non-render load succeed | need real font with unrenderable glyph or synthetic C-backed glyph slot |
| Clipping/sparse bitmap guard branches | 582-639 | only some negative-start clipping branches are hit | add more public start/text rows first; use coverage after every batch |
| `bitmap_coverage` uncommon storage | 644-692 | no current public row produces negative pitch, unsupported bitmap mode, or out-of-buffer coverage | need SBIT/color/unsupported bitmap fixture accepted by Pillow and Rust, or a lower-level C-backed bitmap oracle |

## Synthetic path policy

The word “synthetic” can mean three different things here. Only the first two
are acceptable for parity; the third is forbidden.

| Synthetic class | Allowed? | Description |
|---|---|---|
| Synthetic input font/text through Pillow public API | Yes | Generated or hand-crafted font asset loaded by `ImageFont.truetype`; output still comes from live `_imagingft` at runtime. |
| Synthetic C-backed fixture route | Yes, with new harness | A lower-level C oracle constructs the same FreeType/Pillow structure and Rust constructs the matching Rust structure; exact output/error is compared. |
| Rust-only helper/unit path | No for parity | A test manually constructs Rust values, forces branches, and asserts local behavior without C/Pillow oracle. This may be useful for safety tests but must not count toward Font parity coverage. |

## Exhaustive missing case plan

### 1. Variable font / `has_variations == true`

Current state:

- `has_variations` has only false rows.
- Freetype probe shows Pillow can load:
  - `pillow-rs-freetype/tests/fixtures/input/fonts/variable/named-instances.ttf`
  - `pillow-rs-freetype/tests/fixtures/input/fonts/generated/variable/ubuntu-sans-variable.ttf`
- Pillow reports `get_variation_axes()` succeeds for
  `variable/named-instances.ttf`.

Needed inputs:

- Copy `variable/named-instances.ttf` into
  `pillow-rs/tests/fixtures/font/input/fonts/`.
- Add rows:
  - `has_variations.variable_named_instances_true`
  - `getname.variable_named_instances`
  - `getmetrics.variable_named_instances`
  - `getbbox.variable_named_instances_A`
  - `getlength.variable_named_instances_AV`

Expected value:

- Should cover true branch of `has_variations`.
- May cover metadata/name paths not covered by current DejaVu/CFF assets.

Risk:

- Rust `fontdone` may not set `FT_FACE_FLAG_MULTIPLE_MASTERS` exactly. If it
  mismatches, keep the row only as a failing parity gap or fix core behavior.

### 2. CFF/CID rendering parity

Current state:

- CFF scalar/bbox rows with `pure-cff-cubic.otf` pass.
- CFF mask rows were rejected because grayscale bytes differed.
- Pillow probe shows these CFF/CID assets load:
  - `input/fonts/cff/fontinfo-populated.otf`
  - `input/fonts/cid/ot-cff-cid-keyed.otf`

Needed inputs:

- Add scalar/bbox rows first for `fontinfo-populated.otf` and
  `ot-cff-cid-keyed.otf`.
- Add rendering probe rows only behind the same strict oracle runner. If any
  mask byte differs, do not commit as passing coverage; classify as CFF render
  parity gap.

Expected value:

- Scalar rows may cover additional name/metrics/bbox branches.
- Rendering rows would be valuable only after CFF antialias parity is fixed.

Risk:

- Current pure CFF rendering already differs by antialias bytes; likely
  implementation work is required before these can increase trusted coverage.

### 3. SBIT / embedded bitmap strikes through TrueType

Current state:

- One embedded-strike TTF row family passes.
- Pillow probe shows the following freetype SBIT assets load through
  `ImageFont.truetype`:
  - `input/fonts/bitmap/embedded-strike.ttf`
  - `input/fonts/cache/bitmap-strike-small-sbits.ttf`
  - `fixtures/assets/fonts/sbit_mono_format1.ttf`
  - `fixtures/assets/fonts/sbit_gray_format1.ttf`
  - `fixtures/assets/fonts/sbit_bgra_format1.ttf`
  - `fixtures/assets/fonts/sbit_unsupported_image_format.ttf`

Needed inputs:

- Add a small SBIT matrix:
  - `getbbox`, `getbbox_binary`, `getmask`, `getmask2`,
    `render_text_binary`, `draw_text`
  - font sizes that select the embedded strike, not outline fallback
  - glyphs `A`, `!`, and one missing/space glyph
- Prefer one file each for mono, gray, BGRA/color, unsupported image format,
  missing bitmap, sparse miss, and no matching strike.

Expected value:

- May cover `bitmap_coverage` mono/gray paths more completely.
- May expose unsupported bitmap-mode behavior through the real Pillow oracle.

Risk:

- Pillow may silently use outline fallback rather than the SBIT strike for the
  requested size. Verify with output dimensions and coverage deltas.
- If Rust and Pillow differ on SBIT bitmap selection, keep rows as visible
  parity gaps or fix `fontdone`.

### 4. Missing glyph / empty glyph / zero-sized bitmap

Current state:

- Empty string and space are covered.
- The render loop still has uncovered absent/zero-sized bitmap branches.
- Freetype has:
  - `input/fonts/outlines/empty-glyph.ttf`
  - `input/fonts/charmap/charcode-zero-mapped.ttf`
  - `fixtures/assets/fonts/sbit_composite_mono_zero_height_component_format8.ttf`
  - `fixtures/assets/fonts/sbit_composite_mono_zero_width_component_format8.ttf`

Needed inputs:

- Probe exact text/glyph mapping from freetype cases and add rows that
  exercise:
  - glyph id exists but outline has no contours;
  - glyph id maps to `.notdef` or a no-bitmap strike;
  - component with zero width;
  - component with zero height.

Expected value:

- Targets lines around `bitmap == None`, `sx == 0`, `sy == 0`, and row/column
  skips in `mask_from_run_with_start`.

Risk:

- Many “empty glyph” fonts still produce the DejaVu `A` outline for ASCII
  because the fixture mutates a non-ASCII or specific glyph id. Use the exact
  freetype fixture glyph ids/chars, not guessed `A`.

### 5. Glyph-load failure after successful face load

Current state:

- Load-time failures are covered.
- Runtime glyph-load failure branches are not covered.
- Freetype has malformed or unsupported assets:
  - `generated/fonts/unsupported-feature-glyph.otf`
  - `fixtures/assets/fonts/sbit_missing_bitmap.ttf`
  - `fixtures/assets/fonts/sbit_mono_index_format5_truncated_image.ttf`
  - `fixtures/assets/fonts/sbit_composite_missing_subglyph.ttf`
  - `fixtures/assets/fonts/sbit_unsupported_index_format.ttf`

Needed inputs:

- For each candidate, run a live Pillow oracle probe:
  - load face succeeds;
  - `getname` succeeds;
  - `getbbox` or `getmask` for a specific text fails during glyph load/render.
- If found, add `getbbox`, `getmask`, and `getmask2_with_start` rows.

Expected value:

- Targets `FT_Load_Glyph` error mapping, layout fallback, render fallback,
  and error propagation through `Result`.

Risk:

- Pillow may fail during face load instead. Those are useful load-error rows
  but do not cover runtime glyph-load branches.

### 6. Request-size / face-load error variants

Current state:

- Covered public errors include:
  - missing resource;
  - size `0`, negative integer, negative float;
  - huge invalid argument;
  - huge invalid pixel size;
  - tiny invalid ppem.
- Coverage still shows unmapped branches for `FT_New_Memory_Face`,
  `FT_Request_Size`, and `ft_error_to_pil`.

Freetype assets to reuse:

- `tests/fixtures/files/zero-byte-font.bin`
- `tests/fixtures/generated/fonts/not-a-font.bin`
- `tests/fixtures/input/fonts/malformed/not-a-font.bin`
- `tests/fixtures/input/fonts/sfnt/invalid-maxp.ttf`
- `tests/fixtures/input/fonts/sfnt/truncated-maxp.ttf`
- `tests/fixtures/input/fonts/sfnt/cff-maxp-0500.otf`
- `tests/fixtures/malformed/ttc/count-overflows-offset-array.ttc`

Needed inputs:

- Add load-failure rows only where live Pillow and Rust produce the same
  structured error.
- If Rust maps the error differently, fix the mapping or keep as visible
  failing gap.

Expected value:

- Targets remaining load error mapping branches.

Risk:

- Some malformed files fail earlier in Pillow with `unknown file format`,
  while Rust may report `FT_New_Memory_Face: error ...`. The parity contract
  requires exact public error kind/message category.

### 7. Removed Rust-only bitmap path

Current state:

- These branches were removed from `font/imagingft.rs`.
- `Font::load_default(size)` remains FreeType-only, using Pillow’s embedded
  Aileron TTF subset.
- Legacy PIL bitmap font behavior remains separate in `font::pilfont`.

Needed decision:

- Do not reintroduce bitmap atlas behavior into `_imagingft`.
- If legacy bitmap font parity is expanded, do it through
  `ImageFont.load_default_imagefont()` / `ImageFont.load()` and `pilfont`
  fixtures.

Expected value:

- The imagingft coverage target now measures only Pillow `_imagingft.c`
  FreeType-compatible code.

Risk:

- Adding bitmap behavior back to this file would make 100% imagingft coverage
  untrustworthy again.

### 8. Legacy bitmap start clipping

Current state:

- `shift_bitmap_mask` was removed with the Rust-only bitmap atlas path.
- TrueType `getmask2_with_start` rows continue to cover Pillow `_imagingft.c`
  start behavior through live oracle output.

Needed inputs:

- If a real legacy bitmap oracle suite is added later, add rows under
  `pilfont`, not imagingft:
  - positive integer start;
  - positive fractional start;
  - negative x clipping;
  - negative y clipping;
  - shifted width/height collapse;
  - non-ASCII missing glyph width path.

Expected value:

- Keeps legacy bitmap behavior explicit and separate.

Risk:

- Do not use legacy bitmap rows to claim `_imagingft` parity.

### 9. `pack_rgba` overflow/defensive branches

Current state:

- Normal and zero-height paths are covered.
- Allocation overflow guard remains uncovered.

Needed decision:

- Public Pillow cannot practically allocate a mask large enough to hit this
  branch without failing elsewhere. Do not create a fake Rust-only unit test
  and count it as parity.
- Prefer refactoring the overflow guard into a small non-parity safety helper
  with unit coverage, or mark it outside the imagingft parity region target if
  the coverage tool supports source filtering.

Expected value:

- Keeps parity coverage honest while preserving safety coverage elsewhere.

Risk:

- If the project insists on 100% region coverage for the raw file as-is, this
  path likely requires a synthetic C-backed mask-size oracle, not a public
  Pillow input.

### 10. Negative pitch and unsupported `FT_Bitmap.pixel_mode`

Current state:

- `bitmap_coverage` has branches for:
  - negative pitch;
  - mono bit extraction;
  - gray byte copy;
  - unsupported pixel mode returns `None`.
- Current rows cover gray and mono enough for success paths, not all guards.

Freetype assets to probe:

- `sbit_bgra_format1.ttf`
- `sbit_composite_bgra_success_format8.ttf`
- `sbit_unsupported_bit_depth_format1.ttf`
- `sbit_unsupported_image_format.ttf`
- `sbit_gray_index_format4_sparse_miss.ttf`
- `sbit_mono_index_format5_sparse_miss.ttf`

Needed inputs:

- Add SBIT rows only if Pillow exposes the exact bitmap mode via
  `_imagingft` public calls and Rust can compare exactly.
- If public Pillow converts/normalizes the mode before the tested layer,
  create a lower-level C-backed FreeType bitmap fixture or move this helper
  into `pillow-rs-freetype` where such fixtures already exist.

Expected value:

- Could cover lines 676-692 and several row/column skip guards.

Risk:

- Unsupported modes may be unreachable from Pillow’s public `_imagingft`
  surface. Do not force with Rust-only fabricated `FT_Bitmap`.

## Proposed execution order

1. Add a small probe script under `pillow-rs/scripts/` that:
   - reads candidate fonts from `pillow-rs-freetype/tests/fixtures`;
   - executes the live Pillow oracle for `getname`, `getmetrics`, `getbbox`,
     `getmask`, `getbbox_binary`, `getmask2`, and `render_text_binary`;
   - emits only candidate input rows, not expected output.
2. Add and run the variable-font `has_variations == true` row.
3. Add scalar-only CFF/CID rows.
4. Add SBIT rows in tiny batches:
   - one mono;
   - one gray;
   - one BGRA/color;
   - one missing/sparse/unsupported candidate.
5. After every batch:
   - run `make -C pillow-rs font-tests`;
   - run Coverage MCP `imagingft-tests-coverage-fixed compatibility registration`;
   - query `pillow-rs/src/font/imagingft.rs`;
   - keep only rows that exactly match the live oracle.
6. Keep the removed bitmap ownership decision closed:
   - `_imagingft` is FreeType-only;
   - legacy bitmap font behavior is `pilfont` only.
7. For any remaining helper-only defensive regions:
   - either introduce C-backed synthetic fixtures, or
   - move/refactor them outside the imagingft parity target and cover them as
     safety logic, not parity.

## Acceptance checklist for eventual 100%

- `make -C pillow-rs font-tests` passes.
- Coverage MCP `font-tests-coverage-with-freetype` passes and ingests a
  snapshot.
- `coverage_query(view="file", file_path=...)` reports 100% region coverage
  for every active `pillow-rs/src/font/` file participating in
  `PIL.ImageFont` parity, including `pilfont.rs`, `mod.rs`,
  `default_aileron.rs`, and the FreeType-backed `imagingft.rs` route:
  - `covered_regions == total_regions`;
  - no remaining uncovered or partial branch ranges.
- Every new fixture row is input-only and live-oracle-generated.
- Every synthetic route has an equivalent C/Pillow oracle route.
- Any code moved out of `font/imagingft.rs` is documented with why it is not part
  of Pillow `_imagingft.c` parity.

## Current blocker statement

The direct public `_imagingft` fixture corpus alone cannot prove 100% region
coverage for the current `font/imagingft.rs` file layout. The remaining regions
mix true `_imagingft.c` behavior, legacy Rust bitmap-font compatibility,
FreeType defensive errors, and helper safety guards. Reaching 100% honestly
requires either:

1. more Pillow-loadable freetype-derived font inputs for the real public paths;
2. a new lower-level C-backed synthetic oracle harness for otherwise
   unreachable FreeType bitmap/glyph states; and/or
3. refactoring non-`_imagingft.c` bitmap/safety logic out of the measured
   `font/imagingft.rs` parity target.
