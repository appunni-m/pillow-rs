# ImagingFT parity status (public API corpus)

Date: 2026-07-25

## 1) Corpus / operation coverage

- Fixture corpus used: `pillow-rs/tests/fixtures/imagingft/inputs/public-api`
- Active fixture rows: all non-empty rows across the non-deprecated public-api manifests
  (`45` total cases), excluding deprecated test files.
- Required public surface asserted in test:
  `getname`, `getmetrics`, `getlength`, `has_variations`, `getbbox`, `getbbox_binary`,
  `getmask`, `getmask2`, `getmask2_with_start`, `get_transposed_mask`,
  `get_transposed_mask` error-path probes, `transposed_bbox`,
  `validate_transposed_length`, `draw_text`, `render_text_binary`.

## 2) Current parity matrix

| Surface | Manifest | Rust status | Oracle status | Error handling |
|---|---|---|---|---|
| getname | covered | pass | pass | not applicable |
| getmetrics | covered | pass | pass | not applicable |
| getlength | covered | pass | pass | not applicable |
| has_variations | covered | pass | pass | not applicable |
| getbbox | covered | pass | pass | not applicable |
| getbbox_binary | covered | pass | pass | not applicable |
| getmask | covered | pass | pass | not applicable |
| getmask2 | covered | pass | pass | not applicable |
| getmask2_with_start | covered | pass | pass | not applicable |
| get_transposed_mask | covered | pass | pass | no-success and failure rows covered |
| transposed_bbox | covered | pass | pass | not applicable |
| validate_transposed_length | covered | pass | pass | `Result`-driven success/error |
| draw_text | covered (`render_text` mapped) | pass | pass | `Result`-driven |
| render_text_binary | covered | pass | pass | `Result`-driven |

## 3) Error matrix

- `expect_error = true` cases observed:
  - `imagingft.validate_transposed_length.rotate_90` →
    `PilError::ValueError("text length is undefined for text rotated by 90 or 270 degrees")`
  - `imagingft.layout_failure` (`imagingft.get_transposed_mask`, unsupported orientation) →
    `PilError::ValueError("Unknown transpose method: UNSUPPORTED. Use FLIP_LEFT_RIGHT, FLIP_TOP_BOTTOM, ROTATE_90, ROTATE_180, ROTATE_270, TRANSPOSE, or TRANSVERSE.")`
  - `imagingft.load_failure` (`getname`, missing font asset) →
    `PilError::ValueError("font bytes read failed (input/fonts/no_such_font.ttf): ...")`
  - `imagingft.unsupported_operation.unsupported_magic` →
    `PilError::NotImplementedError("unsupported imagingft operation: unsupported_magic")`

## 4) Coverage notes

- Test command: `make -C pillow-rs imagingft-tests`
- Coverage command: `make -C pillow-rs imagingft-tests` (fixture parity harness)
- Coverage MCP command: `imagingft-tests-coverage`/`imagingft-tests-coverage-fixed` (name `imagingft-tests-coverage-fixed`, id `258e7dec-226f-4b00-9336-04df6e8c67f2`)
- Latest coverage run: `cc2f37cf-e5d7-4f51-927c-aa1ddb432bbf` (status `passed`)
  - `counters`: `passed=1`, `failed=0`
  - `coverage status`: ingested
  - `snapshot id`: `48b925c4-6171-4801-bf82-37e73ca57588`
  - `branch`: `main`
  - `line_rate=0.07997`
  - `branch_rate=0.02960`
  - `function_rate=0.09262`
  - `region_rate=0.06912`
  - `active public-api rows`: 45 cases
- ImagingFT implementation coverage (from this suite):
  - `pillow-rs/src/font/imagingft.rs`: `line_rate=0.8412`, `branch_rate=0.6351`, `function_rate=0.9024`.
- Coverage status of `pillow-rs/src/font/imagingft.rs` still shows uncovered lines/branches:
  - `uncovered lines`: `60`
  - `uncovered/partial branch lines`: `18`

## 5) Remaining gap

- No unresolved public-op parity rows remain in the active public-api corpus.
- Remaining work is coverage completeness for other suite paths and unexercised branches in `imagingft.rs`:
  - gaps are expected to be addressed by dedicated fixture expansions and/or targeted parser-path probes.
