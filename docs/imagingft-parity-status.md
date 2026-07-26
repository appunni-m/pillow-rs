# ImagingFT Public-API Parity Status (Current Worktree)

Last updated: 2026-07-26 (Asia/Kolkata) — latest local imagingft coverage run revalidated

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
  - Run id: `f3aaca70-52c3-4e36-9f7e-d9d0543f66b9` (first submission `submission_reused=false`)
  - Terminal status: `passed`, `1` passed, `0` failed
  - Diagnostics/ingest: `2539a252-b938-4ab2-ae8e-002114dba958` ingested with `target/coverage/imagingft/imagingft-rust.json`
  - Search log checks (`FAILED`, `error:`, `panic`) returned no failure context; only the normal `0 failed` summary line matched.
- Local coverage artifact: `target/coverage/imagingft/imagingft-rust.json`

## Corpus state

- Input files: `17` (`pillow-rs/tests/fixtures/imagingft/inputs/public-api/*.json`)
- Total rows: `75`
- Executed rows: `75/75`
- Required operation coverage check against case-set operations: no required manifest operations missing

## Required operation presence (fixture-defined)

| Operation | OK | Error | Total |
|---|---:|---:|---:|
| `draw_text` | 4 | 0 | 4 |
| `get_transposed_mask` | 9 | 1 | 10 |
| `getbbox` | 5 | 2 | 7 |
| `getbbox_binary` | 5 | 0 | 5 |
| `getlength` | 4 | 0 | 4 |
| `getmask` | 7 | 0 | 7 |
| `getmask2` | 6 | 0 | 6 |
| `getmask2_with_start` | 7 | 0 | 7 |
| `getmetrics` | 1 | 0 | 1 |
| `getname` | 1 | 5 | 6 |
| `has_variations` | 1 | 0 | 1 |
| `render_text_binary` | 4 | 0 | 4 |
| `transposed_bbox` | 7 | 0 | 7 |
| `unsupported_magic` | 0 | 1 | 1 |
| `validate_transposed_length` | 4 | 1 | 5 |

- Total success rows: `64`
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
- `covered_regions: 1228/1548` (`region_rate 0.7935897436`)
- Gaps remain in non-error branches and layout branches not yet covered by this public-input subset.

### Coverage delta

- Baseline: `19162f0c-7d00-47d9-9a69-a7f59e1d8678`
- Current: `2539a252-b938-4ab2-ae8e-002114dba958`
- Sweep movement against previous committed comparator snapshot `27d14363-1512-48c6-8a77-6849c6b14113`: suite covered metrics moved `+54` lines, `+4` branches, `+4` functions, `+91` regions. `pillow-rs/src/font/imagingft.rs` itself remained unchanged.

## Remaining explicit gaps

- Suite-level coverage is not complete by the 100% objective:
  - ImagingFT public-api suite executes all 75 rows and reports zero parity mismatches.
  - `pillow-rs/src/font/imagingft.rs` remains with uncovered lines/branch paths outside this minimal public corpus.
- Sweep finding:
  - A tried `getmask2_with_start` case with DejaVuSans `text="Hello"` and negative fractional `start=[-1.25, -0.5]` exposed a real Pillow/Rust pixel mismatch and was not added to the passing corpus. This is a concrete next implementation gap in the start/clipping path.
- Error/parity:
  - No parity mismatches were observed in this run; error rows are all matched and classified correctly against oracle rows.
