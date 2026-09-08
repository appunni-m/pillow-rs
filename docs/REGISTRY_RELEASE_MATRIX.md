# Registry release matrix

This file is the runbook for the first public package releases. It records
which artifact is published where, the dependency order, and the evidence
required before a publish job is enabled. A registry upload is permanent, so
the version and package archive must be reviewed from the exact clean commit
that will be tagged.

## Current release graph

The exact Cargo, PyPI, and root npm versions below are the first-release
candidates. The sibling release commits are committed and locally verified;
registry publication and the exact-commit CI runs remain external prerequisites.

| Project | Artifact | Version | Registry | Prerequisite/order | Current state |
| --- | --- | --- | --- | --- | --- |
| image-slash-star | `image-slash-star` | `0.1.0` | crates.io | independent; before `pillow-rs` | release candidate `d390b54855aa53fc6bd33f7b8e649cf48d9625c3`; local verification passes, while the exact 13-path package-surface manifest and push-safe history for the oversized AV1 oracle remain open |
| fontdone | `fontdone` | `2.14.3-alpha.1` | crates.io | root crate before facades; before `pillow-rs` | release commit `727dd5aea448d3a6a6130fff231ad96f41d20128`; local parity/package/npm gates pass; cross-platform contract and reviewed benchmark thresholds remain external gates |
| fontdone | `fontdone-c-abi` | `2.14.3-alpha.1` | crates.io | after `fontdone` is visible | workspace facade; publish only from the same release commit |
| fontdone | `fontdone-wasm` | `2.14.3-alpha.1` | crates.io | after `fontdone` is visible | workspace facade; publish only from the same release commit |
| fontdone | `fontdone` | `2.14.3-alpha.1` | npm | already visible; verify metadata before the crate release | existing package metadata points to `appunni-m/fontdone` and uses the `next` tag |
| pillow-rs | `pillow-rs` | `0.1.0` | crates.io | after image-slash-star and fontdone versions are visible | manifest pins the committed sibling revisions; `RELEASE_CRATES_READY=1` enables the package gate after registry visibility |
| pillow-rs | `pillow-rs` | `0.1.0` | PyPI | after the Rust dependency gate | maturin builds an `abi3-py38` wheel |
| pillow-rs | `pillow-rs` | `0.1.0` | npm | after the Rust dependency gate | package is built from `pillow-rs-js` with Node 22.14.0/npm 11.5.1 |

The root workflow accepts a guarded manual dispatch for the first publication.
After that bootstrap, pushing an annotated `v<version>` tag runs the same
preflight, publishes only missing registry versions, and creates the
immutable GitHub release for that tag.

The root release workflow checks that Cargo, PyPI, and npm all report the same
version before it creates an artifact. It is manual and defaults to a
non-publishing preflight. The publish input is intentionally separate from
the dependency input, so a preflight can be reviewed before any registry
write.

## Local credential setup

Install the versions committed by the repository before authenticating:

```sh
rustup toolchain install 1.96.1 --profile minimal --component rustfmt --component clippy
rustup override set 1.96.1
RUSTC_WRAPPER= cargo --version
node --version                 # release workflow uses Node 22.14.0
npm --version                  # release workflow uses npm 11.5.1
```

Use short-lived credentials where the registry supports them. `cargo login`
stores a crates.io token in the local Cargo credentials file; run it
interactively and never place the token in shell history, a commit, or a CI
log. `npm login` should use the account or organization that owns the package,
with two-factor authentication enabled. PyPI uploads should use an API token
through keyring or an environment variable consumed by the upload tool; never
paste that token into a command recorded in the repository.

The first crates.io upload must be manual because a trusted publisher cannot
create a package name that does not yet exist. After the first successful
upload, configure the repository/workflow as a trusted publisher on crates.io,
PyPI, and npm, then use the guarded GitHub workflow for subsequent releases.

## Local preflight and publish order

Run each command from a clean, tagged release commit. The root Makefile refuses
the registry-ready path if `git status --porcelain` is non-empty.

```sh
RUSTC_WRAPPER= make docs-check
RUSTC_WRAPPER= make migration-parity-evidence-check
RUSTC_WRAPPER= make fmt
RUSTC_WRAPPER= make clippy
RUSTC_WRAPPER= make migration-parity-test
RUSTC_WRAPPER= make migration-parity-coverage-rust
RUSTC_WRAPPER= MIGRATION_WASM_NO_OPT=1 make release-check RELEASE_CRATES_READY=0
```

Coverage MCP is an evidence reader, not a test runner. Query its gaps or
comparison only after the managed coverage command has produced a report whose
recorded source revision matches the release commit. A report made by another
revision must be remeasured before it is used as release evidence.

Publish the independent package first, then the font engine, then this
workspace:

1. In a clean `image-slash-star` release checkout, run its fixture, test,
   lint, package, and license checks. Publish `image-slash-star` with
   `cargo publish --locked` and verify the exact version with `cargo info`.
2. In a clean `fontdone` release checkout, run `make fontdone-ci` and the
   package audit. Publish `fontdone`, wait for crates.io indexing, then publish
   `fontdone-c-abi` and `fontdone-wasm`. Build and test `fontdone-wasm/npm`,
   then publish it with the intended `next` tag.
3. In this checkout, rerun `make release-check RELEASE_CRATES_READY=1` after
   the exact dependency versions are visible. Publish the root crate, then
   build/upload the PyPI wheel, then build/upload the npm tarball. Create the
   Git tag and GitHub release only after all three uploads succeed.

If an upload times out, query the registry for the exact version before
retrying. Registry versions are immutable; do not change a version merely to
work around an uncertain response.

## GitHub Actions and protected environments

`.github/workflows/release.yml` runs the same locked preflight on `main`,
uploads checksummed artifacts for review, and keeps each publish job behind
the manual `publish` input plus a protected environment. The publish jobs use
OIDC permissions only in the job that needs them. Configure these environment
names and trusted publishers before enabling publication:

| Environment | Registry | Required reviewer/configuration |
| --- | --- | --- |
| `crates-io` | crates.io | trusted publisher for this repository/workflow and package owner |
| `pypi` | PyPI | trusted publisher matching the exact repository, workflow, and environment |
| `npm` | npm | trusted publisher/provenance enabled for the package owner |

Keep `contents: read` at workflow scope. The tag/release job alone receives
`contents: write`; no registry token is stored in repository secrets.

## Sibling project follow-up

The sibling release files need one clean release commit before they can be
published. `fontdone` and `image-slash-star` now both have tag-driven release
workflows with pinned toolchains, successful-CI checks, checksummed archives,
and registry trusted-publisher jobs. Fontdone still needs the five target
bundles and an owner decision on benchmark budgets. Image-slash-star still
needs its exact Cargo package-surface manifest update and a push-safe history
for the oversized generated AV1 oracle; those are release blockers rather than
reasons to weaken the package or parity checks.

Do not stage or discard those active sibling changes from this checkout. The
release audit must be repeated against the final release commits, with package
lists and checksums retained as artifacts.

## Official registry references

- [Cargo package publishing](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [PyPI Trusted Publishers](https://docs.pypi.org/trusted-publishers/)
- [npm provenance and trusted publishing](https://docs.npmjs.com/generating-provenance-statements/)
