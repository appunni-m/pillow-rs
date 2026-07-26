# ImagingFT Public-API Parity Status (Current Worktree)

Last updated: 2026-07-26 (Asia/Kolkata) — freetype-fixture gap sweep revalidated

## Scope

- Public surface source: `pillow-rs/tests/fixtures/imagingft/inputs/public-api` (non-deprecated corpus only)
- Target suite: `make -C pillow-rs imagingft-tests`
- Oracle: repo-local Pillow C path via `pillow-rs/scripts/imagingft_oracle.py` and `.oracle-venv`
- No deprecated `deprecated/imagingft/*` tests are used.
- Current fixture/test implementation: `pillow-rs/tests/imagingft_public_api.rs` + `pillow-rs/tests/support/imagingft_runner.rs` using explicit `Result` paths.
- Oracle source-of-truth proof:
  - `.oracle-venv` is ignored by git at root via `.oracle-venv/`.
  - The oracle process validates it is running from `<repo>/.oracle-venv/bin/python` and imports `PIL` from that env only.
  - Bootstrap checks assert `ImageFont.core` resolves to `PIL._imagingft`, that `PIL._imagingft` is a native extension module (shared object), and that loaded fonts expose a `builtins.Font` core object (`font.font`) for C-layer execution.
  - Runtime guard inspects `PIL.ImageFont.FreeTypeFont` and `PIL.ImageFont.TransposedFont` source in the oracle venv and requires tested methods (`getmask`, `getmask2`, `getbbox`, `getlength`, `getname`, `get_variation_axes`, and transposed `getmask/getbbox/getlength`) to delegate through the C core.
  - Verified against this repo local `pillow-rs/.oracle-venv` only; this satisfies the "repo-only and gitignored oracle env" requirement.
  - This gives the strict chain: fixtures -> Python oracle -> `PIL._imagingft` C extension -> `Font` core object.
  - Fixture input JSON is input-only: no expected pixel output, hashes, oracle payloads, or expected-error fields are stored in the corpus.

## Acceptance checks

- `make -C pillow-rs imagingft-tests`  
  Result: `1` passed, `0` failed
- Coverage MCP evidence:
  - `mcp__coverage_mcp.run_test` target: `imagingft-tests-coverage-fixed`
  - Latest run id: `8ffb1a4f-93e5-4c3c-868a-a1ad99278148`
  - Terminal status: `passed`, `1` passed, `0` failed
  - Diagnostics/ingest: `906f7d20-a3fd-4e57-a0e7-d36c336bb7c6` ingested with `target/coverage/imagingft/imagingft-rust.json`
  - Prior same-turn probe snapshots:
    - `6b68edcf-1aa9-474f-8f85-9adb95291899`: freetype CFF/embedded-strike rows added; no region movement.
    - `68db7f03-2c6e-4099-a17d-d0736f537be6`: moderate clipping rows added; `imagingft.rs` moved to `1872/2338` regions.
- Local coverage artifact: `target/coverage/imagingft/imagingft-rust.json`

## Corpus state

- Input files: `17` (`pillow-rs/tests/fixtures/imagingft/inputs/public-api/*.json`)
- Total rows: `105`
- Executed rows: `105/105`
- Required operation coverage check against case-set operations: no required manifest operations missing

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

### Suite summary (`imagingft`)

- Current artifact metrics (`target/coverage/imagingft/imagingft-rust.json`, latest run):
  - `total_lines: 18953`, `covered_lines: 2150` (`line_rate 0.1134385058`)
  - `total_branches: 3358`, `covered_branches: 218` (`branch_rate 0.0649195950`)
  - `total_functions: 1313`, `covered_functions: 183` (`function_rate 0.1393754760`)
  - `total_regions: 33078`, `covered_regions: 3455` (`region_rate 0.1044500877`)

### `pillow-rs/src/font/imagingft.rs`

- `covered_lines: 1050/1286` (`line_rate 0.8164852255`)
- `covered_functions: 108/122` (`function_rate 0.8852459016`)
- `covered_branches: 172/258` (`branch_rate 0.6666666667`)
- `covered_regions: 1873/2338` (`region_rate 0.8011120616`)
- Gaps remain in non-error branches and layout branches not yet covered by this public-input subset.

### Coverage delta

- Baseline: `19162f0c-7d00-47d9-9a69-a7f59e1d8678`
- Current: `906f7d20-a3fd-4e57-a0e7-d36c336bb7c6`
- Sweep movement against previous committed comparator snapshot `27d14363-1512-48c6-8a77-6849c6b14113`: suite covered metrics moved `+54` lines, `+4` branches, `+4` functions, `+91` regions. `pillow-rs/src/font/imagingft.rs` itself remained unchanged.
- Same-turn movement from the previous committed imagingft snapshot `cdd83425-0fdc-4861-998c-73dfb9de9345`:
  - `imagingft.rs` lines: `1048 -> 1050` (`+2`)
  - branches: `169 -> 172` (`+3`)
  - regions: `1870 -> 1873` (`+3`)

## Reverse-mapped gap sweep

Source: Coverage MCP snapshot `906f7d20-a3fd-4e57-a0e7-d36c336bb7c6`, `pillow-rs/src/font/imagingft.rs`.

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
| Bitmap-font arms | 156-226, 246, 640-671 | all public methods on `Font::Bitmap` | Current fixture loader creates TrueType fonts only. PCF/WinFNT freetype fixtures are rejected by Pillow `_imagingft` before a native C `Font` object exists, so these branches are not coverable by the current oracle path. Need a real Pillow-compatible bitmap-font surface before parity rows can be trusted. |
| Transpose helper source-map gaps | 127-129, 145 | `get_transposed_mask`, `transposed_bbox`, `validate_transposed_length` | Fixture rows cover all Pillow transpose constants plus `None`/missing orientation; remaining uncovered lines appear to be coverage/source mapping artifacts unless a new source-context query proves otherwise. |
| Layout/load glyph failure inside text shaping/rendering | 373-374, 539-547 | `getlength`, `getbbox`, `getmask*` | Needs a real oracle input that makes FreeType load fail for a glyph after font load succeeds. No current repo font/input does this. |
| `mask_from_run_with_start` clipping and sparse bitmap cases | 497-639 | `getmask`, `getmask2`, `getmask2_with_start`, `draw_text` | Additional oracle-backed start rows covered three more regions. Remaining uncovered branches include render fallback, zero-sized/absent glyph bitmap, defensive canvas slice guard, and bitmap coverage `None` handling. Add only oracle-backed rows; do not synthesize self-comparison rows. |
| `bitmap_coverage` uncommon bitmap modes/pitch | 644-660 | `getmask*`, binary mask paths | Gray and mono coverage are partially exercised. Negative pitch and unsupported pixel mode are not reachable from current repo fonts through Pillow public APIs. Need a real oracle fixture asset before claiming coverage. |

### Hard blocker to 100% region by input rows only

100% region coverage inside `imagingft.rs` cannot be reached honestly with only the current Pillow `_imagingft` public TrueType input corpus:

- `Font::Bitmap` arms are in `imagingft.rs`, but Pillow `_imagingft` oracle cases necessarily load native C `Font` objects through `ImageFont.truetype`/`load_default(size)`, not bitmap fonts.
- Several FreeType fallback/error branches require synthetic internal `FT_Load_Glyph` or unsupported bitmap-mode failures after a face has already loaded. The current public fixture schema cannot force those without mocking or self-comparing.
- Overflow guards such as `pack_rgba` allocation overflow cannot be produced by a practical oracle image allocation without causing the oracle itself to fail outside a useful parity comparison.

### Next targeted probes / implementation tasks

- Search for a repo font/text pair that makes FreeType return glyph-load failure after successful face load; if found, add it as an error/success row from oracle output only.
- Establish whether Pillow exposes a bitmap font object through the same public surface. If not, either remove `Font::Bitmap` from the imagingft public parity target or create a separate, clearly named bitmap-font parity target.
- Consider moving bitmap-only behavior out of `imagingft.rs` if `imagingft.rs` is intended to mean Pillow `_imagingft.c` TrueType parity only; otherwise 100% region coverage for this file requires a separate bitmap oracle, not `_imagingft.c`.

## Remaining explicit gaps

- Suite-level coverage is not complete by the 100% objective:
  - ImagingFT public-api suite executes all 105 rows and reports zero parity mismatches.
  - `pillow-rs/src/font/imagingft.rs` remains with uncovered lines/branch paths outside this minimal public corpus.
- Error/parity:
  - No parity mismatches were observed in this run; error rows are all matched and classified correctly against oracle rows.
