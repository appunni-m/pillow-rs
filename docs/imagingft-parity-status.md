# ImagingFT Public-API Parity Status

Last updated: 2026-07-26 (Asia/Kolkata)

## Scope
- Public surface: `pillow-rs/tests/fixtures/imagingft/inputs/public-api` (non-deprecated corpus only)
- Active suite: `make -C pillow-rs imagingft-tests`
- Oracle source: repo-local Pillow `_imagingft` via `pillow-rs/scripts/imagingft_oracle.py` using `.oracle-venv` enforcement.

## Acceptance checks
- `make -C pillow-rs imagingft-tests`
  - Result: `1` passed, `0` failed
- Coverage MCP flow (required sequence)
  - `project_context` consulted and latest approved command discovered: `imagingft-tests-coverage-fixed` (`258e7dec-226f-4b00-9336-04df6e8c67f2`)
  - `run_test` submitted: `03b80396-ecad-4a5c-9e2e-5ed3e083747f`
  - `get_run_data` terminal status: `passed`, `counters:{passed:1,failed:0}`, `coverage_ingest.status=ingested`
  - Snapshot: `18d74bbe-2ba5-4556-886d-b86b19f3d40d`

## Fixture corpus state
- Input files: `17` under `pillow-rs/tests/fixtures/imagingft/inputs/public-api`
- Total rows: `56`
- Oracle+Rust comparison: `56/56` rows executed and parity-checked

## Per-operation parity matrix (rows are fixture-defined expectations)
| Operation | Status | OK | Error |
|---|---|---:|---:|
| `draw_text` | pass | 4 | 0 |
| `get_transposed_mask` | pass | 4 | 1 |
| `getbbox` | pass | 4 | 0 |
| `getbbox_binary` | pass | 4 | 0 |
| `getlength` | pass | 4 | 0 |
| `getmask` | pass | 6 | 0 |
| `getmask2` | pass | 5 | 0 |
| `getmask2_with_start` | pass | 6 | 0 |
| `getmetrics` | pass | 1 | 0 |
| `getname` | pass | 1 | 4 |
| `has_variations` | pass | 1 | 0 |
| `render_text_binary` | pass | 4 | 0 |
| `transposed_bbox` | pass | 3 | 0 |
| `unsupported_magic` | pass | 0 | 1 |
| `validate_transposed_length` | pass | 2 | 1 |

Global totals: `49` success rows, `7` error rows. No parity failures in fixture corpus at this revision.

### Error-category matrix (from fixture expectations validated against live `_imagingft` oracle)
- `TypeError`: 1
  - `an integer is required (got type str)`
- `ValueError`: 5
  - `font size must be greater than 0, not -1`
  - `font size must be greater than 0, not -5.5`
  - `font size must be greater than 0, not 0`
  - `text length is undefined for text rotated by 90 or 270 degrees`
- `OSError`: 1
  - `cannot open resource`
- `NotImplementedError`: 1
  - `unsupported imagingft operation: unsupported_magic`

## Coverage evidence
### Suite-level (`imagingft`)
- Snapshot: `18d74bbe-2ba5-4556-886d-b86b19f3d40d` (commit `1516723f2023e531d9751a739d5125c81ff9c20b`)
- `total_lines: 17924`, `covered_lines: 1717` (`line_rate 0.09579`)
- `total_branches: 3150`, `covered_branches: 141` (`branch_rate 0.04476`)
- `total_functions: 1205`, `covered_functions: 141` (`function_rate 0.11701`)
- `total_regions: 31362`, `covered_regions: 2692` (`region_rate 0.08584`)

### File-level (`pillow-rs/src/font/imagingft.rs`)
- `covered_lines: 669/815` (`line_rate 0.82086`)
- `covered_functions: 70/80` (`function_rate 0.875`)
- `covered_branches: 96/150` (`branch_rate 0.64`)
- `covered_regions: 1198/1476` (`region_rate 0.81165`)
- Open gaps remain: `uncovered_line_count: 57`, `partial_branch_line_count: 34`

### Coverage delta
- Baseline compared: `b343b0f3-7e31-4ea1-a3e6-a1b066b26f56` (previous commit snapshot on `main`)
- Delta: no metric movement for the suite or for `pillow-rs/src/font/imagingft.rs` in this re-run (same counters and same gap list)

## Remaining gaps
- No parity gaps in fixture rows.
- Coverage is not complete for truth-surface confidence under Coverage MCP standards because imagingft-specific lanes still have unhit lines/branches in `pillow-rs/src/font/imagingft.rs`.
- 100% region/branch/line coverage target is still unmet: `pillow-rs/src/font/imagingft.rs` still has `uncovered_line_count: 57` and `partial_branch_line_count: 34` at snapshot `18d74bbe-2ba5-4556-886d-b86b19f3d40d`.
