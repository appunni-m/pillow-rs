# ImagingFT Public-API Parity Status

Last updated: 2026-07-25 (Asia/Kolkata)

## Scope
- Public surface: `pillow-rs/tests/fixtures/imagingft/inputs/public-api`
- Runner: `pillow-rs/tests/imagingft_public_api.rs`
- Target parity source: Pillow `_imagingft.c` behavior captured in non-deprecated fixtures

## Evidence captured
- Test execution: `make -C pillow-rs imagingft-tests` (pass: 1 test, 0 failures)
- Coverage command run:
  - `imagingft-tests-coverage-fixed` run id `69d78ee4-3839-4fe7-b579-1444d494ca10` (terminal pass, ingested)
- Coverage snapshot: `2518f088-d630-431c-bab6-5e26342e24b0`

### Coverage delta (baseline-internal)
- Reference: `57e99c8a-92d2-41a9-8015-a76e00d79423`
- Current: `2518f088-d630-431c-bab6-5e26342e24b0`
- Net delta: `+0` covered lines, `+0` covered branches, `+0` covered functions

## Test corpus coverage
- Fixture files used: 17 files under `inputs/public-api`
- Total cases: 48
- Success rows: 41
- Error rows: 7
- Required surfaces asserted present: getname, getmetrics, getlength, has_variations, getbbox, getbbox_binary, getmask, getmask2, getmask2_with_start, get_transposed_mask, transposed_bbox, validate_transposed_length, draw_text
- Status coverage across all rows: all passed in runner (no false positives/false negatives)

### Per-operation status matrix
Operation | Total | OK | Error | Result
---|---:|---:|---:|---
`getbbox` | 3 | 3 | 0 | pass
`getbbox_binary` | 3 | 3 | 0 | pass
`getlength` | 3 | 3 | 0 | pass
`getmask` | 5 | 5 | 0 | pass
`getmask2` | 5 | 5 | 0 | pass
`getmask2_with_start` | 5 | 5 | 0 | pass
`getmetrics` | 1 | 1 | 0 | pass
`getname` | 5 | 1 | 4 | pass (4 load-failure variants)
`has_variations` | 1 | 1 | 0 | pass
`transposed_bbox` | 3 | 3 | 0 | pass
`get_transposed_mask` | 4 | 3 | 1 | pass (1 layout-orientation error variant)
`validate_transposed_length` | 3 | 2 | 1 | pass
`draw_text` | 3 | 3 | 0 | pass
`render_text_binary` | 3 | 3 | 0 | pass
`unsupported_magic` | 1 | 0 | 1 | pass

### Error matrix
Operation | Category | Pattern match
---|---|---
`imagingft.getname` | `ValueError` | load failures
`imagingft.unsupported_magic` | `NotImplementedError` | unsupported op
`imagingft.layout_failure` | `ValueError` | invalid layout/orientation path
`validate_transposed_length` | `ValueError` | orientation validation failure

## Result-contract behavior
- Every row is compared via fixture-driven `expectation` paths.
- Success path compares shape/size/mode/pixels (and raw/hex/hash when available).
- Error path enforces fixture-expected `status`/`expected.error` and matches type/category/message (or message-pattern when requested).
- No test-side hardcoded success/error verdicts except required-surface assertion.

## Coverage MCP evidence
- Suite: `imagingft`
- Current snapshot (`2518f088-d630-431c-bab6-5e26342e24b0`):
  - `total_lines: 17924`, `covered_lines: 1715` (`line_rate 0.09568`)
  - `total_branches: 3150`, `covered_branches: 139` (`branch_rate 0.04413`)
  - `total_functions: 1205`, `covered_functions: 141` (`function_rate 0.11701`)
  - `total_regions: 31362`, `covered_regions: 2689` (`region_rate 0.08574`)
- `coverage_query(view="file", file=pillow-rs/src/font/imagingft.rs)`:
  - `covered_lines 668/815`
  - `covered_functions 70/80`
  - `covered_branches 95/150`
  - uncovered lines: 57
  - partial-branch lines: 34
  - total relevant line gaps: 82

## Remaining explicit gaps
- All fixture rows in the non-deprecated public corpus are passing for parity checks.
- Imagingft implementation surface coverage is not 100% yet for `pillow-rs/src/font/imagingft.rs`:
  - `uncovered lines 57`
  - `partial-branch lines 34`
  - exact uncovered line ranges remain those reported in `coverage_query(view="file", snapshot=2518f088-d630-431c-bab6-5e26342e24b0)`.
