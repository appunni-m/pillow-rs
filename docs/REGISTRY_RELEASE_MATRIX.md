# Registry release matrix

This file is the runbook for the first public package releases. It records
which artifact is published where, the dependency order, and the evidence
required before a publish job is enabled. A registry upload is permanent, so
the version and package archive must be reviewed from the exact clean commit
that will be tagged.

## Current release graph

The exact Cargo, PyPI, and root npm versions below are the first-release
candidates. The prepared pillow-rs release branch
`codex/benchmark-backend-parity-fixes` includes the CI diagnostic update
`da2ac3e3556f6bfb58ed0dc250532a479d7789f` and its receipt follow-up. Hosted
run `34626881477` was queued for that follow-up; the preceding run
`34623343821` was cancelled after Rust and documentation jobs passed, Python
3.10 recorded one parity mismatch, and Python 3.12 failed in the build step.
A clean local Python 3.10 replay of the same serial batch settings passes all
11,345 cases. Registry publication and an exact-commit hosted CI pass remain
external prerequisites.
The sibling release branches are recorded below.
The local-only bootstrap bundle is under `dist/release-local/` and is verified
separately from the tracked source tree.

The latest registry probes (2026-09-11) found `image-slash-star@0.1.0` and
`fontdone@2.14.3-alpha.3` absent from crates.io, `pillow-rs` absent from PyPI
and npm, and only `fontdone@2.14.3-alpha.1` visible on npm under both
`latest` and `next`. These probes are discovery evidence; they do not replace
the clean-checkout release gates or authorize a publication.

Fontdone has one public Cargo release unit: the root `fontdone` package. The
workspace members `fontdone-c-abi` and `fontdone-wasm` are private Cargo build
targets (`publish = false`) for the native C SDK and the raw WASM input to the
`fontdone` npm package. They are audited and archived where needed, but neither
is a second crates.io package.

The root Makefile's maintained `build/fontdone-src` parity checkout is pinned to
the same `5f17ad226d7c0a282fa0082316d7cdeb8ab12d9f` revision used by the
`pillow-rs` Cargo dependency and the public `fontdone` release. Root parity and
release evidence therefore exercise the exact fontdone source being packaged.

| Project | Artifact | Version | Registry | Prerequisite/order | Current state |
| --- | --- | --- | --- | --- | --- |
| image-slash-star | `image-slash-star` | `0.1.0` | crates.io | independent; before `pillow-rs` | local `v0.1.0` tag is `8d8ecdfe8699329ae166541be8040b1bc3b253e7`; pushed clean-history candidate `codex/release-image-slash-star-clean` is `892fa567c6fb1bef5d74d374fc7bf0b2e68191b5` with no historical blob at or above 100 MB and a passing `make package-verify`; `origin/main` remains at `8675feb31d274879aaec1d23cdb0458451eea1ea` until the coordinated history update is authorized; strict source coverage remains an explicit release blocker at 95,603/161,451 lines |
| fontdone | `fontdone` | `2.14.3-alpha.3` | crates.io | before `pillow-rs` | immutable local tag commit is `5f17ad226d7c0a282fa0082316d7cdeb8ab12d9f`; current pushed `main` is `857e96d05e32f6756a4322f02c089f501b6a913e` and contains the malformed-face parity fixes, refreshed release evidence, repository inventory, and the exact Cargo registry checksum guard. Because the immutable alpha.3 tag points to an earlier package tree, current `main` is a follow-on candidate rather than the exact alpha.3 package. The synchronized candidate has 20,355/20,355 runnable cases and 3 safety-extension cases pending; local docs, lint, parity, package, C-SDK, and npm checks pass. The generated C-ABI scorecard remains incomplete, and five fresh platform bundles including the Windows import library plus benchmark review are still required |
| fontdone | native C SDK archive | `2.14.3-alpha.3` | GitHub release asset | after the root crate preflight | built from the internal `fontdone-c-abi` workspace target; the tag workflow attaches a target-specific archive |
| fontdone | `fontdone` | `2.14.3-alpha.3` | npm | after the Cargo crate; before `pillow-rs` | synchronized local candidate built from the immutable `v2.14.3-alpha.3` tag; current `main` carries audit-only parity and ABI test-support corrections outside the npm package inputs. The earlier immutable `2.14.3-alpha.1` and superseded `2.14.3-alpha.2` artifacts remain historical evidence; raw `fontdone-wasm` remains an internal build target |
| pillow-rs | `pillow-rs` | `0.1.0` | crates.io | after image-slash-star and fontdone versions are visible | the local `v0.1.0` candidate is rebuilt from the synchronized alpha.3 dependency pin; its exact candidate commit, archive checksum, and staged registry entry are recorded in `dist/release-local/release-manifest.txt` and `SHA256SUMS`; public crate packaging still waits for the image-slash-star and fontdone registry versions |
| pillow-rs | `pillow-rs` | `0.1.0` | PyPI | after the Rust dependency gate | maturin builds an `abi3-py38` wheel |
| pillow-rs | `pillow-rs` | `0.1.0` | npm | after the Rust dependency gate | package is built from `pillow-rs-js` with Node 22.14.0/npm 11.5.1 |

The immutable local bundle retains a benchmark pair for release-candidate
source commit `031edd3f5425df2c6931939deb0ad0d93dad376c`, before the current
documentation updates. Both runs selected and measured the fixed
11-workload cohort, with 44/44 comparable rows and 33/33
terminal CPU/SIMD/GPU receipts; the unchanged five-percent comparison reports
nine timing-only violations. Earlier host-access series at `a5a678401` and
`de571ae57` remain historical evidence with adjacent comparisons reporting
3, 7, 5, 5, and 12 timing-only violations. The performance gate remains
review-needed; the local package rehearsal does not silently promote any
timing result to a release pass. Exact current result and budget hashes are in
the pending checklist.

A newer release-profile probe at source revision
`519226b62b545717c2169be7cbef9141754de91e` completed three identical
11-workload runs with 44/44 comparable rows and 33/33 terminal target receipts
per run. The adjacent comparisons report two and 16 timing-only violations;
the zero-violation gate therefore remains open. These generated results are
additional evidence and are not substituted for the immutable bundle's
tagged artifacts.

A fresh clean pair at current branch head
`13ea3dc177baa44a5d76ed7d86240aee53402ca8` again measured 11/11 workloads
with 44/44 comparable rows and 33/33 terminal requested=actual target receipts
per run. The unchanged budget report
`13f1649a545ebafbe17623931bd9a14e6e5a4eb2861f8871975a1b3f1c94551e` reports
12 timing-only violations; this is additional evidence and does not alter the
immutable local bundle or close the zero-violation gate.

A further foreground-scheduled cohort at the current clean commit measured
11/11 workloads in four runs with 44/44 comparable rows and 33/33 terminal
requested=actual receipts per run. Its first adjacent comparison had zero
violations; the next two had 10 and 10 timing-only violations. The unchanged
budget policy and execution fingerprints are retained, so the required
two-consecutive zero-violation gate remains open.

The current release-preparation commit was then measured in four complete
11-workload runs (runs 20–23). Each run retained 44/44 comparable rows and
33/33 terminal requested=actual CPU, SIMD, and Metal GPU receipts. The
run-21-vs-run-20 comparison reported nine timing-only violations and the
run-23-vs-run-22 comparison reported five. These receipts add no execution or
backend mismatch; the unchanged zero-violation gate remains open.

## Branch publication status

The clean `fontdone` `main` branch is now present on `origin` at
`857e96d05e32f6756a4322f02c089f501b6a913e`; its immutable local release tag
`v2.14.3-alpha.3` remains at `5f17ad226d7c0a282fa0082316d7cdeb8ab12d9f`, so the
tag and current main are intentionally distinct package revisions. The older `v2.14.3-alpha.2`
tag remains as superseded local history.
The clean pillow-rs release-preparation branch
`codex/benchmark-backend-parity-fixes` is present on `origin` with the
diagnostic commit `da2ac3e3556f6bfb58ed0dc250532a479d7789f` and subsequent
receipt updates; the earlier source-fix and implementation-base commits remain
historical receipt references.
`origin/main` remains at
`b4b91cb8a8bb85be861c0ea9e4f62dfbf7e8e0b9` pending review of the candidate
history. The current PyO3 and supply-chain fix is
`b76578c42c27cf7a8ccca8c576156a79b67d897a` and was built from the exact source
tree exercised by the full local CI run at `e1638ce7c11b270ac16540b25dc0fb5c1cb4a72a`:
all 11,345 parity cases passed on the CPU, SIMD, GPU, Node WASM, and browser
WASM lanes, and managed coverage completed all 24 plans with 11,325 passing
checks. The aggregate backend result still reports the intentional SIMD/GPU
host-control partition. The benchmark zero-violation item remains open.
The image-slash-star clean-history candidate is present on `origin` at
`892fa567c6fb1bef5d74d374fc7bf0b2e68191b5`; its local `v0.1.0` tag remains at
`8d8ecdfe8699329ae166541be8040b1bc3b253e7`. Its current tree matches the
release checkout and its package verification passes. `origin/main` still
contains the historical oversized AV1 blobs and is unchanged pending explicit
authorization for the coordinated history rewrite.

The root workflow accepts a guarded manual dispatch for the first publication.
After that bootstrap, pushing an annotated `v<version>` tag runs the same
preflight, publishes only missing registry versions, and creates the
immutable GitHub release for that tag.

The root release workflow checks that Cargo, PyPI, and npm all report the same
version before it creates an artifact. When a version already exists, the
publish jobs compare the crates.io SHA-256, PyPI wheel SHA-256, or npm
`dist.integrity` value with the exact local artifact before skipping it; a
mismatch fails the release. It is manual and defaults to a non-publishing
preflight. The publish input is intentionally separate from the dependency
input, so a preflight can be reviewed before any registry write.

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
