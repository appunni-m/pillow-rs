# ImagingFT 100% Region Coverage Plan

Last updated: 2026-07-26 (Asia/Kolkata)

## Execution status update: 2026-07-26

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

Verified gates:

- `make -C pillow-rs imagingft-tests` passes after each committed batch.
- Coverage MCP command `imagingft-tests-coverage-fixed` passed and ingested
  after each committed batch.

Coverage snapshots:

| Commit | Snapshot | `imagingft.rs` regions | Branches | Notes |
|---|---|---:|---:|---|
| `88504ef6e` | `e77930c8-41c2-4eb2-b943-4ce8ca2066cf` | `1873/2338` (`80.11%`) | `172/258` | pre-error-row baseline used for this execution |
| `f0d02db78` | `b32d6e80-3e98-4e85-ade4-28b416790e97` | `1851/2294` (`80.69%`) | `174/260` | Result propagation changed denominator |
| `a460c1e27` | `f1c881aa-ebc5-40ea-a8ba-371a8418f355` | `1800/2243` (`80.25%`) | `178/262` | bytecode error rows/fixes |
| `550787522` | `d8402aae-43ae-469d-8efd-c01666ce5af8` | `1875/2330` (`80.47%`) | `181/266` | missing-name row/fix |

Current blocker to literal 100% region coverage:

- A substantial remaining region block is `Font::Bitmap` behavior inside
  `pillow-rs/src/font/imagingft.rs`, including `shift_bitmap_mask`. The
  current imagingft public-api loader intentionally exercises Pillow
  `_imagingft`/`ImageFont.truetype`, which creates `Font::TrueType`, not this
  Rust-only `Font::Bitmap` enum variant. Covering these lines with Rust-only
  construction would violate the Pillow-oracle rule and would not count as
  trusted parity.
- Therefore, reaching literal 100% for this file under the current suite
  requires one of these design changes:
  1. move bitmap-font compatibility code out of `imagingft.rs` so the
     `_imagingft` region target contains only oracle-reachable TrueType paths;
  2. add a separate Pillow bitmap-font oracle route and classify it separately
     from `_imagingft` TrueType parity;
  3. exclude bitmap-only compatibility lines from the imagingft region target
     with an explicit, reviewed coverage policy.

Rejected during execution:

- `name_table_bad_storage.ttf` was not kept in the imagingft public corpus.
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
- output and error expectations come from the repo-local live Pillow
  `_imagingft` oracle at runtime;
- no Rust test may compare Rust to itself;
- no expected pixel hex, hashes, status, or error text may be stored in the
  input corpus;
- synthetic cases are allowed only when the synthetic object/path is also
  executed against an equivalent C/Pillow oracle and the comparison remains
  exact.

## Current evidence baseline

- Fixture suite: `make -C pillow-rs imagingft-tests`
- Coverage MCP command: `imagingft-tests-coverage-fixed`
- Latest measured source snapshot: `906f7d20-a3fd-4e57-a0e7-d36c336bb7c6`
- Measured commit for source/fixtures: `1b6baf751e0073be461ca9433cb1bb54c0f09ac3`
- Current corpus: `105` rows across `17` input files
- Current `pillow-rs/src/font/imagingft.rs` metrics:
  - lines: `1050/1286` (`81.64852255%`)
  - branches: `172/258` (`66.66666667%`)
  - functions: `108/122` (`88.52459016%`)
  - regions: `1873/2338` (`80.11120616%`)

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

Coverage MCP reports 81 remaining relevant gap groups in `imagingft.rs`.
The line numbers below are from snapshot `906f7d20-a3fd-4e57-a0e7-d36c336bb7c6`.

| Gap bucket | Lines | Why uncovered | Route to cover |
|---|---:|---|---|
| Load success/error sub-branches | 35-36, 49, 51 | specific FreeType request-size and face-load error codes not yet produced by current inputs | add malformed/pathological font or size rows only when live Pillow emits the same public error |
| `ft_error_to_pil` fallback mapping | 74-83 | glyph-load/render errors after successful face load are not currently reachable | need a font/text pair that loads the face but fails during `FT_Load_Glyph`, or a C-backed synthetic glyph-load fixture |
| Transpose source-map artifacts | 129, 145 | already has all public orientations; line-level artifacts remain | verify with exact line records; likely no new input needed |
| `Font::Bitmap` public arms | 158-162, 183-185, 193-194, 207-209, 216-218, 246, 261, 283, 640-671 | imagingft fixture loader currently creates `Font::TrueType` only; Pillow `_imagingft` does not produce this Rust enum variant | needs separate Pillow `ImageFont.load_default_imagefont`/bitmap oracle route, or move bitmap compatibility out of `imagingft.rs` |
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
| Rust-only helper/unit path | No for parity | A test manually constructs Rust values, forces branches, and asserts local behavior without C/Pillow oracle. This may be useful for safety tests but must not count toward ImagingFT parity coverage. |

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
  `pillow-rs/tests/fixtures/imagingft/input/fonts/`.
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

### 7. `Font::Bitmap` arms inside `imagingft.rs`

Current state:

- These branches are Rust-visible but not covered by the current
  `_imagingft` fixture loader.
- `Font::load_default(size)` intentionally returns `Font::TrueType`, using
  Pillow’s embedded Aileron TTF subset.
- `BitmapFont::new(size)` exists, but no Pillow `_imagingft` route creates an
  equivalent native C `Font` object.

Needed decision:

1. If `imagingft.rs` means only Pillow `_imagingft.c` TrueType behavior:
   - move bitmap arms out of `imagingft.rs` or split them behind a separate
     module so the imagingft coverage target does not include non-imagingft
     code.
2. If bitmap compatibility must remain in this file:
   - create a separate bitmap-font parity harness against Pillow
     `ImageFont.load_default_imagefont()` / legacy bitmap font behavior;
   - add a new fixture font kind such as `load_legacy_bitmap_default`;
   - compare getname, metrics, length, bbox, mask, mask2 start shift, and
     render-binary outputs against that Pillow bitmap oracle.

Expected value:

- This is mandatory for 100% region coverage unless the bitmap arms are moved
  out of `imagingft.rs`.

Risk:

- Counting Rust-only `BitmapFont::new()` tests would inflate coverage but
  would not prove Pillow parity. Do not do that.

### 8. `shift_bitmap_mask` start clipping

Current state:

- The function is only reached through `Font::Bitmap`.
- Current TrueType `getmask2_with_start` rows do not cover it.

Needed inputs:

- Depends on the `Font::Bitmap` decision above.
- Once a real bitmap oracle exists, add rows:
  - positive integer start;
  - positive fractional start;
  - negative x clipping;
  - negative y clipping;
  - shifted width/height collapse;
  - non-ASCII missing glyph width path.

Expected value:

- Covers lines 640-671 and the bitmap branch in `getmask2_with_start_result`.

Risk:

- The current `shift_bitmap_mask` behavior may not match Pillow legacy bitmap
  behavior. Treat mismatches as implementation bugs, not fixture changes.

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
   - run `make -C pillow-rs imagingft-tests`;
   - run Coverage MCP `imagingft-tests-coverage-fixed`;
   - query `pillow-rs/src/font/imagingft.rs`;
   - keep only rows that exactly match the live oracle.
6. Decide the `Font::Bitmap` ownership question:
   - split from `imagingft.rs`, or
   - add a separate bitmap oracle harness.
7. For any remaining helper-only defensive regions:
   - either introduce C-backed synthetic fixtures, or
   - move/refactor them outside the imagingft parity target and cover them as
     safety logic, not parity.

## Acceptance checklist for eventual 100%

- `make -C pillow-rs imagingft-tests` passes.
- Coverage MCP `imagingft-tests-coverage-fixed` passes and ingests a snapshot.
- `coverage_query(view="file", file_path="pillow-rs/src/font/imagingft.rs")`
  reports:
  - `covered_regions == total_regions`;
  - no remaining uncovered or partial branch ranges.
- Every new fixture row is input-only and live-oracle-generated.
- Every synthetic route has an equivalent C/Pillow oracle route.
- Any code moved out of `imagingft.rs` is documented with why it is not part
  of Pillow `_imagingft.c` parity.

## Current blocker statement

The direct public `_imagingft` fixture corpus alone cannot prove 100% region
coverage for the current `imagingft.rs` file layout. The remaining regions
mix true `_imagingft.c` behavior, legacy Rust bitmap-font compatibility,
FreeType defensive errors, and helper safety guards. Reaching 100% honestly
requires either:

1. more Pillow-loadable freetype-derived font inputs for the real public paths;
2. a new lower-level C-backed synthetic oracle harness for otherwise
   unreachable FreeType bitmap/glyph states; and/or
3. refactoring non-`_imagingft.c` bitmap/safety logic out of the measured
   `imagingft.rs` parity target.
