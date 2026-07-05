# Fixture Generator System

Fixture generation is part of the `fontdone` harness. It is not disposable scratch work.

Every reference fixture must be reproducible from maintained generator code in this repository. A fixture update is acceptable only when the generator path, oracle source, command, and expected harness effect are clear.

## Generator Contract

- FreeType C generates reference data; Rust never generates its own expected output.
- Generator scripts live under `scripts/` and are reviewed like source code.
- Generated matrices record their generator, FreeType version, fixture family, load flags, render mode, font, size, glyph identity, metrics, bbox, bitmap placement, hashes, and raw byte paths.
- Raw byte files are written under ignored `tests/fixtures/outputs/`.
- Fixture updates must be reproducible from documented commands.
- New fixture families must extend the maintained generator system before they are used in tests.
- Ad hoc one-off scripts must not be required to regenerate fixtures.
- The only tracked files under `tests/fixtures/` should be font inputs. Generated
  matrices, font inventory, and raw byte outputs are local artifacts.

## Maintained Generators

| Script | Role | Output |
|---|---|---|
| `scripts/fetch_ft.sh` | Fetches and verifies pinned FreeType 2.14.3 source | ignored `freetype/` |
| `scripts/build_ft.sh` | Builds fetched FreeType C used by oracle helpers | ignored `freetype/build/` |
| `scripts/gen_ft_refs.c` | C oracle helper for FreeType load/render paths | JSON rows and raw pixel data consumed by Python generators |
| `scripts/build_ft_fixture.py` | Main FreeType-path matrix generator | `tests/fixtures/*_matrix.json`, `tests/fixtures/outputs/raws_*` |
| `scripts/build_native_tt_fixture.py` | Compatibility wrapper for native TT fixture generation | `native_tt_default_matrix.json` |
| `scripts/build_render_mode_fixture.py` | Dedicated render-mode fixture generator | `render_mode_matrix.json`, `tests/fixtures/outputs/render_modes` |
| `scripts/build_fixtures.py` | Legacy force-autohint inventory pipeline | `font_inventory.json`, `force_autohint_matrix.json` |
| `scripts/classify_failure_ids.py` | Developer triage report from `coverage_matrix_tests` failure ID files | Markdown summary; no fixture changes |
| `scripts/audit_api_abi.py` | Three-way FreeType C / Servo binding / fontdone API and ABI surface audit | `target/api-abi-audit/api_abi_audit.{json,md}` |
| `scripts/extract_blues.py` | Generates blue string Rust data from FreeType source | Rust source tables |
| `scripts/generate_globals.py` | Generates script/style global data from FreeType source | Rust source tables |
| `scripts/generate_script_meta.py` | Generates script metadata from FreeType source | Rust source tables |

Prefer `scripts/build_ft_fixture.py` for new fixture families. Keep wrappers only when they preserve stable historical commands.

Current generated fixture families:

- `native_tt_default`
- `force_autohint`
- `no_hinting`
- `metrics_only`
- `outline_cbox`
- `render_mono`
- `render_lcd`

## Standard Reproduction Flow

From the repository root:

```bash
make fixtures
```

`make fixtures` fetches the pinned FreeType source, builds the C oracle helper,
regenerates `font_inventory.json`, then regenerates every matrix and raw byte
family. Use the narrower `make fixture-*` targets when intentionally refreshing
one family.

Pass `--small` only for explicit seed/debug regeneration. Supplemental parity
fixtures use the full font inventory by default.

## Fixture Update Checklist

Before committing fixture changes:

1. Regenerate through a maintained script under `scripts/`.
2. Do not edit generated matrix rows or raw byte files by hand.
3. Confirm the matrix `generator`, `fixture_family`, `load_flags`, and `render_mode` are correct.
4. Run the exact gate or contract that owns the fixture family.
5. Run `make test-harness`.
6. Do not commit generated matrices or raw byte files.
7. Document any threshold, incomplete, small-baseline, or unexecuted state as debt.

## Adding A New Fixture Family

1. Add family support to `scripts/gen_ft_refs.c`.
2. Add the family to `FAMILIES` in `scripts/build_ft_fixture.py`.
3. Generate matrix rows and raw bytes with stable IDs.
4. Add provenance and breadth checks to `tests/harness_contract.rs`.
5. Add runner support to `tests/coverage_matrix_tests.rs`.
6. Add or update `tests/data/interface_map.json` only with truthful `passing/total` values.
7. Update this document if the reproduction command changes.

The family is not an exact gate until the default tests execute every active row and fail on mismatches.

## Failure Classification Reports

Failure classification reports are maintained developer triage artifacts, not
fixtures.  Use `scripts/classify_failure_ids.py` with the lane-specific
`/tmp/freetype_failure_ids.txt` files emitted by `coverage_matrix_tests`.

See `doc/PARITY_FAILURE_CLASSIFICATION.md` for the exact capture and report
commands.

## API And ABI Audit

`make api-abi-audit` compares pinned FreeType C public headers, Servo's
`rust-freetype` binding surface, and the local `fontdone` public Rust surface.
This is stricter than endpoint coverage: it records C function signatures,
macro constants, typedefs, struct fields, enum variants, Servo exposure, and
the current `fontdone` mapping/status from `tests/data/interface_map.json`.

Use this report when planning the future C ABI replacement layer. The safe Rust
API can preserve FreeType semantics without being ABI-compatible; a C
replacement must additionally export `FT_*` symbols and `repr(C)` record shapes
with matching field names, order, units, and numeric constants.

The compatibility target is FreeType C itself, not Servo `rust-freetype`.
Servo is useful to compare what an FFI binding exposes, but a binding's choices
are not sufficient for replacement claims.
