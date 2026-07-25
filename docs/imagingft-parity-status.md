# ImagingFT parity status (public API corpus)

Date: 2026-07-25

## 1) Corpus / operation coverage

- Fixture corpus used: `pillow-rs/tests/fixtures/imagingft/inputs/public-api`
- Active fixture rows: all non-empty rows across the non-deprecated public-api manifests
  (`35` total cases).
- Required public surface asserted in test:
  `getname`, `getmetrics`, `getlength`, `has_variations`, `getbbox`, `getbbox_binary`,
  `getmask`, `getmask2`, `getmask2_with_start`, `get_transposed_mask`,
  `transposed_bbox`, `validate_transposed_length`, `draw_text`.

## 2) Current parity matrix

| Surface | Manifest | Rust status | Oracle status | Error handling |
|---|---|---|---|---|
| getname | covered | pass | pass | not applicable |
| getmetrics | covered | pass | pass | not applicable |
| getlength | covered | pass | pass | not applicable |
| has_variations | covered | pass | pass | not applicable |
| getbbox | covered | pass | pass | not applicable |
| getbbox_binary | manifest-only | not run (no cases) | not run (no cases) | no rows |
| getmask | covered | pass | pass | not applicable |
| getmask2 | covered | pass | pass | not applicable |
| getmask2_with_start | covered | pass | pass | not applicable |
| get_transposed_mask | covered | pass | pass | not applicable |
| transposed_bbox | covered | pass | pass | not applicable |
| validate_transposed_length | covered | pass | pass | `expect_error` handled via `Result` |
| draw_text | covered (`render_text` surface mapped) | pass | pass | no error rows |

## 3) Error matrix

- `expect_error = true` cases observed:
  - `imagingft.validate_transposed_length.rotate_90` →
    `PilError::ValueError("text length is undefined for text rotated by 90 or 270 degrees")`
- No other error rows in current public-api corpus.

## 4) Coverage notes

- Test command: `make -C pillow-rs imagingft-tests`
- Coverage command: `make -C pillow-rs imagingft-tests-coverage`
- Script: `scripts/coverage/run_imagingft_rust_coverage.sh`
- Coverage MCP command: `imagingft-tests-coverage-fixed` (id `258e7dec-226f-4b00-9336-04df6e8c67f2`)
- Latest coverage run: `7dc878a2-f1df-46e0-b941-d150402a25bd`
  - `counters`: `passed=1`, `failed=0`
  - `coverage status`: ingested
  - `snapshot id`: `e540ed42-74b2-435d-8c58-35bc888fe572`
  - `branch`: `imagingft`
- Focused suite metrics (imagingft target):
  - `line_rate=0.0783`
  - `branch_rate=0.0290`
  - `function_rate=0.0901`
  - `region_rate=0.0680`

## 5) Remaining gap

- No unresolved public-op parity cases remain in active public-api corpus.
- Focus remains on keeping this corpus current as the source of non-deprecated ImagingFT public coverage.
