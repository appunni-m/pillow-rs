# ImagingFT parity status (public API corpus)

Date: 2026-07-25

## Scope

- Active corpus: `pillow-rs/tests/fixtures/imagingft/inputs/public-api/*`
- Deprecated corpus excluded: `deprecated/imagingft/*`
- Public operations represented: `getname`, `getmetrics`, `getlength`, `has_variations`, `getbbox`, `getbbox_binary`, `getmask`, `getmask2`, `getmask2_with_start`, `get_transposed_mask`, `transposed_bbox`, `validate_transposed_length`, `draw_text`.
- Additional error-only fixtures: `unsupported_magic`, `layout_failure` and `load_failure` (kept by fixture surface).
- Runner model: manifest-driven (`imagingft-public_api`), one pass over JSONs under `inputs/public-api`, operation canonicalization via `strip_prefix("imagingft.")`.

## Verification run

- `make -C pillow-rs imagingft-tests` → pass (`1` passed, `0` failed)
- Coverage MCP:
  1. `project_context`
  2. `run_test` (`imagingft-tests-coverage-fixed`, idempotency `imagingft-tests-coverage-fixed-required-ops-check`)
  3. `get_run_data` (terminal)
  4. `coverage_query` (summary + file)

Latest run/snapshot:
- run id: `79bc27fb-ce75-443d-b71e-18d7633315bb`
- snapshot id: `6ab373c0-ca5f-4a51-b6fa-1bcc921d4ef9`
- branch/commit: `main` / `3951915e64613af9193004805da924f26e46d556`

## Suite outcome

- Fixture rows covered by test corpus: **45**
- Fixture rows executed: **45/45**
- Success rows: **42**
- Error rows: **3**

## Per-operation parity matrix (public-api corpus)

| op | cases | ok | error | result |
|---|---:|---:|---:|---|
| `getname` | 2 | 1 | 1 | pass |
| `getmetrics` | 1 | 1 | 0 | pass |
| `getlength` | 3 | 3 | 0 | pass |
| `has_variations` | 1 | 1 | 0 | pass |
| `getbbox` | 3 | 3 | 0 | pass |
| `getbbox_binary` | 3 | 3 | 0 | pass |
| `getmask` | 5 | 5 | 0 | pass |
| `getmask2` | 5 | 5 | 0 | pass |
| `getmask2_with_start` | 5 | 5 | 0 | pass |
| `get_transposed_mask` | 4 | 3 | 1 | pass |
| `transposed_bbox` | 3 | 3 | 0 | pass |
| `validate_transposed_length` | 3 | 2 | 1 | pass |
| `draw_text` | 3 | 3 | 0 | pass |

## Error matrix

All expected-error rows are validated through fixture-driven `expect_error=true` + `expectation.expected.error` and Rust `Result` error path:

- `imagingft.unsupported_operation.unsupported_magic` → `NotImplementedError`
- `imagingft.get_transposed_mask.invalid_orientation_error` → `ValueError` + message contains `Unknown transpose method: UNSUPPORTED`
- `imagingft.validate_transposed_length.rotate_90` → `ValueError` (`text length is undefined for text rotated by 90 or 270 degrees`)
- `imagingft.load_failure.missing_font_asset` → `ValueError` (`font bytes read failed (...)`)

## Coverage summary (latest snapshot)

- suite: `imagingft`
- snapshot: `6ab373c0-ca5f-4a51-b6fa-1bcc921d4ef9`
- `line_rate=0.07997`
- `branch_rate=0.02960`
- `function_rate=0.09262`

Compared with previous suite snapshot (`ec7dbed7-bf33-481d-ab0f-9e2384669533`): no suite-wide deltas.

Targeted core surface (`pillow-rs/src/font/imagingft.rs`):
- `line_rate=0.84123`, `branch_rate=0.63514`, `function_rate=0.90244`
- unresolved lines: `60`
- partial branches: `18`

## Completion status against objective

- Fixture-driven Result-based parity execution: **implemented**
- Output shape/hash/raw checks retained: **implemented**
- Required public surfaces present in corpus: **implemented**
- Error-path parity rows represented (load/layout/unsupported): **implemented**
- Full 100% coverage across target suite/surface: **not reached** (remaining imagingft.rs coverage gaps are explicit above)
