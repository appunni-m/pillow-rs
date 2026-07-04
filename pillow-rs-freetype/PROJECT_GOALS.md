# Project Goals

`pillow-rs-freetype` exists to prove one thing: a 100% Rust runtime can match FreeType C behavior exactly.

The project succeeds only when the harness makes false success impossible. Rust code must have one path to green: produce the same values, metadata, pixels, and bytes as the FreeType C oracle for every in-scope endpoint and fixture row.

## Non-Negotiable Goal

- Runtime implementation is 100% Rust.
- FreeType C is the oracle for references, never the runtime engine.
- Exact pixel/byte/value parity is the goal; approximate visual similarity is failure.
- Broad fixture matrices must not be reduced to smoke tests.
- A test that passes by threshold, skipped oracle, missing raw bytes, or unexecuted fixture presence is not a parity gate.
- Incomplete work must be named as debt until it becomes an exact executable gate.
- Fixture generation and reproducibility are part of the harness, not ad hoc maintenance work.

## Runtime Boundary

The runtime crate must not contain:

- `build.rs` C compilation or runtime C linking.
- `extern "C"` runtime bindings.
- `native_ft` bridge code.
- `freetype-sys`, `bindgen`, `pkg-config`, `cc`, or `rustc-link-lib=freetype`.

C is allowed only under oracle tooling:

- Vendored FreeType source for audits.
- Maintained `scripts/` helpers that generate reference fixtures.
- Test-local oracle helpers for scalar parity, provided they are not linked into the runtime crate.

## Generator System

Fixture generators are project infrastructure. They must be reproducible, documented, and covered by contract tests.

- `doc/GENERATOR_SYSTEM.md` is the source of truth for fixture generation.
- Generator scripts live under `scripts/` and are reviewed like source code.
- New fixture families must extend `scripts/gen_ft_refs.c` and `scripts/build_ft_fixture.py` unless there is a documented reason for a dedicated generator.
- Committed matrices must identify their generator, fixture family, load flags, render mode, and raw byte paths.
- Fixture updates must be reproducible through documented commands.
- One-off scripts are not acceptable as hidden dependencies for future fixture maintenance.

## Harness Goal

The harness is the project control system. It must enforce all of these rules:

- Required matrices fail if missing.
- Exact render gates compare raw pixel bytes and bitmap metadata.
- Scalar gates compare exact values.
- Table gates compare raw bytes.
- Error gates compare exact error behavior for invalid inputs.
- Fixture update paths cannot bless Rust output as the reference.
- Coverage contracts lock row counts and operation families so broad matrices cannot shrink quietly.
- Interface reports cannot claim a path is 100% when its fixture family is incomplete.
- Unexecuted fixture families must be listed as debt, not counted as parity.

## Current Gate Status

Exact gates:

- `force_autohint_matrix.json`: exact broad `getmask` and `getbbox` gate, 22,168 rows.
- `render_mode_matrix.json`: exact raw byte and metadata gate for the current render-mode matrix.
- `fixed_parity.rs`: mandatory scalar C-oracle comparison for fixed-point math.
- `core_face_size_charmap.rs`: exact API behavior checks for current face, size, charmap, and SFNT table coverage.
- `no_runtime_ffi.rs`: runtime boundary guard.
- `harness_contract.rs`: gate strength and fixture breadth guard.
- `generator_contract.rs`: generator documentation and reproducibility guard.

Incomplete gates:

- `native_tt_default_matrix.json`: threshold baseline only, currently `3176/7640`. This is not complete parity. It must become `7640/7640` exact before the native TrueType bytecode path can be called done.
- `render_mono_matrix.json`: executed baseline only, currently `0/8`. This is not complete parity.
- `render_lcd_matrix.json`: executed baseline only, currently `0/8`. This is not complete parity.

Present-but-unexecuted fixture debt:

- `metrics_only_matrix.json`
- `no_hinting_matrix.json`
- `outline_cbox_matrix.json`

These fixtures are evidence that C references exist. They are not success until the runner executes them as exact gates.

## Promotion Rules

A fixture family can move to "exact gate" only when:

1. The Rust implementation path exists.
2. The matrix is generated from FreeType C oracle output.
3. The default test runner executes every active row.
4. Every active row compares the required raw bytes, values, metadata, and errors.
5. The test fails on missing fixtures, unsupported operations, or missing raw references.
6. `interface_map.json` reports truthful `passing/total` numbers.

No plan item is complete because code exists. It is complete only when the harness forces exact parity and passes.

## Required Habit

When changing renderer, scaler, bytecode hinter, rasterizer, table parser, fixtures, or tests:

1. Keep broad matrices broad.
2. Prefer adding rows over replacing coverage with spot checks.
3. Use C only to produce expected data.
4. Make Rust output match the C expected data.
5. Tighten the harness before trusting a new pass count.
6. Document threshold, partial, and unexecuted states as unfinished work.
