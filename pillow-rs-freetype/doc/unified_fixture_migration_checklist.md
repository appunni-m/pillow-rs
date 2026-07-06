# Unified Fixture Migration Checklist

Baseline runner/doc commit: `ec027266`

The active source is `tests/fixtures/inputs/public-api/*.json`. Workers process
30-file slices and commit JSON-only changes from separate worktrees.

## Slice Status

| Slice | Files | Branch | Status |
|---|---:|---|---|
| 000-029 | 30 | `codex/unified-inputs-000-029` | complete, no changes |
| 030-059 | 30 | `codex/unified-inputs-030-059` | active |
| 060-089 | 30 | `codex/unified-inputs-060-089` | active |
| 090-119 | 30 | `codex/unified-inputs-090-119` | active |
| 120-149 | 30 | `codex/unified-inputs-120-149` | active |
| 150-179 | 30 | `codex/unified-inputs-150-179` | active |
| 180-209 | 30 | `codex/unified-inputs-180-209` | active |
| 210-239 | 30 | `codex/unified-inputs-210-239` | pending thread slot |
| 240-269 | 30 | `codex/unified-inputs-240-269` | pending thread slot |
| 270-299 | 30 | `codex/unified-inputs-270-299` | pending thread slot |

## Worker Acceptance Criteria

- Only assigned JSON files are changed.
- Cases with common aggregate fields declare `inputs.variability.axes`.
- No per-font, per-size, per-codepoint, or legacy matrix row materialization is
  added.
- No expected outputs are committed.
- Edited JSON files pass `python3 -m json.tool`.
- `git diff --check` passes in the worker worktree.
- Worker branch contains a commit when changes were made.
