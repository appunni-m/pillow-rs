# Fixture Generator System

Fixture generation is part of the `pillow-rs-freetype` harness. It is not disposable scratch work.

Every reference fixture must be reproducible from maintained generator code in this repository. A fixture update is acceptable only when the generator path, oracle source, command, and expected harness effect are clear.

## Generator Contract

- FreeType C generates reference data; Rust never generates its own expected output.
- Generator scripts live under `scripts/` and are reviewed like source code.
- Generated matrices record their generator, FreeType version, fixture family, load flags, render mode, font, size, glyph identity, metrics, bbox, bitmap placement, hashes, and raw byte paths.
- Raw byte files are written under `tests/fixtures/outputs/`.
- Fixture updates must be reproducible from documented commands.
- New fixture families must extend the maintained generator system before they are used in tests.
- Ad hoc one-off scripts must not be required to regenerate committed fixtures.

## Maintained Generators

| Script | Role | Output |
|---|---|---|
| `scripts/build_ft.sh` | Builds vendored FreeType C used by oracle helpers | `freetype/build/` |
| `scripts/gen_ft_refs.c` | C oracle helper for FreeType load/render paths | JSON rows and raw pixel data consumed by Python generators |
| `scripts/build_ft_fixture.py` | Main FreeType-path matrix generator | `tests/fixtures/*_matrix.json`, `tests/fixtures/outputs/raws_*` |
| `scripts/build_native_tt_fixture.py` | Compatibility wrapper for native TT fixture generation | `native_tt_default_matrix.json` |
| `scripts/build_render_mode_fixture.py` | Dedicated render-mode fixture generator | `render_mode_matrix.json`, `tests/fixtures/outputs/render_modes` |
| `scripts/build_fixtures.py` | Legacy force-autohint inventory pipeline | `font_inventory.json`, `force_autohint_matrix.json` |
| `scripts/classify_failure_ids.py` | Developer triage report from `coverage_matrix_tests` failure ID files | Markdown summary; no fixture changes |
| `scripts/extract_blues.py` | Generates blue string Rust data from FreeType source | Rust source tables |
| `scripts/generate_globals.py` | Generates script/style global data from FreeType source | Rust source tables |
| `scripts/generate_script_meta.py` | Generates script metadata from FreeType source | Rust source tables |

Prefer `scripts/build_ft_fixture.py` for new fixture families. Keep wrappers only when they preserve stable historical commands.

## Standard Reproduction Flow

From the repository root:

```bash
bash scripts/build_ft.sh
python3 scripts/build_ft_fixture.py --family force_autohint --build-ref-bin
python3 scripts/build_ft_fixture.py --family native_tt_default
python3 scripts/build_ft_fixture.py --family no_hinting
python3 scripts/build_ft_fixture.py --family metrics_only
python3 scripts/build_ft_fixture.py --family outline_cbox
python3 scripts/build_ft_fixture.py --family render_mono
python3 scripts/build_ft_fixture.py --family render_lcd
python3 scripts/build_render_mode_fixture.py
```

`build_ft_fixture.py` uses `FT_REF_BIN` when set. Without it, it uses `/tmp/gen_refs_v4`; the `--build-ref-bin` option builds that helper from `scripts/gen_ft_refs.c` after `scripts/build_ft.sh` has produced the vendored FreeType library.

Pass `--small` only for explicit seed/debug regeneration. Committed supplemental parity fixtures use the full font inventory by default.

## Fixture Update Checklist

Before committing fixture changes:

1. Regenerate through a maintained script under `scripts/`.
2. Do not edit generated matrix rows or raw byte files by hand.
3. Confirm the matrix `generator`, `fixture_family`, `load_flags`, and `render_mode` are correct.
4. Run the exact gate or contract that owns the fixture family.
5. Run `cargo test --test harness_contract --locked`.
6. Document any threshold, incomplete, small-baseline, or unexecuted state as debt.

## Adding A New Fixture Family

1. Add family support to `scripts/gen_ft_refs.c`.
2. Add the family to `FAMILIES` in `scripts/build_ft_fixture.py`.
3. Generate matrix rows and raw bytes with stable IDs.
4. Add provenance and breadth checks to `tests/harness_contract.rs`.
5. Add runner support to `tests/coverage_matrix_tests.rs`.
6. Add or update `interface_map.json` only with truthful `passing/total` values.
7. Update this document if the reproduction command changes.

The family is not an exact gate until the default tests execute every active row and fail on mismatches.

## Failure Classification Reports

Failure classification reports are maintained developer triage artifacts, not
fixtures.  Use `scripts/classify_failure_ids.py` with the lane-specific
`/tmp/pillow_failure_ids.txt` files emitted by `coverage_matrix_tests`.

See `doc/PARITY_FAILURE_CLASSIFICATION.md` for the exact capture and report
commands.
