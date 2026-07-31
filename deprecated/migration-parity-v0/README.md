# Deprecated migration-parity archive

This tree contains the pre-`migration-parity/manifest@2` parity consumers,
inputs, generated oracle outputs, and their generators. It is retained for
provenance and migration mapping only. Nothing below this directory is an
active test input, coverage source, benchmark workload, or result source.

## Replacement authority

The active specification is:

- manifest: `pillow-rs/tests/fixtures/manifest.yaml`
- parity inputs: `pillow-rs/tests/fixtures/inputs/parity/`
- coverage inputs: `pillow-rs/tests/fixtures/inputs/coverage/`
- benchmark inputs: `pillow-rs/tests/fixtures/inputs/benchmark/`
- live runner: `scripts/run_migration_parity.py`
- result interfaces: `scripts/validate_migration_parity_result.py`

The active denominator is the frozen Pillow 12.2.0 project authority expanded
by `scripts/migration_parity_inventory.py`. Old fixture names, expected output
hashes, and old module-level manifest rows do not add operations to that
denominator.

## Archived groups

| Archive group | Former role | Active replacement |
| --- | --- | --- |
| `python/` | Pytest fixture/oracle tests and their shared `conftest.py` | Manifest-driven live source/target workflows |
| `rust/` | Rust integration parity/oracle tests and old Cargo test fixtures | Manifest-driven Python facade parity; Rust unit tests remain under `pillow-rs/src` |
| `wasm/` | WASM fixture/oracle corpus, execution engine, and browser parity tests | No active WASM profile is declared in the current manifest; add a reviewed profile before reactivation |
| `fixtures/` | Image backend, image-eval, fromarray, encoded-input, and expected-output corpora | Recreate only as input-only assets/cases under the active indexed tree |
| `scripts/` | Generators and legacy coverage tooling for the archived corpus | `build_migration_parity_*`, `run_migration_*`, and strict validators |
| `manifest.yaml` | Obsolete project-wide numeric-version manifest | `pillow-rs/tests/fixtures/manifest.yaml` (`migration-parity/manifest@2`) |

## Migration rules

1. Do not import or read this archive from an active runner.
2. Do not copy expected values, expected errors, output bytes, or hashes into
   active parity inputs.
3. When an archived scenario is needed, map it to a canonical operation and
   requirement ID, then express only its public stimulus in an active workflow.
4. Retain the archive until equivalent live parity and managed coverage
   evidence are available for the mapped requirement.

This file is a provenance record, not a result and not a second manifest.
