# ImagingFT parity status (public API corpus)

Date: 2026-07-25

## 1) Scope in force

- Working corpus only: `pillow-rs/tests/fixtures/imagingft/inputs/public-api/*`
- Explicitly excluded: deprecated `deprecated/imagingft/*` fixtures.
- Active fixture rows in scope: **45**.
- Canonical public surfaces represented: `getname`, `getmetrics`, `getlength`, `has_variations`, `getbbox`, `getbbox_binary`, `getmask`, `getmask2`, `getmask2_with_start`, `get_transposed_mask`, `transposed_bbox`, `validate_transposed_length`, `draw_text`.
- Additional manifest-authored fixture-only cases: `imagingft.unsupported_magic`, legacy-prefixed `imagingft.get_transposed_mask` and `imagingft.unsupported_magic` operation labels (runner strips optional `imagingft.` prefix and compares by canonical op name).

## 2) Execution + gate evidence

- `make -C pillow-rs imagingft-tests`
  - result: pass (single test `imagingft_public_api_parity_matches_fixture_oracles`)
  - observed cases: **45/45**
- Coverage MCP flow:
  1. `project_context`
  2. `run_test` on command `imagingft-tests-coverage-fixed` (`258e7dec-226f-4b00-9336-04df6e8c67f2`)
  3. `get_run_data` (terminal)
  4. `coverage_compare` (overview/files for imagingft suite)
- run id: `9163d5bb-cfce-44de-9cbc-6de62d4274e6`
- status: `passed` (`terminal` state, no failures)
- counters: `passed=1`, `failed=0`
- coverage snapshot: `94f7f3a0-21fa-43c6-a21d-a40d032c1592`
- suite/branch/commit: `imagingft` / `main` / `67b86bd399c6cc36277c31cb97f367deb5674259`

### Snapshot metric deltas

- current snapshot (`94f7f3a0...`):
  - `line_rate=0.07997`, `branch_rate=0.02960`, `function_rate=0.09262`, `region_rate=0.06912`
- previous ingested imagingft snapshot (`743979ef...`) showed identical rates in this surface scope.
- target-surface focused projection (`pillow-rs/src/font/imagingft.rs`):
  - `line_rate=0.84123`, `branch_rate=0.63514`, `function_rate=0.90244`
  - unresolved in this suite: `60` uncovered lines / `18` partial branch lines

## 3) Per-operation parity status matrix

- Total fixture rows: **45**
- Success vs expected-error: **42 pass**, **3 error**.

| Fixture operation key | Canonical op | Cases | Expected success | Expected error | Result |
|---|---|---:|---:|---:|---|
| `getname` | getname | 2 | 1 | 1 | pass |
| `getmetrics` | getmetrics | 1 | 1 | 0 | pass |
| `getlength` | getlength | 3 | 3 | 0 | pass |
| `has_variations` | has_variations | 1 | 1 | 0 | pass |
| `getbbox` | getbbox | 3 | 3 | 0 | pass |
| `getbbox_binary` | getbbox_binary | 3 | 3 | 0 | pass |
| `getmask` | getmask | 5 | 5 | 0 | pass |
| `getmask2` | getmask2 | 5 | 5 | 0 | pass |
| `getmask2_with_start` | getmask2_with_start | 5 | 5 | 0 | pass |
| `get_transposed_mask` | get_transposed_mask | 3 + 1 alias row | 3 | 1 | pass |
| `transposed_bbox` | transposed_bbox | 3 | 3 | 0 | pass |
| `validate_transposed_length` | validate_transposed_length | 3 | 2 | 1 | pass |
| `draw_text` | draw_text | 3 | 3 | 0 | pass |
| `render_text_binary` | render_text_binary | 3 | 3 | 0 | pass |

## 4) Error matrix (expected-error rows)

- `imagingft.validate_transposed_length.rotate_90`
  - expected status: `error`
  - expected type: `ValueError`
  - expected message: `text length is undefined for text rotated by 90 or 270 degrees`
- `imagingft.unsupported_operation.unsupported_magic`
  - operation alias: `imagingft.unsupported_magic`
  - expected status: `error`
  - expected type: `NotImplementedError`
  - expected message: `unsupported imagingft operation: unsupported_magic`
- `imagingft.layout_failure.imagingft_get_transposed_mask_invalid_orientation_error`
  - expected status: `error`
  - expected type: `ValueError`
  - expected message fragment: `Unknown transpose method: UNSUPPORTED`
- `imagingft.load_failure.missing_font_asset`
  - expected status: `error`
  - expected type: `ValueError`
  - expected message: includes missing path + OS I/O text

All expected-error cases are asserted through `Result`-driven matching in
`pillow-rs/tests/imagingft_public_api.rs` using fixture-provided `type` and `message`/`message_pattern` fields.

## 5) Remaining gaps (explicit)

- `pillow-rs/src/font/imagingft.rs` has unresolved defensive paths and branch edges in this suite:
  - `uncovered_line_count=60`
  - `partial_branch_line_count=18`
- These arise from:
  - bitmap/font-mode branches not exercised by current public-api fixtures,
  - layout/error/overflow fallback logic,
  - optional-path guards in mask composition.
- No unresolved public operation rows remain for the current non-deprecated public corpus. Parity itself is green for all **45 active fixture rows**; coverage completeness is the only known blocker to a full 100% claim.

