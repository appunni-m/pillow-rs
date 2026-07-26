# Font Public-API Parity Status (Current Worktree)

Last updated: 2026-07-26 (Asia/Kolkata) — Font public-api harness measured

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
- Coverage MCP evidence:
  - `mcp__coverage_mcp.run_test` target: `imagingft-tests-coverage-fixed` compatibility registration, which now runs `make -C pillow-rs imagingft-tests -> font-tests`.
  - Latest run id: `79e4bdfa-4a7a-43bf-aa8a-f064f178078c`
  - Terminal status: `passed`, `1` passed, `0` failed
  - Diagnostics/ingest: `e3c79419-67ff-4b76-ac15-17cf0822a908` ingested with `target/coverage/imagingft/imagingft-rust.json`
  - Refactor impact: active tests now target `pillow-rs/tests/font_public_api.rs` and call the Rust `Font` public surface. The previous imagingft-named public harness, runner, oracle, and fixture tree are preserved under `pillow-rs/tests/deprecated/imagingft/current-public-api/`.
  - Prior same-turn probe snapshots:
    - `6b68edcf-1aa9-474f-8f85-9adb95291899`: freetype CFF/embedded-strike rows added; no region movement.
    - `68db7f03-2c6e-4099-a17d-d0736f537be6`: moderate clipping rows added; `pillow-rs/src/font/imagingft.rs` moved to `1872/2338` regions.
- Local coverage artifact: `target/coverage/imagingft/imagingft-rust.json`

## Corpus state

- Input manifest: `pillow-rs/tests/fixtures/font/font_manifest.yaml`
- Raw input files: `17` (`pillow-rs/tests/fixtures/font/inputs/public-api/font.*.json`)
- Total rows: `105`
- Executed rows: `105/105`
- Required operation coverage check is manifest-driven: no required manifest operations missing.
- Input-only guard: active manifest and raw input documents must contain no oracle output, expected hash/raw path, expected error, or status fields; all output/error expectations are generated at runtime from the live Python Pillow Font oracle and compared to Rust `Result`-style status payloads.
- Error handling: the active Font parity runner uses fallible Rust APIs (`getbbox_result`, `getlength_result`, `getmask_result`, `getmask2*_result`, render/result variants) and serializes only the resulting `Ok`/`Err` payload at the test boundary. Non-Result convenience methods remain only for compatibility and are not used as trusted parity comparisons.

## Required operation presence (fixture-defined)

| Operation | OK | Error | Total |
|---|---:|---:|---:|
| `draw_text` | 6 | 0 | 6 |
| `get_transposed_mask` | 9 | 1 | 10 |
| `getbbox` | 7 | 2 | 9 |
| `getbbox_binary` | 7 | 0 | 7 |
| `getlength` | 6 | 0 | 6 |
| `getmask` | 8 | 0 | 8 |
| `getmask2` | 7 | 0 | 7 |
| `getmask2_with_start` | 17 | 2 | 19 |
| `getmetrics` | 3 | 0 | 3 |
| `getname` | 3 | 5 | 8 |
| `has_variations` | 3 | 0 | 3 |
| `render_text_binary` | 6 | 0 | 6 |
| `transposed_bbox` | 7 | 0 | 7 |
| `unsupported_magic` | 0 | 1 | 1 |
| `validate_transposed_length` | 3 | 2 | 5 |

- Total success rows: `92`
- Total error rows: `13`
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

- Current artifact metrics (`target/coverage/imagingft/imagingft-rust.json`, latest run):
  - `total_lines: 17412`, `covered_lines: 1557` (`line_rate 0.0894210889`)
  - `total_branches: 3048`, `covered_branches: 135` (`branch_rate 0.0442913386`)
  - `total_functions: 1171`, `covered_functions: 127` (`function_rate 0.1084543126`)
  - `total_regions: 30400`, `covered_regions: 2342` (`region_rate 0.0770394737`)

### `pillow-rs/src/font/imagingft.rs`

- `covered_lines: 394/432` (`line_rate 0.9120370370`)
- `covered_functions: 38/43` (`function_rate 0.8837209302`)
- `covered_branches: 86/108` (`branch_rate 0.7962962963`)
- `covered_regions: 687/762` (`region_rate 0.9015748031`)
- Gaps remain in FreeType load/error branches, glyph render fallback, clipping guards, and uncommon bitmap coverage modes. The previous Rust-only bitmap-font blocker has been removed from this file.

### `pillow-rs/src/font/mod.rs`

- `covered_lines: 67/91` (`line_rate 0.7362637363`)
- `covered_functions: 16/23` (`function_rate 0.6956521739`)
- `covered_regions: 80/120` (`region_rate 0.6666666667`)
- Remaining uncovered regions are public convenience wrappers such as `font_size`, `text_bbox`, non-Result `getmask`, non-Result `getname`, non-Result binary bbox/getmask2 wrappers, and `Debug`. The exact-result parity runner intentionally drives the `Result` variants for error truth.

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
