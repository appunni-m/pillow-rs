# Unified Fixture Migration Record

The original aggregate-axis migration is complete and superseded by explicit
grouped input variants. This file preserves the historical slice record; it is
not current input-authoring guidance.

Current contracts:

- `doc/unified_fixture_inputs.md` defines the executable JSON format.
- `doc/FONT_FIXTURE_COVERAGE_PLAN.md` defines fixture-corpus and structural
  coverage work.
- `tests/fixtures/inputs/public-api/*.json` contains the only executable public
  parity inputs.

## Historical Slice Status

Baseline runner/doc commit: `ec027266`.

| Slice | Files | Branch | Historical status |
|---|---:|---|---|
| 000-029 | 30 | `codex/unified-inputs-000-029` | complete, no changes |
| 030-059 | 30 | `codex/unified-inputs-030-059` | merged, 1 case |
| 060-089 | 30 | `codex/unified-inputs-060-089` | merged, 11 cases |
| 090-119 | 30 | `codex/unified-inputs-090-119` | complete, no changes |
| 120-149 | 30 | `codex/unified-inputs-120-149` | merged, 6 cases |
| 150-179 | 30 | `codex/unified-inputs-150-179` | merged, 9 cases |
| 180-209 | 30 | `codex/unified-inputs-180-209` | merged, 10 cases |
| 210-239 | 30 | `codex/unified-inputs-210-239-v2` | merged, 18 cases |
| 240-269 | 30 | `codex/unified-inputs-240-269-v2` | merged, 2 cases |
| 270-299 | 30 | `codex/unified-inputs-270-299-v2` | merged, 2 cases |
| remaining-00 | 30 | `codex/unified-inputs-remaining-00` | merged, 35 cases |
| remaining-01 | 30 | `codex/unified-inputs-remaining-01` | merged, 32 cases |
| remaining-02 | 17 | `codex/unified-inputs-remaining-02` | merged, 17 cases |

## Final Migration State

- Top-level `matrix_cases`: removed and rejected.
- Runtime font-folder discovery: removed and rejected.
- Runtime all-glyph enumeration: removed.
- `inputs.variability` axes: removed and rejected.
- Implicit combinations: zero.
- Grouped concrete combinations: `inputs.variants` with mandatory coverage
  intent.
- Oracle and face cache identity: includes fixture content hashes.

Do not reopen the old slice workflow. New coverage work changes focused fonts
and existing public JSON cases directly, then verifies exact C/Rust/C ABI/WASM
interchangeability.
