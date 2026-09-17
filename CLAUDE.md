# pillow-rs agent guide

`AGENTS.md` and `AGENT.md` link to this file. Edit `CLAUDE.md` only.

## Scope and ownership

- `pillow-rs/` owns image algorithms and backend execution in Rust. Do not use
  native Pillow, FreeType, or codec libraries as runtime substitutes.
- `pillow-rs-py/` and `pillow-rs-js/` own host conversion, I/O, validation, and
  delegation. Keep image algorithms in core. The public Python namespace is
  `PIL`; `pillow_rs` contains internal binding modules.
- Draw in the image's native pixel format. Mode conversion must be explicit.
- fontdone and image-slash-star are separate repositories. Follow their own
  instructions when a task includes them. `build/fontdone-src/` may contain
  active standalone work; preserve it and do not reset it to `FONTDONE_REF`.
  Root `fontdone-*` targets require the configured pin; use the standalone
  checkout's Makefile for work on its current revision.
- Preserve unrelated changes and existing safety/lint checks. Keep library
  diagnostics in `log` macros; do not commit temporary prints or traces.

## Behavior and evidence

- Fix implementation failures without weakening assertions, thresholds, or the
  selected contract. Do not special-case fixture identities in runtime code.
- Update generator-owned inputs through their maintained generators when
  behavior changes. Keep active inputs free of expected outputs and run status.
  Start from [the public manifest](pillow-rs/tests/fixtures/manifest.yaml).
- Add cases for changed behavior and collect changed-line coverage for runtime
  fixes. Report parity, measured coverage, fallback, and unmeasured paths
  separately. Coverage receipts must match the measured source and inputs.
- For Pillow comparisons, use `make build-parity` and isolated processes or
  environments. `make build` installs the replacement `PIL` namespace and can
  overwrite the oracle in that environment.
- Record subtle reference behavior beside the implementation. Keep user guides
  separate from contributor procedures, and retain README acknowledgements.

## Verification and references

Use existing Makefile targets; `make help` and `make help-all` list them.
Direct commands are fine for focused diagnostics or a task without a suitable
target. Add a maintained target when introducing a reusable workflow.

Run checks relevant to the change. Documentation edits use `make docs-lint`;
site changes also use `make docs-test docs-build`. Rust changes use the focused
tests and `make fmt clippy`; behavior changes also need the affected parity
lanes. Run broader campaigns when the change affects their scope. Report
failures and checks that could not run; do not describe old results as fresh.

- [Contributing](CONTRIBUTING.md): setup and change workflow.
- [Command reference](docs/COMMANDS.md): targets, prerequisites, and side effects.
- [Architecture](docs/ARCHITECTURE.md) and [repository map](docs/REPO_MAP.md):
  ownership. Use `make repo-map-update repo-map-check` when files move, are
  added, or are removed.
- [Coverage](docs/COVERAGE.md) and [benchmarking](docs/BENCHMARKING.md):
  evidence collection and interpretation.
- [Releasing](RELEASING.md): version synchronization, package checks, and
  tag-triggered publishing.
