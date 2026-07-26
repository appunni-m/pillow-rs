# ImagingFT Public-API Parity Status

Last updated: 2026-07-26 (Asia/Kolkata)

## Scope
- Public surface: `pillow-rs/tests/fixtures/imagingft/inputs/public-api` (non-deprecated corpus only)
- Runner: `pillow-rs/tests/imagingft_public_api.rs`
- Oracle source: live Pillow `_imagingft.c` behavior through `pillow-rs/scripts/imagingft_oracle.py`

## Required evidence
- `make -C pillow-rs imagingft-tests`
  - Result: pass (1 test, 0 failures).
- Coverage MCP flow
  - `project_context` checked and used to discover approved coverage commands.
  - Approved command run: `imagingft-tests-coverage` was superseded by approved fixed command due artifact-path ingest issue.
  - `run_test`: `89a6e6ef-50a6-499c-9aa8-a47991d2ef80` (`imagingft-tests-coverage-fixed`)
  - `get_run_data`: terminal, `coverage_ingest.status = ingested`, `snapshot_ids = ["47d58757-9d73-4fdb-a712-22465d467009"]`
- `coverage_query` collected for snapshot `47d58757-9d73-4fdb-a712-22465d467009` with views: `summary`, `files`, `file` (for `pillow-rs/src/font/imagingft.rs`), `insights`.

## Commit under test
- `e0fec67f9f7aed5ba29911d1e9f5d8cf288fd93c`

## Corpus state
- Input files: 17 under `pillow-rs/tests/fixtures/imagingft/inputs/public-api`
- Total rows: 56
- Row status (Oracle truth, after prefix-stripping): 49 success, 7 error
- Prefix-remapped operation coverage in fixtures:
  - 15 unique logical operations after `imagingft.` strip.

## Per-operation success/error matrix (current non-deprecated fixture corpus)
| Operation | OK | Error | Total | Status |
|---|---:|---:|---:|---|
| `draw_text` | 4 | 0 | 4 | pass |
| `get_transposed_mask` | 4 | 1 | 5 | pass |
| `getbbox` | 4 | 0 | 4 | pass |
| `getbbox_binary` | 4 | 0 | 4 | pass |
| `getlength` | 4 | 0 | 4 | pass |
| `getmask` | 6 | 0 | 6 | pass |
| `getmask2` | 5 | 0 | 5 | pass |
| `getmask2_with_start` | 6 | 0 | 6 | pass |
| `getmetrics` | 1 | 0 | 1 | pass |
| `getname` | 1 | 4 | 5 | pass |
| `has_variations` | 1 | 0 | 1 | pass |
| `render_text_binary` | 4 | 0 | 4 | pass |
| `transposed_bbox` | 3 | 0 | 3 | pass |
| `unsupported_magic` | 0 | 1 | 1 | pass (expected error path) |
| `validate_transposed_length` | 2 | 1 | 3 | pass |

Error kinds observed:
- `TypeError`: 1 (invalid transpose string)
- `ValueError`: 5 (size/load/length undefined cases)
- `OSError`: 1 (bad font path)
- `NotImplementedError`: 1 (unsupported public operation)

## Coverage evidence snapshot
### Suite-level (snapshot `47d58757-9d73-4fdb-a712-22465d467009`)
- `total_lines: 17924`, `covered_lines: 1717` (`line_rate 0.09579`)
- `total_branches: 3150`, `covered_branches: 141` (`branch_rate 0.04476`)
- `total_functions: 1205`, `covered_functions: 141` (`function_rate 0.11701`)
- `total_regions: 31362`, `covered_regions: 2692` (`region_rate 0.08584`)

### File-level (`pillow-rs/src/font/imagingft.rs`, snapshot `47d58757-9d73-4fdb-a712-22465d467009`)
- `covered_lines: 669/815` (`line_rate 0.82086`)
- `covered_functions: 70/80` (`function_rate 0.875`)
- `covered_branches: 96/150` (`branch_rate 0.64`)
- `covered_regions: 1198/1476` (`region_rate 0.81165`)
- Uncovered lines remain: `146`
- Uncovered branches remain: `54`

### Metrics delta vs prior imagingft snapshot (`5075479d-20fc-4f57-aaa3-92e5a8ff42ac`)
- `covered_lines: 1715 -> 1717`
- `covered_branches: 139 -> 141`
- `covered_regions: 2689 -> 2692`
- Overall suite coverage improved slightly, no regressions introduced.

### Coverage gaps / remaining work
- Current target is not at 100% imagingft parity/coverage: remaining imagingft implementation branches and lines are still outstanding in `pillow-rs/src/font/imagingft.rs`.
- No 100% parity/coverage claim is valid yet.
