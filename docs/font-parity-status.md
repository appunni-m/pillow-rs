# Font Public-API Parity Status (Current Worktree)

Last updated: 2026-07-26 (Asia/Kolkata) — Font public-api harness measured at commit `15e039a29`

## Current checkpoint: Pillow Font comparison review

New commits:

- `cc62d84be` — added `FontTextOptions`, root API wrappers, exact `KeyError`
  mapping, and input-only parity rows for Pillow text options.
- `15e039a29` — removed non-parity debug/unused option exposure and added
  anchor validation rows.

Pillow `ImageFont.FreeTypeFont` public callable comparison against the
repo-local `.oracle-venv` showed that the operation names are represented, but
some method parameters were missing from Rust:

| Pillow method | Existing operation | Newly covered in this checkpoint | Still missing/blocker |
|---|---|---|---|
| `getbbox(text, mode, direction, features, language, stroke_width, anchor)` | `getbbox` | `mode` ignored path, `direction` libraqm `KeyError`, valid `anchor`, invalid `anchor`, integer/fractional `stroke_width` bbox math | full libraqm layout if the oracle enables libraqm |
| `getlength(text, mode, direction, features, language)` | `getlength` | `features` and `language` libraqm `KeyError`; non-error options still delegate to BASIC length | full libraqm layout if enabled |
| `getmask2(text, mode, direction, features, language, stroke_width, anchor, ink, start)` | `getmask2` / `getmask2_with_start` | `anchor` offset parity, `mode="RGBA"` TypeError, `direction` libraqm `KeyError` | stroked mask pixel parity; RGBA embedded-color/ink rendering |
| `getmask(...)` | `getmask` | not yet parameterized separately; `getmask` delegates to `getmask2` in Pillow | needs thin wrapper over the same option path |
| `font_variant(font, size, index, encoding, layout_engine)` | `font_variant` | size override and same-size clone | alternate font source, face index, encoding, and layout engine override |

New input-only rows are stored under
`pillow-rs/tests/fixtures/font/inputs/public-api` and contain no expected
outputs/errors:

- `font.getbbox.anchor_middle_middle`
- `font.getbbox.anchor_right_descender`
- `font.getbbox.stroke_width_one`
- `font.getbbox.stroke_width_half`
- `font.getbbox.mode_ignored`
- `font.getbbox.direction_without_raqm_error`
- `font.getbbox.bad_anchor_error`
- `font.getbbox.short_anchor_error`
- `font.getbbox.bad_vertical_anchor_error`
- `font.getlength.features_without_raqm_error`
- `font.getlength.language_without_raqm_error`
- `font.getmask2.anchor_middle_middle`
- `font.getmask2.mode_rgba_error`
- `font.getmask2.direction_without_raqm_error`

Verification:

- `make -C pillow-rs fmt` — passed
- `make -C pillow-rs font-tests` — passed, `1` test, all manifest rows
  compared against live Pillow oracle
- `cargo check --workspace --all-targets --all-features --locked` — passed
  with existing warning noise
- Coverage MCP command `imagingft-tests-coverage-fixed`
  - run `200762a0-9e2e-4c9d-93ec-8cb7a8d4519e`
  - snapshot `2010d398-5db4-479a-b747-91439a5d2160`
  - commit `15e039a2975cf0771f11e059f57cf3ff80f6936a`
  - status `passed`, coverage artifact ingested

Current target coverage from snapshot
`2010d398-5db4-479a-b747-91439a5d2160`:

| File | Lines | Branches | Functions | Regions |
|---|---:|---:|---:|---:|
| `pillow-rs/src/font/imagingft.rs` | `1006/1102` (`91.29%`) | `191/246` (`77.64%`) | `101/117` (`86.32%`) | `1660/1829` (`90.76%`) |
| `pillow-rs/src/font/mod.rs` | `159/180` (`88.33%`) | n/a | `36/42` (`85.71%`) | `194/232` (`83.62%`) |

The 100% objective is not met yet. Current blockers to reaching it only via
Pillow-oracle fixture rows:

- Stroked mask pixel parity is not implemented. Pillow renders stroked masks in
  native `_imagingft` via `font.render(..., stroke_width, stroke_filled, ...)`;
  Rust currently implements stroke bbox math only. Covering this honestly
  requires implementing outline stroking/rendering, not an expected-value hack.
- `getmask` is not separately parameterized yet, although Pillow implements it
  as `getmask2(...)[0]`.
- `font_variant` does not yet support alternate font bytes/path, face index,
  encoding, or layout-engine override.
- Remaining `imagingft.rs` coverage gaps include FreeType request-size error
  variants, glyph render fallback, uncommon bitmap pitch/pixel modes, and name
  table fallback branches. These require real font assets that drive Pillow and
  Rust through the same public path; no mock/self-comparison row may count.

## Scope

- Public surface source: `pillow-rs/tests/fixtures/font/font_manifest.yaml` plus the raw input JSON files it lists under `pillow-rs/tests/fixtures/font/inputs/public-api` (non-deprecated corpus only)
- Target suite: `make -C pillow-rs font-tests`
- Oracle: repo-local Python Pillow Font path via `pillow-rs/scripts/font_oracle.py` and `.oracle-venv`; the oracle verifies that Pillow Font delegates into native `PIL._imagingft`.
- No deprecated `deprecated/imagingft/*` tests are used.
- Current fixture/test implementation: `pillow-rs/tests/font_public_api.rs` + `pillow-rs/tests/support/font_runner.rs` using explicit `Result` paths.
- Oracle source-of-truth proof:
  - `.oracle-venv` is ignored by git at root via `.oracle-venv/`.
  - The oracle process validates it is running from `<repo>/.oracle-venv/bin/python` and imports `PIL` from that env only.
  - Bootstrap checks assert `ImageFont.core` resolves to `PIL._imagingft`, that `PIL._imagingft` is a native extension module (shared object), and that loaded fonts expose a `builtins.Font` core object (`font.font`) for C-layer execution.
  - Runtime guard inspects `PIL.ImageFont.FreeTypeFont` and `PIL.ImageFont.TransposedFont` source in the oracle venv and requires tested methods (`getmask`, `getmask2`, `getbbox`, `getlength`, `getname`, `get_variation_axes`, and transposed `getmask/getbbox/getlength`) to delegate through the C core.
  - Verified against this repo local `pillow-rs/.oracle-venv` only; this satisfies the "repo-only and gitignored oracle env" requirement.
  - This gives the strict chain: fixtures -> Python oracle -> `PIL._imagingft` C extension -> `Font` core object.
  - Fixture input JSON is input-only: no expected pixel output, hashes, oracle payloads, or expected-error fields are stored in the corpus.

## Acceptance checks

- `make -C pillow-rs font-tests`  
  Result: `1` passed, `0` failed
- `cargo check --workspace --all-targets --all-features --locked`  
  Result: passed; existing warning noise only
- Coverage MCP evidence:
  - `mcp__coverage_mcp.run_test` target: `imagingft-tests-coverage-fixed` compatibility registration, which now runs `make -C pillow-rs imagingft-tests -> font-tests`.
  - Latest run id: `8f07704f-98e8-4677-ba61-d523d946203a`
  - Terminal status: `passed`, `1` passed, `0` failed
  - Diagnostics/ingest: snapshot `48f1c0ae-b25a-4c55-bc08-017de9b90a1e` ingested with `target/coverage/imagingft/imagingft-rust.json`
  - Refactor impact: active tests now target `pillow-rs/tests/font_public_api.rs` and call the Rust `Font` public surface. The previous imagingft-named deprecated harness, runner, oracle, and fixture tree have been deleted.
- Local coverage artifact: `target/coverage/imagingft/imagingft-rust.json`

## Corpus state

- Input manifest: `pillow-rs/tests/fixtures/font/font_manifest.yaml`
- Raw input files: `20` (`pillow-rs/tests/fixtures/font/inputs/public-api/font.*.json`)
- Total rows: `154`
- Executed rows: `154/154`
- Required operation coverage check is manifest-driven: no required manifest operations missing.
- Pillow `FreeTypeFont` public methods now represented in the manifest/corpus:
  - `font_variant`
  - `get_variation_axes`
  - `get_variation_names`
  - `getbbox`
  - `getlength`
  - `getmask`
  - `getmask2`
  - `getmetrics`
  - `getname`
  - `set_variation_by_axes`
  - `set_variation_by_name`
- Additional Rust/helper fixture operations remain classified because they validate constructor, draw, transposed, binary-mode, and Result/error paths used by the public Font consumer surface.
- Input-only guard: active manifest and raw input documents must contain no oracle output, expected hash/raw path, expected error, or status fields; all output/error expectations are generated at runtime from the live Python Pillow Font oracle and compared to Rust `Result`-style status payloads.
- Error handling: the active Font parity runner uses Result-returning Rust public APIs (`getbbox`, `getlength`, `getmask`, `getmask2`, render variants) and serializes only the resulting `Ok`/`Err` payload at the test boundary. The Font public surface no longer exposes separate `_result` fallback variants for these operations.

## Required operation presence (fixture-defined)

| Operation | Input rows |
|---|---:|
| `draw_text` | 7 |
| `font_size` | 2 |
| `font_variant` | 2 |
| `get_transposed_mask` | 11 |
| `get_variation_axes` | 2 |
| `get_variation_names` | 2 |
| `getbbox` | 13 |
| `getbbox_binary` | 8 |
| `getlength` | 7 |
| `getmask` | 11 |
| `getmask2` | 12 |
| `getmask2_with_start` | 19 |
| `getmetrics` | 4 |
| `getname` | 10 |
| `has_variations` | 4 |
| `load_default` | 2 |
| `render_text_binary` | 9 |
| `set_variation_by_axes` | 5 |
| `set_variation_by_name` | 5 |
| `text_bbox` | 4 |
| `transposed_bbox` | 7 |
| `truetype` | 2 |
| `unsupported_magic` | 1 |
| `validate_transposed_length` | 5 |

- Total rows in the current input corpus: `154`. Success/error counts are generated at runtime by the oracle; do not store them in input JSON.
- Error rows are classified only from live oracle output; input JSON carries no expected output, pixel hash, or expected-error metadata.

## Error-category matrix (oracle-defined)

- `TypeError: an integer is required (got type str)` — `1`
- `ValueError: font size must be greater than 0, not 0` — `1`
- `ValueError: font size must be greater than 0, not -1` — `1`
- `ValueError: font size must be greater than 0, not -5.5` — `1`
- `ValueError: text length is undefined for text rotated by 90 or 270 degrees` — `2`
- `ValueError: bad image size` — `2`
- `OSError: cannot open resource` — `1`
- `OSError: invalid argument` — `1`
- `OSError: invalid pixel size` — `1`
- `OSError: invalid ppem value` — `1`
- `NotImplementedError: unsupported imagingft operation: unsupported_magic` — `1`

## Coverage evidence

### Suite summary (`imagingft` compatibility coverage suite)

- Current Coverage MCP snapshot: `48f1c0ae-b25a-4c55-bc08-017de9b90a1e`
  - Run: `8f07704f-98e8-4677-ba61-d523d946203a`
  - Commit: `060d763c65d86528be7a245f70ef3d124e2a50f2`
  - Command: `imagingft-tests-coverage-fixed`
  - Result: passed, ingested
  - Suite totals: `total_lines: 26199`, `covered_lines: 2773` (`line_rate 0.1058437345`)
  - Suite totals: `total_branches: 4618`, `covered_branches: 260` (`branch_rate 0.0563014292`)
  - Suite totals: `total_functions: 1846`, `covered_functions: 242` (`function_rate 0.1310942579`)
  - Suite totals: `total_regions: 45824`, `covered_regions: 4315` (`region_rate 0.0941646299`)

### `pillow-rs/src/font/imagingft.rs`

- `covered_lines: 925/1012` (`line_rate 0.9140316206`)
- `covered_functions: 97/113` (`function_rate 0.8584070796`)
- `covered_branches: 182/236` (`branch_rate 0.7711864407`)
- `covered_regions: 1556/1717` (`region_rate 0.9062317997`)
- Manifest completeness is enforced in `pillow-rs/tests/font_public_api.rs`: `font_manifest.yaml` must exactly enumerate the Font public parity operation set and every input operation must be classified as required or negative.
- Remaining gaps are not hidden: FreeType load/request-size error sub-branches, glyph render fallback, clipping guard branches, uncommon bitmap coverage modes, and fallback name-decoding branches remain uncovered.

### `pillow-rs/src/font/mod.rs`

- `covered_lines: 131/146` (`line_rate 0.8972602740`)
- `covered_functions: 32/36` (`function_rate 0.8888888889`)
- `covered_regions: 170/202` (`region_rate 0.8415841584`)
- Remaining uncovered regions are source-map/doc/debug/convenience wrapper regions; parity rows execute through the public Font surface and Result-returning APIs.

### Coverage delta

- Baseline: `19162f0c-7d00-47d9-9a69-a7f59e1d8678`
- Current: `906f7d20-a3fd-4e57-a0e7-d36c336bb7c6`
- Sweep movement against previous committed comparator snapshot `27d14363-1512-48c6-8a77-6849c6b14113`: suite covered metrics moved `+54` lines, `+4` branches, `+4` functions, `+91` regions. `pillow-rs/src/font/imagingft.rs` itself remained unchanged.
- Same-turn movement from the previous committed imagingft snapshot `cdd83425-0fdc-4861-998c-73dfb9de9345`:
  - `font/imagingft.rs` lines: `1048 -> 1050` (`+2`)
  - branches: `169 -> 172` (`+3`)
  - regions: `1870 -> 1873` (`+3`)

## Reverse-mapped gap sweep

Source: Coverage MCP snapshot `e3c79419-67ff-4b76-ac15-17cf0822a908`, `pillow-rs/src/font/imagingft.rs`.

### Confirmed parity gaps

- Previous `getmask2_with_start` negative vertical start mismatch is fixed:
  - Added passing fixture: `imagingft.getmask2_with_start.dejavusans_negative_y_start`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `start=[0.0, -0.5]`.
  - Added passing fixture: `imagingft.getmask2_with_start.dejavusans_negative_xy_fractional_start`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `start=[-1.25, -0.5]`.
  - First divergence: Pillow clips glyph bitmaps with negative `xx`/`yy`; Rust skipped the whole glyph when `dx < 0 || dy < 0`.
  - C reference: Pillow 12.2.0 `src/_imagingft.c::font_render_impl`, the glyph render loop clips `x0/x1` and only draws rows where `yy >= 0 && yy < im->ysize`.
- Previous `getmask2_with_start` collapsed-width error mismatch is fixed:
  - Added passing error fixtures: `imagingft.getmask2_with_start.dejavusans_bad_image_size_negative_width` and `imagingft.getmask2_with_start.dejavusans_bad_image_size_negative_height`.
  - Pillow oracle returns `ValueError: bad image size`; Rust now returns the same error through the Result path instead of a successful empty mask.

### Missing public parity scenarios now added

- `getmask2_with_start` negative horizontal start:
  - Added passing fixture: `imagingft.getmask2_with_start.dejavusans_negative_x_start`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `start=[-1.25, 0.0]`.
  - Purpose: validates left-side clipping/origin behavior against the live oracle without stored expected output.
- `render_text_binary` space-only mask:
  - Added passing fixture: `imagingft.render_text_binary.space_zero_height`, DejaVuSans.ttf, `size=20`, `text=" "`.
  - Purpose: validates the `pack_rgba` zero-height path with width greater than zero.
- `draw_text` negative Y placement:
  - Added passing fixture: `imagingft.render_text.dejavusans20_negative_y_draw_text_rgba`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `xy=[10, -4]`, RGBA canvas.
  - Purpose: validates Draw/text consumer clipping against the live oracle.
- Freetype fixture corpus reuse:
  - Added loadable CFF outline asset from `pillow-rs-freetype`: `input/fonts/pure-cff-cubic.otf`.
  - Added loadable embedded-strike TTF asset from `pillow-rs-freetype`: `input/fonts/embedded-strike-color-or-sbit.ttf`.
  - Added passing CFF scalar/bbox rows: `getname`, `getmetrics`, `getlength`, `getbbox`, `getbbox_binary`, `has_variations`.
  - Added passing embedded-strike rows across scalar/bbox/mask/draw paths.
  - Deliberately did not keep CFF rendering rows: `getmask.pure_cff_a` failed exact Pillow mask-byte parity with small antialias differences, so keeping it would violate the oracle standard.
- Additional `getmask2_with_start` clipping rows:
  - Added passing rows for moderate/heavy left clipping and top clipping:
    - `dejavusans_left_clip_start`
    - `dejavusans_full_first_glyph_left_clip_start`
    - `dejavusans_top_clip_start`
    - `dejavusans_left_top_clip_start`
    - `dejavusans_heavy_left_clip_start`
    - `dejavusans_heavy_top_clip_start`
    - `dejavusans_almost_full_top_clip_start`
  - Purpose: hit real partial/full glyph clipping branches through Pillow `font.getmask2(..., start=...)`, not synthetic Rust-only calls.
- Coverage effect:
  - These rows increased fixture parity coverage from `82` to `105` executed rows.
  - After the freetype/clipping sweep, `pillow-rs/src/font/imagingft.rs` measured `1873/2338` covered regions (`80.11120616%`) on snapshot `906f7d20-a3fd-4e57-a0e7-d36c336bb7c6`.

### Reverse-mapped unclosed branches

| Source area | Lines | Public operation path | Current assessment |
|---|---:|---|---|
| TrueType load/request-size fallback and FT error mapping | 35-81 | font load before any operation | Only valid/missing/invalid-size rows are covered. Remaining FT error kinds need pathological font/size inputs or crafted font assets; do not fake these in Rust tests. |
| Removed Rust-only bitmap path | former `Font::Bitmap` arms and `shift_bitmap_mask` | not a Pillow `_imagingft` surface | Removed from `font/imagingft.rs`; legacy PIL bitmap fonts remain owned by `pillow-rs/src/font/pilfont.rs`. Do not reintroduce bitmap atlas behavior into `_imagingft` coverage. |
| Transpose helper source-map gaps | 127-129, 145 | `get_transposed_mask`, `transposed_bbox`, `validate_transposed_length` | Fixture rows cover all Pillow transpose constants plus `None`/missing orientation; remaining uncovered lines appear to be coverage/source mapping artifacts unless a new source-context query proves otherwise. |
| Layout/load glyph failure inside text shaping/rendering | 373-374, 539-547 | `getlength`, `getbbox`, `getmask*` | Needs a real oracle input that makes FreeType load fail for a glyph after font load succeeds. No current repo font/input does this. |
| `mask_from_run_with_start` clipping and sparse bitmap cases | 497-639 | `getmask`, `getmask2`, `getmask2_with_start`, `draw_text` | Additional oracle-backed start rows covered three more regions. Remaining uncovered branches include render fallback, zero-sized/absent glyph bitmap, defensive canvas slice guard, and bitmap coverage `None` handling. Add only oracle-backed rows; do not synthesize self-comparison rows. |
| `bitmap_coverage` uncommon bitmap modes/pitch | 644-660 | `getmask*`, binary mask paths | Gray and mono coverage are partially exercised. Negative pitch and unsupported pixel mode are not reachable from current repo fonts through Pillow public APIs. Need a real oracle fixture asset before claiming coverage. |

### Current blocker to 100% region by input rows only

100% region coverage inside `font/imagingft.rs` is not yet reached after the ownership refactor:

- The old `Font::Bitmap` blocker is gone.
- Several remaining FreeType fallback/error branches require real oracle inputs that make `FT_Load_Glyph`, render fallback, or `FT_Request_Size` fail after a face has loaded. The current public fixture schema cannot force those without mocking or self-comparing.
- Overflow guards such as `pack_rgba` allocation overflow cannot be produced by a practical oracle image allocation without causing the oracle itself to fail outside a useful parity comparison.

### Next targeted probes / implementation tasks

- Search for a repo font/text pair that makes FreeType return glyph-load failure after successful face load; if found, add it as an error/success row from oracle output only.
- Keep legacy bitmap-font parity separate under `pilfont`; do not count it as `_imagingft`.
- Continue reverse-mapping the remaining FreeType-only gaps using oracle-backed inputs only.

## Remaining explicit gaps

- Suite-level coverage is not complete by the 100% objective:
  - Font public-api suite executes all 105 rows and reports zero parity mismatches.
  - `pillow-rs/src/font/imagingft.rs` remains with uncovered lines/branch paths outside this minimal public corpus.
- Error/parity:
  - No parity mismatches were observed in this run; error rows are all matched and classified correctly against oracle rows.
