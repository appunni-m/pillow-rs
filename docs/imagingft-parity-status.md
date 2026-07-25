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
  - `imagingft-tests-coverage-fixed` run id `393d8e0d-3c07-4334-97b7-fbf113398f1b` (passed, terminal)
- Coverage snapshot ingested: `23943940-275a-454d-b3a6-65c63320e42b`
- Previous comparison snapshot in immediate history: `9160bb50-6ce2-4e66-85c5-f056bc452798`

## Test corpus coverage
- Fixture files used: 16 files under `inputs/public-api`
- Total cases: 45
- Success rows: 41
- Error rows: 4
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
`getname` | 2 | 1 | 1 | pass (includes load-failure row)
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
- Snapshot metrics:
  - `total_lines: 17531`, `covered_lines: 1402` (`line_rate 0.07997`)
  - `total_branches: 3074`, `covered_branches: 91` (`branch_rate 0.02960`)
  - `total_functions: 1166`, `covered_functions: 108` (`function_rate 0.09262`)
  - `total_regions: 30644`, `covered_regions: 2118` (`region_rate 0.06912`)
- `pillow-rs/src/font/imagingft.rs` (file-level):
  - `covered_lines 355/422`
  - `covered_functions 37/41`
  - `covered_branches 47/74`
  - uncovered lines present at ranges: 35,36-36,59,127-129,138,145,152,163-165,173,193,215,220,221,228,373,374,444-445,461-462,471-472,474-475,497-498,502,503,509-510,514-515,520-521,524,526-527,534,540,544-545,555-558,560-565,567,570-571,576,577,585,592, etc. (`coverage_query file` shows this explicitly)

## Remaining explicit gaps
- Imagingft public corpus is parity-green by row for the current fixture set, but imagingft implementation is not fully covered by this suite (`pillow-rs/src/font/imagingft.rs` still has uncovered lines/branches listed above).
- Coverage parity requirement in this environment is therefore limited to fixture-parity outcomes, not full function-branch exhaustiveness.
