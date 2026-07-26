# ImagingFT Public-API Parity Status

Last updated: 2026-07-26 (Asia/Kolkata)

## Scope
- Public surface: `pillow-rs/tests/fixtures/imagingft/inputs/public-api` (non-deprecated corpus only)
- Runner: `pillow-rs/tests/imagingft_public_api.rs`
- Oracle source: live Pillow `_imagingft` behavior via `pillow-rs/scripts/imagingft_oracle.py`
- Oracle runtime policy: only `.oracle-venv/bin/python` in this repository is accepted (`IMAGINGFT_ORACLE_PYTHON` must point to `.../.oracle-venv/bin/python` when set).
- Python layer used for oracle calls is the Pillow Python API (`ImageFont`), which initializes and delegates to the C extension (`_imagingft`) in this environment (`core.getfont` is present and used by Font loading).

## Acceptance evidence
- `make -C pillow-rs imagingft-tests`:
  - Result: pass (`1` passed, `0` failed)

- Coverage MCP flow (required toolchain):
  - `project_context` consulted to discover approved commands.
  - Approved command used: `imagingft-tests-coverage-fixed`.
  - `run_test` submitted: `5a18ab67-e3bc-4b16-bfc7-ceb837cb4e37`
  - `get_run_data` terminal: `status=passed`, `coverage_ingest.status=ingested`, `snapshot_ids=["90b5621b-eab0-4da5-bedc-c10d12a0d876"]`

## Corpus state
- Input files: 17 under `pillow-rs/tests/fixtures/imagingft/inputs/public-api`
- Total rows: 56
- All rows executed with live oracle and Rust implementation comparison.

## Per-operation parity matrix
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
| `unsupported_magic` | pass (expected error) | 0 | 1 |
| `validate_transposed_length` | pass | 2 | 1 |

Global: 49 success rows, 7 error rows (no parity mismatches in the suite).

### Error kind matrix
- `TypeError`: 1 (`get_transposed_mask`)
  - Message: `an integer is required (got type str)`
- `ValueError`: 5 (`getname`/`validate_transposed_length`)
  - Messages: negative/zero font size errors and rotated-length undefined
- `OSError`: 1 (`getname`)
  - Message: `cannot open resource`
- `NotImplementedError`: 1 (`unsupported_magic`)
  - Message: `unsupported imagingft operation: unsupported_magic`

## Coverage evidence snapshot
### Suite-level (`90b5621b-eab0-4da5-bedc-c10d12a0d876`)
- Command artifact: `target/coverage/imagingft/imagingft-rust.json` (suite: `imagingft`, format: `llvm-json`)
- `total_lines: 17924`, `covered_lines: 1717` (`line_rate 0.09579`)
- `total_branches: 3150`, `covered_branches: 141` (`branch_rate 0.04476`)
- `total_functions: 1205`, `covered_functions: 141` (`function_rate 0.11701`)
- `total_regions: 31362`, `covered_regions: 2692` (`region_rate 0.08584`)

### File-level (`pillow-rs/src/font/imagingft.rs`)
- `covered_lines: 669/815` (`line_rate 0.82086`)
- `covered_functions: 70/80` (`function_rate 0.875`)
- `covered_branches: 96/150` (`branch_rate 0.64`)
- `covered_regions: 1198/1476` (`region_rate 0.81165`)
- `uncovered_line_count: 57`, `partial_branch_line_count: 34` (no uncovered functions in this file)

### Coverage deltas
- Baseline snapshot: `99b3515a-34ef-49f0-a82a-4e746150d813` (from previous commit `149f41a8...`)
- No suite-level metric delta from the previous imagingft snapshot.

## Remaining gaps
- Target parity is not complete from a coverage perspective:
  - ImagingFT branch/line/region coverage still has open lanes in `pillow-rs/src/font/imagingft.rs`.
  - Full 100% coverage (including zero pending lines in targeted surfaces) is still blocked by implementation coverage gaps, not row-level correctness.
