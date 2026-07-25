# ImagingFT Public-API Parity Status

Last updated: 2026-07-25 (Asia/Kolkata)

## Scope
- Public surface: `pillow-rs/tests/fixtures/imagingft/inputs/public-api`
- Runner: `pillow-rs/tests/imagingft_public_api.rs`
- Target parity source: Pillow `_imagingft.c` behavior captured in non-deprecated fixtures

## Evidence captured
- Test execution: `make -C pillow-rs imagingft-tests` (pass: 1 test, 0 failures)
- Coverage command (Coverage MCP approved): `imagingft-tests-coverage-fixed`
  - run id: `021ab85c-a5ba-44f7-9959-320f50bee0e2`
  - snapshot id: `5075479d-20fc-4f57-aaa3-92e5a8ff42ac`
- Commit under test: `021e2b902bbd0bb04b57e16cbc81331f78a8750a`
- Branch: `main`

### Coverage metrics snapshot
- `total_lines: 17924`, `covered_lines: 1715` (`line_rate 0.09568`)
- `total_branches: 3150`, `covered_branches: 139` (`branch_rate 0.04413`)
- `total_functions: 1205`, `covered_functions: 141` (`function_rate 0.11701`)
- `total_regions: 31362`, `covered_regions: 2689` (`region_rate 0.08574`)
- Delta vs previous imagingft snapshot (`c3c528ee-096d-4884-a154-a225f1d6dc8e`, commit `25f0f07227380c7c5f2bb27047ee26e74067a783`): no metric delta observed.

## ImagingFT implementation surface
- `coverage_query(view="file", snapshot_id="5075479d-20fc-4f57-aaa3-92e5a8ff42ac", file_path="pillow-rs/src/font/imagingft.rs")`
  - `covered_lines: 668/815` (`line_rate 0.8196`)
  - `covered_functions: 70/80` (`function_rate 0.875`)
  - `covered_branches: 95/150` (`branch_rate 0.6333`)
  - `covered_regions: 1196/1476` (`region_rate 0.8103`)
  - `uncovered_line_count: 57`
  - `partial_branch_line_count: 34`
  - `uncovered_line_count` gap ranges: 48 ranges, remaining at lines listed in coverage artifact for snapshot above (non-zero and unchanged from prior run).

## Test corpus state
- Fixture files: 17 under `inputs/public-api`
- Rows: 48
- Success rows: 41
- Error rows: 7
- Required public operations from runner set are present:
  - getname, getmetrics, getlength, has_variations, getbbox, getbbox_binary, getmask, getmask2, getmask2_with_start, get_transposed_mask, transposed_bbox, validate_transposed_length, draw_text
- Coverage-only/auxiliary operations present:
  - render_text_binary, unsupported_magic

### Per-operation status matrix
Operation | Total | OK | Error | Result
---|---:|---:|---:|---
`draw_text` | 3 | 3 | 0 | pass
`get_transposed_mask` | 4 | 3 | 1 | pass
`getbbox` | 3 | 3 | 0 | pass
`getbbox_binary` | 3 | 3 | 0 | pass
`getlength` | 3 | 3 | 0 | pass
`getmask` | 5 | 5 | 0 | pass
`getmask2` | 5 | 5 | 0 | pass
`getmask2_with_start` | 5 | 5 | 0 | pass
`getmetrics` | 1 | 1 | 0 | pass
`getname` | 5 | 1 | 4 | pass
`has_variations` | 1 | 1 | 0 | pass
`render_text_binary` | 3 | 3 | 0 | pass
`transposed_bbox` | 3 | 3 | 0 | pass
`unsupported_magic` | 1 | 0 | 1 | pass (error-path exercised)
`validate_transposed_length` | 3 | 2 | 1 | pass

## Error matching contract
- Rows with `expect_error: true` are executed through fixture-driven `Result` failure handling in `pillow-rs/tests/imagingft_public_api.rs`.
- Error matching is driven from fixture `expectation.expected.error` keys and `compare.paths`; tests do not hardcode Rust error values.
- `status` is asserted as `error`, and error category/message are compared per fixture contract.

### Coverage MCP trace
- Snapshot metadata is read from `coverage_query(view="summary", snapshot_id="5075479d-20fc-4f57-aaa3-92e5a8ff42ac")`.
- Suite-level and file-level evidence were collected from
  - `coverage_query(view="summary", snapshot_id="5075479d-20fc-4f57-aaa3-92e5a8ff42ac")`
  - `coverage_query(view="file", snapshot_id="5075479d-20fc-4f57-aaa3-92e5a8ff42ac", file_path="pillow-rs/src/font/imagingft.rs")`
  - `coverage_query(view="insights", snapshot_id="5075479d-20fc-4f57-aaa3-92e5a8ff42ac")`

## Explicit remaining gaps
- Fixture-level parity for all non-deprecated public-api corpus rows is currently **pass** (0 parity failures).
- Coverage for `pillow-rs/src/font/imagingft.rs` still has remaining gaps:
  - `uncovered_line_count: 57`
  - `partial_branch_line_count: 34`
- No 100% coverage claim is valid yet at suite level or for ImagingFT implementation-targeted branches/functions.
