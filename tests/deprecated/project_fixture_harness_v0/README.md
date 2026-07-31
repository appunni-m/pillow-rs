# Deprecated Project Fixture Harness v0

This directory preserves the old stored-output Pillow/RSPIL parity harness only
as migration evidence. It is not an active test suite and must not be used as
an oracle by the replacement migration-parity system.

## Archived code

| Former active path | Deprecated path |
| --- | --- |
| `tests/engine.py` | `tests/deprecated/project_fixture_harness_v0/tests/engine.py` |
| `tests/fixture_coverage.py` | `tests/deprecated/project_fixture_harness_v0/tests/fixture_coverage.py` |
| `tests/test_parity.py` | `tests/deprecated/project_fixture_harness_v0/tests/test_parity.py` |
| `scripts/generate_fixtures.py` | `tests/deprecated/project_fixture_harness_v0/scripts/generate_fixtures.py` |

## Archived data

The code formerly consumed these separately retained data archives:

| Former active path | Deprecated path |
| --- | --- |
| `tests/fixtures/` | `tests/deprecated/fixtures/` |
| `tests/fixtures_2/` | `tests/deprecated/fixtures_2/` |

The old inputs may be migrated into input-only public workflows. The old
outputs may be inspected only to understand historical coverage and migration
provenance. Active parity evidence must execute the pinned Pillow `12.2.0`
oracle and the public `pillow_rs` target independently from the same new input
workflow.

The canonical legacy-row expansion is maintained by:

```text
make migration-parity-inventory-check
```
