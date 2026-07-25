# ImagingFT Public-API Parity Status

Last updated: 2026-07-25 (Asia/Kolkata)

## Scope
- Public surface: `pillow-rs/tests/fixtures/imagingft/inputs/public-api`
- Runner: `pillow-rs/tests/imagingft_public_api.rs`
- Target parity source: Pillow `_imagingft.c` behavior captured in fixtures (non-deprecated inputs only)

## Evidence captured
- Test execution: `make -C pillow-rs imagingft-tests` (pass: 1 test, 0 failures)
- Coverage command attempts:
  - `imagingft-tests-coverage` run id `5a4c51fb-b08f-46d0-a41b-082014cbc860` (artifact ingest failed because command registered with absolute artifact path `/target/...`)
  - `imagingft-tests-coverage-fixed` run id `64f4f94b-81c0-49ad-af88-b0e93598d936` (terminal pass)
  - `imagingft-tests-coverage-fixed` run id `50ea6e0d-976a-4aa5-b3c7-c74640d9e1df` (terminal pass, current refresh)
- Coverage snapshots:
  - Previous: `23943940-275a-454d-b3a6-65c63320e42b`
  - Previous in-context latest: `0950457b-f6fb-4df4-b06f-328813bdd6ac`
  - Current: `57e99c8a-92d2-41a9-8015-a76e00d79423`

### Coverage delta (latest - previous)
  - `coverage_query(view="summary", snapshot=0950457b-f6fb-4df4-b06f-328813bdd6ac)` -> `total_lines 17531 / covered 1402`
  - `coverage_query(view="summary", snapshot=57e99c8a-92d2-41a9-8015-a76e00d79423)` -> `total_lines 17924 / covered 1715`
  - Net aggregate move (local): `+393` covered lines, `+48` covered branches, `+33` covered functions.

## Test corpus coverage
- Fixture files used: 17 files under `inputs/public-api`
- Total cases: 48
- Success rows: 41
- Error rows: 7
- Status coverage across all rows: all passed in runner

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
`getname` | 5 | 1 | 4 | pass (includes 4 load-failure variants)
`has_variations` | 1 | 1 | 0 | pass
`transposed_bbox` | 3 | 3 | 0 | pass
`get_transposed_mask` | 4 | 3 | 1 | pass (includes invalid orientation row)
`validate_transposed_length` | 3 | 2 | 1 | pass
`draw_text` | 3 | 3 | 0 | pass
`render_text_binary` | 3 | 3 | 0 | pass
`unsupported_magic` | 1 | 0 | 1 | pass (expected-unsupported path)

### Error classes observed
- `imagingft.getname` load failure row: `ValueError`
- `imagingft.unsupported_magic` row: `NotImplementedError`
- `imagingft.layout_failure` row: `ValueError`
- `validate_transposed_length` 90/270 row: `ValueError`

## Result-contract behavior
- All rows are evaluated through the fixture-driven `expectation` path list.
- Runtime no longer hard-codes error class/message in test logic; comparisons are derived from fixture fields.
- Error rows never pass as success; success rows never accepted as error.

## Coverage MCP evidence
- Suite: `imagingft`
Snapshot metrics:
  - `total_lines: 17924`, `covered_lines: 1715` (`line_rate 0.09568`)
  - `total_branches: 3150`, `covered_branches: 139` (`branch_rate 0.04413`)
  - `total_functions: 1205`, `covered_functions: 141` (`function_rate 0.11701`)
  - `total_regions: 31362`, `covered_regions: 2689` (`region_rate 0.08574`)
- `pillow-rs/src/font/imagingft.rs` (file-level):
  - `coverage_query(view="file", snapshot=57e99c8a-92d2-41a9-8015-a76e00d79423, file=pillow-rs/src/font/imagingft.rs)`
    - `covered_lines 668/815`
    - `covered_functions 70/80`
    - `covered_branches 95/150`
    - uncovered lines: 57
    - partial-branch lines: 34
    - total relevant line gaps: 82
    - gap ranges: 35, 36, 46, 127, 137-140, 145, 152, 156, 163-165, 174, 176, 184, 193, 220, 226, 231, 373, 374, 384, 444, 455, 471-475, 497, 502, 503, 508, 509, 510-514, 520, 521, 524-526, 531, 535-537, 544-547, 550-554, 555-558, 559-563, 564, 565, 566-569, 570-572, 575-578, 585, 587, 588, 603

## Remaining explicit gaps
- Imagingft public corpus is parity-green by row for the current fixture set, but target implementation coverage is not 100% on `pillow-rs/src/font/imagingft.rs` (`uncovered lines 57`, `partial-branch lines 34`).
- Remaining explicit gap: all listed lines/ranges in the current `coverage_query(view=\"file\")` output above are still uncovered/partially covered.
