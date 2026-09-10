# Registry release matrix

This file is the runbook for the first public package releases. It records
which artifact is published where, the dependency order, and the evidence
required before a publish job is enabled. A registry upload is permanent, so
the version and package archive must be reviewed from the exact clean commit
that will be tagged.

## Current release graph

The exact Cargo, PyPI, and root npm versions below are the first-release
candidates. The sibling release branches are prepared and locally verified;
remote branch publication, registry publication, and the exact-commit hosted CI
runs remain external prerequisites. The local-only bootstrap bundle is recorded
under `dist/release-local/` and is verified separately from the tracked source
tree.

| Project | Artifact | Version | Registry | Prerequisite/order | Current state |
| --- | --- | --- | --- | --- | --- |
| image-slash-star | `image-slash-star` | `0.1.0` | crates.io | independent; before `pillow-rs` | release tag commit `35dd72808e6b2a8488b98caf685a3d48e4c97468`; the pushed clean-history candidate `codex/release-image-slash-star-clean` is `eb3fc54ca4bb2f78fce9cc1c9f1f60692832f818` with the identical current tree, no historical blob at or above 100 MB, and a passing `make package-verify`; `origin/main` remains at `8675feb31d274879aaec1d23cdb0458451eea1ea` until the coordinated history update is authorized; strict source coverage remains an explicit release blocker at 95,603/161,451 lines |
| fontdone | `fontdone` | `2.14.3-alpha.2` | crates.io | before `pillow-rs` | local tag commit `ac6e7b8e9300663afca809ad10423ab441e13798`; `main` has source-matched 20,354/20,354 runnable parity, complete `ci-fast`, package, C SDK, and npm gates; full release verification still has unresolved C-ABI route/error debt (C01.1 182/218, C01.7 13,572/18,098, C08.2 2,756/3,745, C08.3 15,829/20,355), five-platform contract bundles including the Windows import library, and benchmark review |
| fontdone | native C SDK archive | `2.14.3-alpha.2` | GitHub release asset | after the root crate preflight | built from the internal `fontdone-c-abi` workspace target; the tag workflow attaches a target-specific archive |
| fontdone | `fontdone` | `2.14.3-alpha.2` | npm | after the Cargo crate; before `pillow-rs` | synchronized local candidate built from the same `main` commit; the earlier immutable `2.14.3-alpha.1` npm artifact remains historical evidence; raw `fontdone-wasm` remains an internal build target |
| pillow-rs | `pillow-rs` | `0.1.0` | crates.io | after image-slash-star and fontdone versions are visible | local tag `v0.1.0` at `7d39bb7da7d4fdc5ece66d6a973f32760c1392ad`; package candidate `d0cedc8c47bbe3a437aa3e6d051a54d612aa0b9b2345856f9109d62d45cc1520` is built from that exact tag, uses the matching tagged image-slash-star and fontdone archives in the staged source registry, and passes documentation/input checks, all-target Clippy, external-artifact benchmark-report regression checks, and offline Cargo/Python/Node consumer checks; public crate packaging still waits for the image-slash-star and fontdone registry versions |
| pillow-rs | `pillow-rs` | `0.1.0` | PyPI | after the Rust dependency gate | maturin builds an `abi3-py38` wheel |
| pillow-rs | `pillow-rs` | `0.1.0` | npm | after the Rust dependency gate | package is built from `pillow-rs-js` with Node 22.14.0/npm 11.5.1 |

The immutable local bundle retains a tagged benchmark receipt pair for the
fixed 11-workload cohort with 44/44 comparable rows and 33/33 terminal
CPU/SIMD/GPU receipts. Its unchanged five-percent comparison reports nine
timing-only violations. A newer host-access run at the current root revision
has five complete runs with identical execution structure and adjacent
comparisons reporting 3, 7, 5, and 5 timing-only violations. The performance
gate remains review-needed; the local package rehearsal does not silently
promote either result to a release pass. A further two-run replay at current
HEAD `de571ae57` retained 11/11 measured workloads, 44/44 comparable rows, and
33/33 terminal requested=actual receipts in each run; its unchanged comparison
reported 12 timing-only violations. The exact result and budget hashes are in
the pending checklist.

## Branch publication status

The clean `fontdone` `main` branch is now present on `origin` at
`ac6e7b8e9300663afca809ad10423ab441e13798`. The pillow-rs release-prep branch
`codex/benchmark-backend-parity-fixes` is present on `origin` with the
documented release-preparation commits pushed.
The image-slash-star clean-history candidate is present on `origin` at
`eb3fc54ca4bb2f78fce9cc1c9f1f60692832f818`; its current tree matches the
release checkout and its package verification passes. `origin/main` still
contains the historical oversized AV1 blobs and is unchanged pending explicit
authorization for the coordinated history rewrite.

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
PYTHON_COMPAT=python3 make python-compat-check
```

Coverage MCP is an evidence reader, not a test runner. Query its gaps or
comparison only after the managed coverage command has produced a report whose
recorded source revision matches the release commit. A report made by another
revision must be remeasured before it is used as release evidence.

The local first-release rehearsal is already assembled under
`dist/release-local/`. It stages and verifies the packages in dependency order:
`image-slash-star`, the public `fontdone` crate, the native fontdone C SDK,
`fontdone` npm package, and finally the registry-normalized `pillow-rs`
archive. Internal C and raw-WASM Cargo archives are retained only as package
audit evidence. The same bundle contains the `pillow-rs` ABI3 wheel, the
`pillow-rs` npm tarball, complete Git bundles for the three annotated tags, and
checksum-bound offline consumer checks. This file-backed rehearsal does not
contact or mutate crates.io, PyPI, npm, GitHub, or any remote Git repository.

Run `make release-local-check` from the root checkout to revalidate the
artifact list, checksum manifests, benchmark receipts, and complete Git
histories after copying or rebuilding local release outputs. The check also
rejects `fontdone-c-abi` and `fontdone-wasm` if either internal workspace
package appears
in the staged public Cargo registry.

Publish the independent package first, then the font engine, then this
workspace:

1. In a clean `image-slash-star` release checkout, run its fixture, test,
   lint, package, and license checks. Publish `image-slash-star` with
   `cargo publish --locked` and verify the exact version with `cargo info`.
2. In a clean `fontdone` release checkout, run `make fontdone-ci` and the
   package audit. Publish the single `fontdone` crate, build the native C SDK
   archive, and build/test `fontdone-wasm/npm`; publish a new npm package
   version with the intended `next` tag. The workflow compares an already
   visible version's extracted files with the reviewed archive and stops on a
   mismatch. Attach the C SDK archive to the GitHub release.
3. In this checkout, rerun `make release-check RELEASE_CRATES_READY=1` after
   the exact dependency versions are visible. Publish the root crate, then
   build/upload the PyPI wheel, then build/upload the npm tarball. Create the
   Git tag and GitHub release only after the crate, PyPI, and npm uploads
   succeed; attach the native C SDK archive to the GitHub release.

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

The sibling release files now have clean release branches with tag-driven
workflows, pinned toolchains, successful local checks, checksummed archives,
and registry trusted-publisher jobs. Remote branch publication and hosted CI
remain external prerequisites. Fontdone still needs its unresolved C-ABI
route/error debt closed, the five target bundles including Windows import
library evidence, and an owner decision on benchmark budgets. Image-slash-star
still needs its strict source-coverage gate to pass before a registry upload.
These are release blockers rather than reasons to weaken the package or parity
checks.

Do not stage or discard those active sibling changes from this checkout. The
release audit must be repeated against the final release commits, with package
lists and checksums retained as artifacts.

## Official registry references

- [Cargo package publishing](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [PyPI Trusted Publishers](https://docs.pypi.org/trusted-publishers/)
- [npm provenance and trusted publishing](https://docs.npmjs.com/generating-provenance-statements/)
