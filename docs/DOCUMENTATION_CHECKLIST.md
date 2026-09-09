# Documentation checklist

This is the active documentation contract for pillow-rs. It was reviewed on
2026-09-08 against the pinned manifest, Makefile, package manifests, and the
latest recorded parity and benchmark artifacts.

## Audience paths

| Reader | Start here | Then read |
| --- | --- | --- |
| User of the Python package | [`README.md`](../README.md) | Pillow's API reference and the package release notes |
| User of the WASM package | [`README.md`](../README.md) | `pillow-rs-js/package.json`, the package tests, and the published package contents |
| Contributor | [`CONTRIBUTING.md`](../CONTRIBUTING.md) | [`docs/REPO_MAP.md`](REPO_MAP.md) and the relevant crate docs |
| Parity maintainer | [`pillow-rs/tests/fixtures/manifest.yaml`](../pillow-rs/tests/fixtures/manifest.yaml) | [`docs/benchmark-backend-pending-2026-09-03.md`](benchmark-backend-pending-2026-09-03.md) |
| Release maintainer | [`docs/CI_CD_RELEASE_PLAN.md`](CI_CD_RELEASE_PLAN.md) | [`docs/LOCAL_FIRST_RELEASE.md`](LOCAL_FIRST_RELEASE.md), [`docs/REGISTRY_RELEASE_MATRIX.md`](REGISTRY_RELEASE_MATRIX.md), and `Makefile` release-check targets |
| Benchmark maintainer | [`BENCHMARKS.md`](../BENCHMARKS.md) | [`docs/BENCHMARKING.md`](BENCHMARKING.md) and the JSON artifacts under `build/migration-parity/` |
| Agent or automation author | [`CLAUDE.md`](../CLAUDE.md) | [`docs/REPO_MAP.md`](REPO_MAP.md) |

## Source-of-truth order

1. `CLAUDE.md`/`AGENTS.md` defines repository constraints.
2. `Makefile` defines maintained commands and their dependency graph.
3. `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, Python packaging metadata,
   `pillow-rs-js/package.json`, and `pillow-rs-js/package-lock.json` define build
   and release inputs.
4. `pillow-rs/tests/fixtures/manifest.yaml` defines the public parity,
   coverage, and benchmark contract.
5. `pillow-rs/tests/fixtures/inputs/` contains input-only generated documents.
6. `build/migration-parity/` contains run evidence and is never an input.
7. Human-facing pages summarize the sources above and must link to the exact
   source or evidence they summarize.

## Claim labels

Every numeric or compatibility claim in maintained documentation uses one of
these labels, either in prose or in a nearby table column:

- **Measured** — backed by a named artifact, run ID, source hash, and command.
- **Declared** — present in the active manifest or package metadata; it is not
  evidence that a runtime path passed.
- **Planned** — a dated plan with an owner and an exit condition.
- **Not proven** — the input or implementation exists but compatible evidence
  is absent, stale, incomplete, or environment-dependent.
- **Historical** — retained for provenance and excluded from active guidance.

Do not use unqualified claims such as “complete API”, “all platforms”,
“production-ready”, or a speedup average without the scope and evidence that
make the claim reproducible.

## Per-change checklist

- [ ] Identify the source-of-truth file before editing a summary page.
- [ ] Update the manifest first for a new public operation or requirement.
- [ ] Add input-only parity and coverage cases in the indexed input files.
- [ ] Add benchmark coverage only for a successful, repeatable workflow.
- [ ] Add a code comment when a fix depends on subtle Pillow or FreeType
      behavior; record the first divergence and lane impact.
- [ ] Regenerate deterministic inputs and run
      `make migration-parity-inputs-check`.
- [ ] Run the narrow parity/coverage target, then the relevant all-backend
      target. Keep failures and backend fallbacks visible.
- [ ] Run `make repo-map-check`, `make fmt`, and the package-appropriate lint.
- [ ] Update the current status page and link to fresh evidence. Do not edit
      generated evidence by hand.
- [ ] Review local links, code-fence language labels, headings, version
      claims, and examples against the current API.
- [ ] Record changed files, commands, counts, and remaining gaps in the final
      report and commit message.

## Generated documentation

Run `make migration-parity-docs` after producing a compatible aggregate. The
generator writes:

- `docs/generated/migration-parity-public-contract.md` (declared contract),
- `docs/generated/migration-parity-status.md` (parity evidence),
- `docs/generated/migration-coverage-status.md` (coverage evidence), and
- `docs/generated/migration-benchmark-status.md` (benchmark evidence).

`docs/COVERAGE.md` and `BENCHMARKS.md` are maintained landing pages. They must
not repeat an older generated snapshot as if it were current.

## Archive and deletion checklist

- [ ] Search references with `rg` before moving or removing a file.
- [ ] Map an old fixture or harness to the active manifest and retain the
      mapping in `deprecated/migration-parity-v0/migration-map.yaml`.
- [ ] Keep expected outputs and oracle data in the archive until equivalent
      live input-only evidence exists.
- [ ] Remove only generated debris or files that are demonstrably outside the
      active source tree. Do not remove historical evidence to make an audit
      look clean.
- [ ] Run `make repo-map-update` followed by `make repo-map-check` after a
      source-tree move.

The current file classification is recorded in
[`docs/REPOSITORY_FILE_AUDIT.md`](REPOSITORY_FILE_AUDIT.md).

## Validation commands

```sh
make migration-parity-inputs-check
make migration-parity-fixtures-check
make migration-parity-evidence-check
make migration-parity-receipt-test migration-parity-coverage-receipt-test
make repo-map-check
make docs-check
make fmt
make clippy
make release-check
```

The Open Source documentation audit was run against the checkout. Its default
recursive mode also visits ignored worktrees, virtual environments, and build
trees, so its raw finding count is not an active-document denominator. The
tracked active pages above are the review scope; archived and generated pages
are classified rather than silently treated as current guidance.
