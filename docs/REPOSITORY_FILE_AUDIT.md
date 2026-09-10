# Repository file audit

Reviewed 2026-09-11 from `git ls-files`, the active manifest, the Makefile, and
the repository map. The repository contains one active Rust workspace and one
read-only FreeType/parity archive.

## Inventory

| Area | Tracked items | Policy |
| --- | ---: | --- |
| Root control and package files | 16 | Keep active; versions and commands are authoritative |
| `pillow-rs/` core crate | 412 | Keep active pure-Rust implementation and tests |
| `pillow-rs-py/` | 21 | Keep active thin PyO3 package and wrappers |
| `pillow-rs-js/` | 18 | Keep active wasm-bindgen package, package metadata, legal text, and package lock |
| `scripts/` | 40 | Keep active maintained generators, runners, validators, and reports |
| `docs/` | 116 | Keep current landing/evidence pages; classify dated reports as historical |
| `deprecated/migration-parity-v0/` | 328 | Retain read-only provenance until mapped live coverage replaces it |
| `deprecated/imagingft/` | 12 | Retain historical ImagingFT fixtures and migration mapping |

The counts are a review snapshot, not a test denominator. `build/`, `target/`,
virtual environments, package installs, nested worktrees, and caches are
ignored local state and are excluded from releases.

## Classification

### Active

The active surface is `README.md`, `CONTRIBUTING.md`, `CLAUDE.md`, `AGENTS.md`,
`Makefile`, the workspace/package manifests and lock files, `.github/workflows`,
`scripts/`, `pillow-rs/`, `pillow-rs-py/`, `pillow-rs-js/`,
`pillow-rs/tests/fixtures/`, and the current landing/evidence pages listed in
[`docs/DOCUMENTATION_CHECKLIST.md`](DOCUMENTATION_CHECKLIST.md).

### Generated

`docs/generated/` is written by `scripts/generate_migration_parity_docs.py`.
Parity, coverage, benchmark, and status JSON under `build/migration-parity/`
are run evidence. Neither category is an input source, and generated files are
updated only through their Make targets.

### Historical

`CODEBASE_AUDIT.md`, `SYSTEMIC_FIXES.md`, dated audit/roadmap reports under
`docs/`, and `docs/superpowers/` preserve design decisions and evidence from
earlier phases. They are useful for archaeology, but active implementation
guidance comes from `CLAUDE.md`, `Makefile`, `docs/REPO_MAP.md`, and the active
manifest. New findings belong in a current page or the pending checklist.

### Deprecated archive

`deprecated/migration-parity-v0/` and `deprecated/imagingft/` are deliberately
kept. Their READMEs and `migration-map.yaml` identify the active replacement.
Deleting them before equivalent input-only parity and managed coverage exists
would destroy provenance and make regressions harder to diagnose.

## Removed or quarantined debris

- The three tracked `rustc-ice-*.txt` compiler crash dumps were build
  diagnostics, not source or reproducible fixtures, and are removed from the
  active tree.
- `.DS_Store` is operating-system metadata. It is ignored and must not be
  staged; an existing local copy is preserved for the working tree owner.
- The ignored `.worktrees/`, `.oracle-venv/`, `.venv/`, `build/`, and `target/`
  trees are never release inputs. Do not clean them with a broad destructive
  command while another task may be using them.

## Review procedure

Run these commands before moving or deleting a candidate:

```sh
git ls-files
git ls-files 'deprecated/**'
rg -n 'deprecated/|build/|target/|BENCHMARKS\\.md|docs/COVERAGE\\.md' \
  --glob '!deprecated/**' --glob '!build/**' --glob '!target/**' .
make repo-map-check
```

Before deleting a candidate, prove that it is not referenced by a maintained
Make target, manifest index, package manifest, generator, or migration map. If
it contains an old oracle output or fixture, map it first and leave the archive
until the replacement evidence is complete.
