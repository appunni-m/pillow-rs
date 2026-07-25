# ImagingFT parity status (public API corpus)

Date: 2026-07-25

## Scope

- Active corpus: `pillow-rs/tests/fixtures/imagingft/inputs/public-api/*`
- Deprecated corpus excluded: `deprecated/imagingft/*`
- Public operations represented: `getname`, `getmetrics`, `getlength`, `has_variations`, `getbbox`, `getbbox_binary`, `getmask`, `getmask2`, `getmask2_with_start`, `get_transposed_mask`, `transposed_bbox`, `validate_transposed_length`, `draw_text`, plus `render_text_binary` and `unsupported_magic` as public fixture-surface cases.
- Runner model: manifest-driven (`imagingft_public_api`), one pass over JSONs under `inputs/public-api`, operation canonicalization via `strip_prefix("imagingft.")`.

## Verification run

- `make -C pillow-rs imagingft-tests` → pass (`1` passed, `0` failed)
Coverage MCP (fresh for current runner checks):
  1. `project_context`
  2. `run_test` (`imagingft-tests-coverage-fixed`, idempotency `imagingft-tests-coverage-fixed-continue-run-2`)
  3. `get_run_data` (terminal)
  4. `coverage_query` (summary + files)

Latest run/snapshot:
- run id: `5d366690-151d-440e-be94-b3d8c3981bac`
- snapshot id: `883205f3-16bc-4757-bf46-db95692fd955`
- branch/commit: `main` / `fab8f0d4ea6db38545c1c0284a695f772973cae1`

## Suite outcome

- Fixture rows covered by test corpus: **45**
- Fixture rows executed: **45/45**
- Success rows: **41**
- Error rows: **4**

## Per-operation parity matrix (public-api corpus)

| op | cases | ok | error | result |
|---|---:|---:|---:|---|
| `draw_text` | 3 | 3 | 0 | pass |
| `getbbox` | 3 | 3 | 0 | pass |
| `getbbox_binary` | 3 | 3 | 0 | pass |
| `getlength` | 3 | 3 | 0 | pass |
| `getmask` | 5 | 5 | 0 | pass |
| `getmask2` | 5 | 5 | 0 | pass |
| `getmask2_with_start` | 5 | 5 | 0 | pass |
| `getmetrics` | 1 | 1 | 0 | pass |
| `getname` | 2 | 1 | 1 | pass |
| `has_variations` | 1 | 1 | 0 | pass |
| `get_transposed_mask` | 4 | 3 | 1 | pass |
| `render_text_binary` | 3 | 3 | 0 | pass |
| `transposed_bbox` | 3 | 3 | 0 | pass |
| `unsupported_magic` | 1 | 0 | 1 | pass |
| `validate_transposed_length` | 3 | 2 | 1 | pass |

## Error matrix

All expected-error rows are validated through fixture-driven `expect_error=true` + `expectation.expected.error` and Rust `Result` error path:

- `imagingft.unsupported_operation.unsupported_magic` → `NotImplementedError`
- `imagingft.get_transposed_mask.invalid_orientation_error` → `ValueError` (`Unknown transpose method: UNSUPPORTED...`)
- `imagingft.validate_transposed_length.rotate_90` → `ValueError` (`text length is undefined for text rotated by 90 or 270 degrees`)
- `imagingft.load_failure.missing_font_asset` → `ValueError` (`font bytes read failed (...)`)

## Coverage summary (latest snapshot)

- suite: `imagingft`
- snapshot: `0332cd5a-1d82-43ab-8448-636a0282e18d`
- `line_rate=0.07997`
- `branch_rate=0.02960`
- `function_rate=0.09262`

Compared with previous suite snapshot (`4ddb64b0-d2bd-47b9-9b42-5e716cbf9247`): no suite-wide deltas.

Targeted core surface (`pillow-rs/src/font/imagingft.rs`):
- `line_rate=0.84123`, `branch_rate=0.63514`, `function_rate=0.90244`
- unresolved lines: `60`
- partial branches: `27`
- explicit unresolved windows: `35`, `36`, `59`, `127..129`, `138`, `145`, `152..154`, `163..166`, `173`, `193`, `215`, `220`, `221`, `228`, `373`, `374`, `444`, `461..462`, `471..472`, `474..475`, `497`, `498`, `502`, `503`, `509`, `510`, `514`, `515`, `520`, `521`, `524`, `526`, `527`, `534`, `540`, `544..554`, `555..563`, `564`, `565`, `567`, `570..571`, `576`, `577`, `585`, `592`.

## Completion status against objective

- Fixture-driven Result-based parity execution: **implemented**
- Output shape/hash/raw checks retained: **implemented**
- Required public surfaces present in corpus: **implemented**
- Error-path parity rows represented (load/layout/unsupported): **implemented**
- Single source manifest surface coverage (no deprecated fixture dependency): **implemented**
- Full 100% coverage across target suite/surface: **not reached** (remaining explicit imagingft.rs coverage gaps listed above)
