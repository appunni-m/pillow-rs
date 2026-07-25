# ImagingFT Public-API Parity Status

Last updated: 2026-07-25 (Asia/Kolkata)

## Scope
- Public surface: `pillow-rs/tests/fixtures/imagingft/inputs/public-api`
- Runner: `pillow-rs/tests/imagingft_public_api.rs`
- Target parity source: Pillow `_imagingft.c` behavior captured in non-deprecated fixtures

## Evidence captured
- Test execution: `make -C pillow-rs imagingft-tests` (pass: 1 test, 0 failures)
- Coverage command: `imagingft-tests-coverage-fixed`
  - run id: `a8258543-665e-4383-9bad-2edf47191613`
- Coverage snapshot: `1bd8dd73-92ae-4f6a-9928-70743022348b`

### Coverage metrics snapshot
- `total_lines: 17924`, `covered_lines: 1715`
- `total_branches: 3150`, `covered_branches: 139`
- `total_functions: 1205`, `covered_functions: 141`
- `total_regions: 31362`, `covered_regions: 2689`

### ImagingFT implementation surface
- `coverage_query(view="file", file="pillow-rs/src/font/imagingft.rs")`
  - `covered_lines: 668/815` (`line_rate 0.8196`)
  - `covered_functions: 70/80` (`function_rate 0.875`)
  - `covered_branches: 95/150` (`branch_rate 0.6333`)
  - `uncovered_line_count: 57`
  - `partial_branch_line_count: 34`
  - Remaining gap ranges are available in coverage artifact at the above snapshot.

## Test corpus state
- Fixture files: 17 under `inputs/public-api`
- Rows: 48
- Success rows: 41
- Error rows: 7
- Required public operations from runner set are present:
  - getname, getmetrics, getlength, has_variations, getbbox, getbbox_binary, getmask, getmask2, getmask2_with_start, get_transposed_mask, transposed_bbox, validate_transposed_length, draw_text

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
`unsupported_magic` | 1 | 0 | 1 | pass (runner-path coverage)
`validate_transposed_length` | 3 | 2 | 1 | pass

## Error matching contract
- Rows with `expect_error: true` are executed through fixture-driven `Result` failure handling.
- Error matching is driven from fixture `expectation.expected.error` keys and `compare.paths`; tests do not hardcode Rust error forms.
- `status` is asserted as `error`, and error category/message are compared per fixture contract.

## Explicit remaining gaps
- All non-deprecated fixture rows currently in corpus are passing.
- Coverage requirement for 100% ImagingFT parity remains incomplete because `pillow-rs/src/font/imagingft.rs` still has remaining gaps (`uncovered_line_count: 57`, `partial_branch_line_count: 34`) under the imagingft suite snapshot.
- Until those gaps are closed, we cannot claim full branch/line parity coverage.
