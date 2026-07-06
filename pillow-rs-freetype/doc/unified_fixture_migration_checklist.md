# Unified Fixture Migration Checklist

Baseline runner/doc commit: `ec027266`

The active source is `tests/fixtures/inputs/public-api/*.json`. Workers process
30-file slices and commit JSON-only changes from separate worktrees.

## Slice Status

| Slice | Files | Branch | Status |
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

## Remaining Aggregate-Axis Slices

After the initial 300-file pass, an audit found 77 files with 85 aggregate-ish
cases that still need explicit `inputs.variability.axes`.

| Slice | Files | Branch | Status |
|---|---:|---|---|
| remaining-00 | 30 | `codex/unified-inputs-remaining-00` | pending |
| remaining-01 | 30 | `codex/unified-inputs-remaining-01` | pending |
| remaining-02 | 17 | `codex/unified-inputs-remaining-02` | pending |

## Worker Acceptance Criteria

- Only assigned JSON files are changed.
- Cases with common aggregate fields declare `inputs.variability.axes`.
- No per-font, per-size, per-codepoint, or legacy matrix row materialization is
  added.
- No expected outputs are committed.
- Edited JSON files pass `python3 -m json.tool`.
- `git diff --check` passes in the worker worktree.
- Worker branch contains a commit when changes were made.
