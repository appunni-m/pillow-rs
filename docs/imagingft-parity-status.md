# ImagingFT Public-API Parity Status (Current Worktree)

Last updated: 2026-07-26 (Asia/Kolkata) — reverse-mapped gap sweep revalidated

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
  - Run id: `6ab7aeb8-78ae-446d-9ed4-d3cc8d73c5e1` (first submission `submission_reused=false`)
  - Terminal status: `passed`, `1` passed, `0` failed
  - Diagnostics/ingest: `72fe69d2-cd78-427c-9eab-1e988fe3c243` ingested with `target/coverage/imagingft/imagingft-rust.json`
  - Search log checks (`FAILED`, `error:`, `panic`) returned no failure context; only the normal `0 failed` summary line matched.
- Local coverage artifact: `target/coverage/imagingft/imagingft-rust.json`

## Corpus state

- Input files: `17` (`pillow-rs/tests/fixtures/imagingft/inputs/public-api/*.json`)
- Total rows: `77`
- Executed rows: `77/77`
- Required operation coverage check against case-set operations: no required manifest operations missing

## Required operation presence (fixture-defined)

| Operation | OK | Error | Total |
|---|---:|---:|---:|
| `draw_text` | 5 | 0 | 5 |
| `get_transposed_mask` | 9 | 1 | 10 |
| `getbbox` | 5 | 2 | 7 |
| `getbbox_binary` | 5 | 0 | 5 |
| `getlength` | 4 | 0 | 4 |
| `getmask` | 7 | 0 | 7 |
| `getmask2` | 6 | 0 | 6 |
| `getmask2_with_start` | 8 | 0 | 8 |
| `getmetrics` | 1 | 0 | 1 |
| `getname` | 1 | 5 | 6 |
| `has_variations` | 1 | 0 | 1 |
| `render_text_binary` | 4 | 0 | 4 |
| `transposed_bbox` | 7 | 0 | 7 |
| `unsupported_magic` | 0 | 1 | 1 |
| `validate_transposed_length` | 4 | 1 | 5 |

- Total success rows: `66`
- Total error rows: `11`
- Error rows are classified only from live oracle output; input JSON carries no expected output, pixel hash, or expected-error metadata.

## Error-category matrix (oracle-defined)

- `TypeError: an integer is required (got type str)` — `1`
- `ValueError: font size must be greater than 0, not 0` — `1`
- `ValueError: font size must be greater than 0, not -1` — `1`
- `ValueError: font size must be greater than 0, not -5.5` — `1`
- `ValueError: text length is undefined for text rotated by 90 or 270 degrees` — `2`
- `OSError: cannot open resource` — `1`
- `OSError: invalid argument` — `1`
- `OSError: invalid pixel size` — `1`
- `OSError: invalid ppem value` — `1`
- `NotImplementedError: unsupported imagingft operation: unsupported_magic` — `1`

## Coverage evidence

### Suite summary (`imagingft`)

- Current artifact metrics (`target/coverage/imagingft/imagingft-rust.json`, latest run):
  - `total_lines: 18520`, `covered_lines: 1792` (`line_rate 0.0967602592`)
  - `total_branches: 3266`, `covered_branches: 154` (`branch_rate 0.0471524801`)
  - `total_functions: 1274`, `covered_functions: 146` (`function_rate 0.1145996860`)
  - `total_regions: 32288`, `covered_regions: 2813` (`region_rate 0.0871221506`)

### `pillow-rs/src/font/imagingft.rs`

- `covered_lines: 690/853` (`line_rate 0.808909855`)
- `covered_functions: 71/83` (`function_rate 0.8554216867`)
- `covered_branches: 105/166` (`branch_rate 0.6325301205`)
- `covered_regions: 1228/1548` (`region_rate 0.7932816537`)
- Gaps remain in non-error branches and layout branches not yet covered by this public-input subset.

### Coverage delta

- Baseline: `19162f0c-7d00-47d9-9a69-a7f59e1d8678`
- Current: `72fe69d2-cd78-427c-9eab-1e988fe3c243`
- Sweep movement against previous committed comparator snapshot `27d14363-1512-48c6-8a77-6849c6b14113`: suite covered metrics moved `+54` lines, `+4` branches, `+4` functions, `+91` regions. `pillow-rs/src/font/imagingft.rs` itself remained unchanged.

## Reverse-mapped gap sweep

Source: Coverage MCP snapshot `72fe69d2-cd78-427c-9eab-1e988fe3c243`, `pillow-rs/src/font/imagingft.rs`.

### Confirmed parity gaps

- `getmask2_with_start` negative vertical start:
  - Tried input: DejaVuSans.ttf, `size=20`, `text="Hello"`, `start=[0.0, -0.5]`.
  - Result: Pillow oracle and Rust both returned `ok` with matching `size=[63,19]`, `offset=[0,4]`, and `mode="L"`, but `pixels_hex` differed.
  - Reverse map: `mask_from_run_with_start`, especially origin/clipping branches around lines 497-602 (`start_height`, `y_origin`, `dy < 0`, row clipping, and destination placement).
  - Status: not added to the passing corpus; keep as a fix target.
- `getmask2_with_start` combined negative fractional start:
  - Tried input: DejaVuSans.ttf, `size=20`, `text="Hello"`, `start=[-1.25, -0.5]`.
  - Result: Pillow oracle and Rust both returned `ok` with matching `size=[62,19]`, `offset=[0,4]`, and `mode="L"`, but `pixels_hex` differed.
  - Reverse map: same `mask_from_run_with_start` origin/clipping path; negative X alone passed, so the currently confirmed mismatch is tied to vertical negative start handling, not horizontal negative start alone.
  - Status: not added to the passing corpus; keep as a fix target.

### Missing public parity scenarios now added

- `getmask2_with_start` negative horizontal start:
  - Added passing fixture: `imagingft.getmask2_with_start.dejavusans_negative_x_start`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `start=[-1.25, 0.0]`.
  - Purpose: validates left-side clipping/origin behavior against the live oracle without stored expected output.
- `draw_text` negative Y placement:
  - Added passing fixture: `imagingft.render_text.dejavusans20_negative_y_draw_text_rgba`, DejaVuSans.ttf, `size=20`, `text="Hello"`, `xy=[10, -4]`, RGBA canvas.
  - Purpose: validates Draw/text consumer clipping against the live oracle.
- Coverage effect:
  - These rows increased fixture parity coverage from `75` to `77` executed rows.
  - Coverage MCP metrics did not move (`imagingft.rs` remains `1228/1548` regions), so they are compatibility confidence rows, not coverage-closing rows.

### Reverse-mapped unclosed branches

| Source area | Lines | Public operation path | Current assessment |
|---|---:|---|---|
| TrueType load/request-size fallback and FT error mapping | 35-81 | font load before any operation | Only valid/missing/invalid-size rows are covered. Remaining FT error kinds need pathological font/size inputs or crafted font assets; do not fake these in Rust tests. |
| Bitmap-font arms | 156-226, 246, 608-639 | all public methods on `Font::Bitmap` | Current fixture loader creates TrueType fonts only. These branches are implementation-visible but not covered by the current Pillow `_imagingft` TrueType public corpus. Need a real Pillow-compatible bitmap-font surface before parity rows can be trusted. |
| Transpose helper source-map gaps | 127-129, 145 | `get_transposed_mask`, `transposed_bbox`, `validate_transposed_length` | Fixture rows cover all Pillow transpose constants plus `None`/missing orientation; remaining uncovered lines appear to be coverage/source mapping artifacts unless a new source-context query proves otherwise. |
| Layout/load glyph failure inside text shaping/rendering | 373-374, 539-547 | `getlength`, `getbbox`, `getmask*` | Needs a real oracle input that makes FreeType load fail for a glyph after font load succeeds. No current repo font/input does this. |
| `mask_from_run_with_start` clipping and sparse bitmap cases | 497-602 | `getmask`, `getmask2`, `getmask2_with_start`, `draw_text` | Negative vertical start is a confirmed mismatch. Other uncovered branches include zero-sized glyph bitmap, render fallback, canvas slice guard, empty bitmap coverage, and no-coverage pixels. Add only oracle-backed rows; do not synthesize self-comparison rows. |
| `bitmap_coverage` uncommon bitmap modes/pitch | 644-660 | `getmask*`, binary mask paths | Gray and mono coverage are partially exercised. Negative pitch and unsupported pixel mode are not reachable from current repo fonts through Pillow public APIs. Need a real oracle fixture asset before claiming coverage. |

### Next targeted probes

- Fix and then add the two negative vertical start rows above.
- Search for a repo font/text pair that makes FreeType return glyph-load failure after successful face load; if found, add it as an error/success row from oracle output only.
- Establish whether Pillow exposes a bitmap font object through the same public surface. If not, either remove `Font::Bitmap` from the imagingft public parity target or create a separate, clearly named bitmap-font parity target.

## Remaining explicit gaps

- Suite-level coverage is not complete by the 100% objective:
  - ImagingFT public-api suite executes all 77 rows and reports zero parity mismatches.
  - `pillow-rs/src/font/imagingft.rs` remains with uncovered lines/branch paths outside this minimal public corpus.
- Sweep finding:
  - Tried `getmask2_with_start` cases with DejaVuSans `text="Hello"` and negative vertical starts (`start=[0.0, -0.5]`, `start=[-1.25, -0.5]`) exposed real Pillow/Rust pixel mismatches and were not added to the passing corpus. This is a concrete next implementation gap in the start/clipping path.
- Error/parity:
  - No parity mismatches were observed in this run; error rows are all matched and classified correctly against oracle rows.
