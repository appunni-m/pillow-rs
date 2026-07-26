# ImagingFT Public-API Parity Status (Current Worktree)

Last updated: 2026-07-26 (Asia/Kolkata)

## Scope

- Public surface source: `pillow-rs/tests/fixtures/imagingft/inputs/public-api` (non-deprecated corpus only)
- Target suite: `make -C pillow-rs imagingft-tests`
- Oracle: repo-local Pillow C path via `pillow-rs/scripts/imagingft_oracle.py` and `.oracle-venv`
- No deprecated `deprecated/imagingft/*` tests are used.

## Acceptance checks

- `make -C pillow-rs imagingft-tests`  
  Result: `1` passed, `0` failed
- Coverage MCP sequence:
  - `project_context` consulted
  - `run_test` executed on `imagingft-tests-coverage-fixed` (`258e7dec-226f-4b00-9336-04df6e8c67f2`)
  - `run_test` result: terminal `passed`, `counters:{passed:1, failed:0}`
  - `coverage_ingest.status=ingested`
  - `snapshot_ids=["5817fe8b-7e59-4315-82b3-fb3829feb7ec"]`

## Corpus state

- Input files: `17` (`pillow-rs/tests/fixtures/imagingft/inputs/public-api/*.json`)
- Total rows: `58`
- Executed rows: `58/58`
- Required operation coverage check against case-set operations: no required-manifest operations missing

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

- Current snapshot: `5817fe8b-7e59-4315-82b3-fb3829feb7ec`
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

### Coverage delta

- Baseline checked: `9353fd3a-561e-4039-9eff-cf503dfe3396` (same branch, same commit)
- Net movement vs baseline: no metric movement observed in suite totals or `pillow-rs/src/font/imagingft.rs` for this run.

## Remaining explicit gaps

- Suite is not coverage-complete by objective definition:
  - imagingft-targeted test only executes the imagingft public-api row set, so unrelated modules remain largely unexecuted.
  - Imagingft core file still has uncovered lines/branches, so end-state is **not 100% parity-coverage complete**.
