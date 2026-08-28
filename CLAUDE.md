# pillow-rs Agent Instructions

This file is the default briefing for agents working in this repository.
Keep it short, contextual, and enforceable. Put long debugging playbooks in
skills or project docs, then link to them here.

`AGENTS.md` and `AGENT.md` are symlinks to this file. Maintain this file only.

## First Principles

- Do the work end to end: explore, implement, verify, and report.
- Prefer existing repo patterns over new abstractions.
- Do not hand the task back to the user when the next step is actionable.
- Never hide failures by weakening tests, thresholds, fixtures, or expected
  outputs.
- Keep main clean unless you are intentionally integrating a reviewed change.
- Runtime code must be pure Rust unless a crate is explicitly a binding crate.

## Skills To Load

Use skills instead of repeating long procedure in this file.

- `rust-development`: any Rust implementation, refactor, API design, tests,
  borrow/lifetime work, or performance-sensitive Rust code.
- `coding-guidelines`: Rust naming, formatting, clippy, or code review style.
- `systematic-debugging`: stuck bugs, C-vs-Rust first divergence tracing,
  porting algorithms, root cause analysis, and pipeline instrumentation.
- `.claude/skills/freetype-parity`: any external `fontdone` parity, fixture,
  harness, native TrueType, autohinter, rasterizer, metrics, bbox/cbox, or
  subagent-split task.
- `.claude/skills/fix-pil-parity`: PIL/RSPIL fixture parity outside the
  FreeType-specific harness.
- `.claude/skills/compute-backend`: GPU/CPU backend, shader, or compute path
  work.
- `unsafe-check`, `unsafe-checker`, or `unsafe-review`: unsafe Rust, raw
  pointers, FFI audits, repr/layout, or soundness questions.

If a named skill is unavailable in the current agent environment, read the
repo-local skill file directly when present and continue.

## Repository Shape

Workspace crates:

- `pillow-rs/`: pure Rust image logic. No binding dependencies.
- `pillow-rs-py/`: PyO3 wrapper. Keep it thin.
- `pillow-rs-js/`: wasm-bindgen wrapper. Keep it thin.
- `build/fontdone-src/`: pinned GitHub checkout of the standalone pure Rust
  FreeType-compatible implementation and parity harness. The Cargo package/
  crate name is `fontdone`; the root Makefile owns the checkout at the pinned
  `FONTDONE_REF`.

Core crates never touch Python objects, JS objects, file paths, or network.
Core takes Rust primitives and returns Rust primitives. I/O and conversion live
in binding crates.

The maintained ownership map and generated source tree live in
`docs/REPO_MAP.md`. When important files move, are added, or are removed, update
that document with `make repo-map-update` and verify it with
`make repo-map-check`.

## Non-Negotiable Rules

- No runtime FFI shortcuts in core or the pinned `fontdone` checkout: no `freetype-sys`,
  `bindgen`, `cc`, `extern "C"`, `dlopen`, or native FreeType calls.
- C/Pillow/FreeType references are read-only oracles for fixture generation,
  diagnosis, and trace comparison.
- Never edit fixture output/input JSON, oracle data, expected hashes, or
  thresholds to make tests pass.
- Never commit temporary debug prints. Permanent traces must use guarded
  `log::trace!` patterns.
- Never work on legacy `pillow-rs-font`; keep FreeType effort focused on the
  pinned GitHub `fontdone` repository through the root Makefile checkout.
- Do not use destructive git commands like `git reset --hard` or broad
  checkouts unless the user explicitly asks.
- Do not revert user changes. Work with them or ask only if they make progress
  impossible.
- All FFI layers must be thin wrappers. `fontdone-ffi-c` and `fontdone-ffi-wasm`
  may own raw-pointer validation, handle lifetime, `repr(C)` record copying,
  and C-ABI boilerplate — they must not contain font parsing, glyph rendering,
  math algorithms, fixture interpretation, native FreeType calls, or any
  parity-specific behavior. The core crate `fontdone` owns all logic and must
  be 100% safe Rust (`#![deny(unsafe_code)]`).

## Binding Rules

`pillow-rs-py/python/pillow_rs/` must stay thin:

- No algorithmic loops or list comprehensions.
- No math-heavy logic.
- No filesystem/subprocess/tempfile logic.
- `if`/`elif`/`else` only for type checks, `None` defaults, or mode dispatch.
- Bindings delegate to Rust core via `_core.xxx()` or `_rust_image.xxx()`.

FreeType FFI/ABI crates must stay thin:

- `fontdone` is the pure-Rust implementation and owns behavior.
- `fontdone-ffi-c` exports only the intentionally implemented C ABI symbols.
  It may own `repr(C)` records, handles, pointer validation, allocation
  lifetime, and field copying needed to expose the ABI; it must not parse font
  formats, implement glyph logic, interpret fixture JSON, call native FreeType,
  or contain parity-specific behavior.
- `fontdone-ffi-wasm` exports only the intentionally implemented WASM handle
  ABI. It may own linear-memory allocation helpers, handle validation, and ABI
  record copying; it must not contain font parsing or glyph logic.
- Test-only ABI inspection helpers must be feature-gated, must not be
  `no_mangle`, and must not appear in public C headers or exported symbol
  checks.
- Keep `scripts/check_public_api_inputs.py` as the thin-wrapper export gate:
  C ABI exports must be public FreeType symbols only; WASM exports must match
  the explicit WASM export allow-list.

## Drawing Rule

Draw directly in the image's native pixel format. Never convert to RGBA just
to draw.

Expected dispatch families:

```text
Luma8 | LumaA8 | Rgb8 | Rgba8
```

## Logging

Use `log` macros in library code. Do not use `println!` or `eprintln!` in core
library code.

- `error`: corrupt data or unrecoverable failures.
- `warn`: recoverable fallbacks.
- `info`: high-level operations.
- `debug`: algorithm stages and backend choices.
- `trace`: per-scan, per-point, per-pixel internals.

Core crates never initialize the logger. Bindings do that.

Permanent trace pattern:

```rust
#[cfg(debug_assertions)]
if log::log_enabled!(target: "autohint::pipeline", log::Level::Trace) {
    log::trace!(target: "autohint::pipeline", "[TAG] field={}", value);
}
```

## fontdone Goal

The project goal is 100% pure-Rust parity with the version-matched C FreeType
oracle across every public endpoint exposed by `fontdone`.

Parity means:

- Pixel or bitmap byte parity where a rendered mask exists.
- Exact metric parity where metrics are returned.
- Exact outline bbox/cbox parity where geometry is exposed.
- Deterministic, reproducible fixture generation.
- Every incomplete lane remains visible as a failing or explicitly named
  incomplete baseline. Do not narrow the goal to the passing subset.

For FreeType work, load `.claude/skills/freetype-parity` and
`systematic-debugging` before changing code.

Full parity work must fix the pure-Rust implementation first. Do not grow C or
WASM FFI wrappers to compensate for Rust behavior differences; wrappers should
only reflect already-correct core behavior through the ABI surface.

## Harness And Fixtures

- Fixture generators are part of the system. Add or update documented
  generators under `build/fontdone-src/scripts/` or `build/fontdone-src/doc/`
  only in a separately authorized fontdone task.
- Do not create one-off scripts that future agents cannot reproduce.
- Prefer exact comparisons: pixel bytes for masks, bytes/hashes for bitmap
  output, exact 26.6 values for metrics and geometry.
- When a lane is incomplete, make the failure count visible and classify it.

## Parity Documentation

Every discovered implementation nuance must be preserved for future agents.

- When a fix depends on subtle C behavior, add a short code comment at the
  implementation site with the C function/file area and the reason.
- If a finding affects future debugging strategy, fixture generation, or
  harness expectations, update the relevant project notes or skill docs.
- Commit messages must include the first divergence, the C behavior, the Rust
  behavior before the fix, and the exact lane count impact.
- Do not leave knowledge only in chat, temporary traces, or one-off scripts.

## Subagents

Subagents are isolated workers for classified failure buckets only.

- They must use separate worktrees and branches.
- They must never edit `/home/appunni/work/pil-wasm` directly.
- They must receive the exact worktree path, branch, baseline counts, Makefile
  lane target, and constraints.
- They must not push main.
- They must commit only verified improvements and report changed files,
  before/after counts, verification commands, and remaining bucket.
- They must document any newly discovered implementation nuance in code at the
  relevant site, and call it out in their final report. If the nuance is broad
  enough to guide future workers, they must update the appropriate project
  note or skill doc instead of leaving it only in the report.
- The orchestrating agent reviews and merges into main, then runs the relevant
  lane, full harness, no-runtime-FFI, fmt, and clippy checks.
- Archive or remove completed worktrees. Do not report archived trees as
  active subagents.

Detailed subagent protocol lives in `.claude/skills/freetype-parity`.

## Build And Test

All normal workflows must go through `make`.

- Run `make help` first when you do not know the target name.
- Do not paste raw `cargo`, `python`, `node`, `wasm-pack`, or shell script
  commands for routine build, test, lint, fixture, benchmark, or CI work.
- If a repeated workflow has no target, add or extend a Makefile target in the
  same change and document it here. A manual command that is not documented is
  not a maintained workflow.
- One-off diagnostic commands are allowed only for investigation. If they become
  useful twice, promote them to a Makefile target.

Common root targets:

```bash
make help
make setup
make build
make build-dev
make build-wasm-release
make test
make test-wasm
make test-all
make migration-parity-test
make migration-parity-test-all-backends
make image-backend-parity-test
make fixtures
make fixture-coverage-check
make pillow-rs-fixtures-check
make fmt
make fmt-fix
make clippy
make lint
make repo-map-check
make repo-map-update
make ci
make verify
```

`make pillow-rs-fixtures-check` regenerates the current imagingft fixtures in a
temporary directory and requires exact JSON and raw-byte equality. The
crate-local equivalent is `make -C pillow-rs fixtures-check`.

fontdone / FreeType parity targets:

```bash
make fontdone-help
make fontdone-ci
make fontdone-test
make fontdone-parity
make fontdone-ffi
make fontdone-ffi-compat
make fontdone-doc
make fontdone-doc-test
make fontdone-lint
make fontdone-bench
make fontdone-bench-quick
make fontdone-fixtures
```

`make fontdone-ffi` is the no-runtime-native-FFI guard for `fontdone` core.
`make fontdone-ffi-compat` is the public API/ABI compatibility gate; it runs
the generated FreeType C surface audit and verifies manifest/input coverage
plus thin C/WASM ABI exports. These gates are not parity substitutes; they keep
the harness and wrapper boundaries honest while parity failures remain visible.

For narrow FreeType lanes, prefer the crate-local Makefile targets:

```bash
make fontdone-source
make -C build/fontdone-src test-harness
make -C build/fontdone-src test-generator
make -C build/fontdone-src test-render-mode
make -C build/fontdone-src test-fixed
make -C build/fontdone-src test-interface
make -C build/fontdone-src test-ffi
make -C build/fontdone-src test-ffi-compat
make -C build/fontdone-src test-perf
```

Run the narrow failing Makefile target first, then the broader harness target.
If a full workspace clippy failure is unrelated, report it clearly and keep the
touched package clean.

## Manifest And Coverage

All new public PIL-style operations start from `manifest.yaml`.

1. Add the manifest entry.
2. Generate stubs with the project script.
3. Implement in core.
4. Add binding delegation.
5. Add parity fixture and coverage map entry.
6. Verify with the fixture parity tests and coverage report.

Coverage is trust-based: a function is trusted only when at least one PIL
parity test passes. Signature-only tests do not count.

## Reporting

Final reports should include:

- Commit or working tree status.
- Files changed and why.
- Test commands and results.
- Current parity counts when working on `fontdone`.
- Remaining risks or failures, classified by bucket.

Keep status reports factual. If no subagents are active, say so directly.
