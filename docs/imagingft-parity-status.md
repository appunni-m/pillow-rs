# ImagingFT Public-API Parity Status (Current Worktree)

Last updated: 2026-07-26 (Asia/Kolkata)

## Scope

- Public surface source: `pillow-rs/tests/fixtures/imagingft/inputs/public-api` (non-deprecated corpus only)
- Target suite: `make -C pillow-rs imagingft-tests`
- Oracle: repo-local Pillow C path via `pillow-rs/scripts/imagingft_oracle.py` and `.oracle-venv`
- No deprecated `deprecated/imagingft/*` tests are used.
- Current fixture/test implementation: `pillow-rs/tests/imagingft_public_api.rs` + `pillow-rs/tests/support/imagingft_runner.rs` using explicit `Result` paths.
- Oracle source-of-truth proof:
  - `.oracle-venv` is ignored by git at root via `.oracle-venv/`.
  - The oracle process validates it is running from `<repo>/.oracle-venv/bin/python` and imports `PIL` from that env only.
  - The repo-local Pillow shows `ImageFont.core is PIL._imagingft`; `ImageFont.truetype` builds `FreeTypeFont` via `core.getfont`, and `load_font()` requires `builtins.Font` on `font.font`, confirming C-layer-backed objects.

## Acceptance checks

- `make -C pillow-rs imagingft-tests`  
  Result: `1` passed, `0` failed
- Coverage evidence:
  - Local coverage command: `make -C pillow-rs imagingft-tests-coverage`
  - Test result from command output: `1` passed, `0` failed
  - MCP-managed run: `9d7bdbda-5c15-4eda-9a6b-50889a9a945b`
  - Coverage MCP run command: `imagingft-tests-coverage-fixed`
  - Coverage artifact: `target/coverage/imagingft/imagingft-rust.json`
  - Snapshot id: `8c36e664-1acd-4dba-a52b-f31098913e95`
  - Prior suite snapshot for comparison: `5817fe8b-7e59-4315-82b3-fb3829feb7ec`

## Corpus state

- Input files: `17` (`pillow-rs/tests/fixtures/imagingft/inputs/public-api/*.json`)
- Total rows: `58`
- Executed rows: `58/58`
- Required operation coverage check against case-set operations: no required-manifest operations missing
- No fixture files were added/removed in this cycle.

## Required operation presence (fixture-defined)

| Operation | OK | Error | Total |
|---|---:|---:|---:|
| `draw_text` | 4 | 0 | 4 |
| `get_transposed_mask` | 4 | 1 | 5 |
| `getbbox` | 4 | 2 | 6 |
| `getbbox_binary` | 4 | 0 | 4 |
| `getlength` | 4 | 0 | 4 |
| `getmask` | 6 | 0 | 6 |
| `getmask2` | 5 | 0 | 5 |
| `getmask2_with_start` | 6 | 0 | 6 |
| `getmetrics` | 1 | 0 | 1 |
| `getname` | 1 | 4 | 5 |
| `has_variations` | 1 | 0 | 1 |
| `render_text_binary` | 4 | 0 | 4 |
| `transposed_bbox` | 3 | 0 | 3 |
| `unsupported_magic` | 0 | 1 | 1 |
| `validate_transposed_length` | 2 | 1 | 3 |

- Total success rows: `49`
- Total error rows: `9`
- `expect_error` rows always resolve to `error` on Rust and match oracle status/category/message.

## Error-category matrix (oracle-defined)

- `TypeError: an integer is required (got type str)` — `1`
- `ValueError: font size must be greater than 0, not 0` — `1`
- `ValueError: font size must be greater than 0, not -1` — `1`
- `ValueError: font size must be greater than 0, not -5.5` — `1`
- `ValueError: text length is undefined for text rotated by 90 or 270 degrees` — `1`
- `OSError: cannot open resource` — `1`
- `OSError: invalid argument` — `1`
- `OSError: invalid pixel size` — `1`
- `NotImplementedError: unsupported imagingft operation: unsupported_magic` — `1`

## Coverage evidence

### Suite summary (`imagingft`)

- Current artifact metrics (`target/coverage/imagingft/imagingft-rust.json`, latest run):
  - `total_lines: 17962`, `covered_lines: 1736` (`line_rate 0.09664848012470771`)
  - `total_branches: 3166`, `covered_branches: 147` (`branch_rate 0.04643082754264056`)
  - `total_functions: 1208`, `covered_functions: 142` (`function_rate 0.11754966887417219`)
  - `total_regions: 31434`, `covered_regions: 2717` (`region_rate 0.08643507030603804`)

### `pillow-rs/src/font/imagingft.rs`

- `covered_lines: 688/853` (`line_rate 0.8065650644783119`)
- `covered_functions: 71/83` (`function_rate 0.8554216867469879`)
- `covered_branches: 102/166` (`branch_rate 0.6144578313253012`)
- `covered_regions: 1223/1548` (`region_rate 0.7900516795865633`)
- Gaps: `uncovered_line_count: 80`, `partial_branch_line_count: 41`, `uncovered_function_line_count: 0`
- `coverage_query` equivalent file-level coverage confirms:
  - weak module ranking includes `pillow-rs/src/font/imagingft.rs` among the least-covered files in this target run
  - many uncovered paths are in non-error branches inside font layout/render internals and unexercised bitmap-font control flow.

### Coverage delta

- Baseline checked: `5817fe8b-7e59-4315-82b3-fb3829feb7ec`
- Net movement vs baseline: no measured suite-level or imagingft-file-level metric improvement (`all metrics unchanged`).

## Remaining explicit gaps

- Suite is not coverage-complete by objective definition:
- `coverage`:
  - ImagingFT public-api suite executes all 58 rows but does not close all lines/branches in `pillow-rs/src/font/imagingft.rs` yet.
  - Imagingft core file still has uncovered lines/branches, so end-state is **not 100% parity-coverage complete**.
- Error/parity:
  - No parity mismatches were observed in this run; error rows are all matched and classified correctly against oracle rows.
