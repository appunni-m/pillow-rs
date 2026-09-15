# Registry release matrix

All subsequent publication uses GitHub OIDC. The local Cargo and npm
bootstraps are complete; local login errors do not diagnose the GitHub
publisher. The owner reports configuring trusted publishing in each registry.
Acceptance requires a successful publishing job and the exact registry artifact.

## Release order and package boundaries

| Order | Repository | Released version | Registry artifacts | GitHub assets |
|---|---|---|---|---|
| 1 | `appunni-m/fontdone` | `2.14.3-alpha.10` | Cargo `fontdone`; npm `fontdone` under `next` | Native C SDK, crate, npm archive, checksums |
| 2 | `appunni-m/image-slash-star` | `0.1.2` | Cargo `image-slash-star` | Crate, checksums |
| 3 | `appunni-m/pillow-rs` | `0.1.3` | Cargo, PyPI, npm, all named `pillow-rs` | Crate, Python wheels/sdist, npm archive, checksums |

Fontdone has exactly one public Cargo crate. Its C ABI and raw WASM workspace
members remain private Cargo packages. Image-slash-star has no npm or PyPI
package. Pillow-rs has one npm package serving Node.js and browser environments;
`pillow-rs-js` is the source folder. Its Python public import is
`from PIL import Image`, with `pillow_rs` as the implementation namespace and
`RSPIL` as a deprecated alias. Use isolated processes/environments for comparison
with upstream Pillow because both distributions own `PIL`.

Pillow-rs pins the exact dependency versions and Git revisions in
`pillow-rs/Cargo.toml`. `Cargo.lock` and `FONTDONE_REF` in the root Makefile must
match those reviewed releases. Do not tag pillow-rs until the new dependency
Cargo versions and synchronized fontdone npm version are registry-visible.

## Trusted publisher identities

Every registry uses workflow filename **`release.yml`** in its own repository.
Do not enter the full `.github/workflows/` path in the registry workflow field.

| Repository | crates.io environment | npm environment | PyPI environment |
|---|---|---|---|
| `appunni-m/fontdone` | `crates-io` | `npm` | Not applicable |
| `appunni-m/image-slash-star` | `crates-io` | Not applicable | Not applicable |
| `appunni-m/pillow-rs` | `crates-io` | `npm` | `pypi` |

Publish jobs run on GitHub-hosted runners with `id-token: write`. Crates.io uses
the pinned official authentication action. npm uses Node 22.14.0, npm 11.5.1,
and `--provenance`. PyPI uses the pinned official publishing action. No
long-lived registry secret is read by these workflows. If an environment has
required reviewers, GitHub pauses the job for that configured review.

## Working reference and diagnosed blockers

[coverage-mcp v0.16.0](https://github.com/appunni-m/coverage-mcp/actions/runs/33979588689)
is the reference: its release succeeded and crates.io records GitHub
`trustpub_data` tied to the exact repository/run/commit. The adopted pattern is
read-only validation, verified artifacts, a separate OIDC job, registry identity
checks, and a final GitHub release with attestations.

All three releases completed through GitHub OIDC, in the order above. The
2026-09-16 verification distinguishes repository defects from publisher authentication:

| Repository | Failure before this release | Correction and acceptance |
|---|---|---|
| fontdone | Earlier platform/oracle defects are fixed. Alpha.9 passed complete tag CI and published Cargo through OIDC, but npm interpreted its relative tarball path as a GitHub repository. | **Released:** [alpha.10 release](https://github.com/appunni-m/fontdone/actions/runs/35002120087) and [complete tag CI](https://github.com/appunni-m/fontdone/actions/runs/35002119892) passed. Cargo `trustpub_data` and npm provenance identify this exact run and commit `cb90d41a863f8569335d8ed775a4038d23c79dc5`; registry archives match the GitHub checksums. The absolute npm archive path is covered by an offline CLI dry-run. Local parity is 20,357/20,357, with three named undefined-C inputs pending; the external C audit passes 218 functions and 1,496 cases. Cross-host equality for unspecified SBit fields remains unproven. |
| image-slash-star | The earlier coverage CLI formatting defect is fixed. Version 0.1.1 uploaded through OIDC, but its verifier requested JSON and received a URL descriptor instead of the crate. | **Released:** [0.1.2 release](https://github.com/appunni-m/image-slash-star/actions/runs/35010129246) and [exact main CI](https://github.com/appunni-m/image-slash-star/actions/runs/35007807946) passed. The binary download request preserves strict checksums; the registry, candidate, and GitHub archive all have SHA-256 `e53037e57d0c5cae052ba94851c8cf72a80b9dfe195cd21b166506ab7bdbeb3b`. Eight release-tool tests cover negotiation, checksum rejection, authorization, and coverage guards. |
| pillow-rs | Dependencies, Windows integer widths, and packaging are fixed. Version 0.1.2 passed complete CI and wheel/source-package checks, then published Cargo and PyPI through OIDC. Its npm job failed before authentication because `cache: false` selected an unsupported package manager in setup-node. | **Released:** [0.1.3 release](https://github.com/appunni-m/pillow-rs/actions/runs/35017008075) passed all three publishers and created the GitHub release. [Exact main CI](https://github.com/appunni-m/pillow-rs/actions/runs/35014896711) passed 11,348 cases in each Python/Node/browser lane. Cargo and npm provenance identify commit `fd78eb80402a0d6d99e6b696d0a1ebf9ba11d5fb` and this release run; PyPI identifies the configured `release.yml` publisher. All six downloaded registry artifacts match the GitHub checksum manifest. The corrected `package-manager-cache: false` is covered by a regression that reproduced the original failure and now runs in `make docs-check`; all six release-tool tests pass. The 0.1.2 uploads and tag remain immutable. |

The published [pillow-rs 0.1.3 assets and checksums](https://github.com/appunni-m/pillow-rs/releases/tag/v0.1.3)
cover the crate, npm archive, three platform wheels, and source distribution.
The npm archive also matches its registry SHA-512 integrity. The
[PyPI source provenance](https://pypi.org/integrity/pillow-rs/0.1.3/pillow_rs-0.1.3.tar.gz/provenance)
identifies GitHub repository `appunni-m/pillow-rs`, workflow `release.yml`, and
environment `pypi`, with the exact source archive digest. The macOS wheel
downloaded from PyPI passed `make release-wheel-test` in a fresh environment
outside the checkout; Linux and Windows wheels passed their native installation
checks before publication. These checks compare public provenance records and
artifact digests; they do not claim an independent signature-chain verification.

Historical Cargo uploads without GitHub `trustpub_data` do not prove the newly
configured OIDC publisher is wrong. A skipped publish job has not attempted
authentication. Report a configuration blocker only when the actual OIDC job
rejects the claimed repository/workflow/environment.

For the pinned setup-node v5 action, `cache` accepts a package-manager name
(`npm`, `yarn`, or `pnpm`). Its separate `package-manager-cache` boolean disables
automatic detection. Passing `false` as the manager caused the 0.1.2 npm setup
failure. This follows the pinned
[action input contract](https://github.com/actions/setup-node/blob/a0853c24544627f65ddf259abe73b1d18a591444/action.yml).
The npm publisher has no source checkout or lockfile to cache; the corrected
configuration disables automatic caching and retains the same OIDC permissions.

The npm failure is distinct from trusted-publisher setup: npm 11.5.1 parses
`release-bundle/fontdone-<version>.tgz` as GitHub shorthand. Its parser treats an
absolute path or `./release-bundle/...` as a local file. A real offline dry-run
reproduced the Git lookup with the former argument and passed with the resolved
path. This follows npm's documented
[package specifier rules](https://docs.npmjs.com/cli/v11/using-npm/package-spec/).
Pillow-rs also uses an explicit local prefix and retains bounded npm failure
annotations. Fontdone alpha.10 and pillow-rs 0.1.3 each establish npm OIDC
acceptance through their own successful uploads and matching provenance.

Image-slash-star's archive download must send `Accept: application/octet-stream`.
With `application/json`, the crates.io download endpoint returns a JSON object
containing the archive URL; hashing that response correctly fails archive
verification. The binary request follows the redirect to the actual crate.
The 0.1.1 crate SHA-256 is
`de07cb0b4fc08e5e20130339ae17f0db2d09013815897ff7ac4114d587eba120`.
This was a post-upload verifier defect, not a rejected trusted publisher.

The crates.io metadata check also needs a descriptive HTTP `User-Agent` with
the repository contact URL. The local default-curl request returned HTTP 403;
the identified request returned HTTP 200 for the existing `pillow-rs/0.1.0`
version. The release workflow now identifies that request before checking
whether a version is absent or immutable. This follows the
[crates.io maintainer's explanation of generic-agent rejection](https://users.rust-lang.org/t/tor-i2p-yggdrasil-mycelium-proxy-for-crates-io/138527/3).

## Coverage and compatibility acceptance

The owner approved reduced coverage requirements for this alpha release:

- Fontdone reports measured source coverage and incomplete C-contract adoption.
  All runnable exact comparisons, consumers, five platform checks, package
  checks, and coverage collection must pass. Its stricter complete-release
  target retains full C-contract and benchmark-threshold requirements. The
  current benchmark policy is still collecting baselines and defines no budget;
  alpha CI retains measurements without claiming a budget pass.
- Image-slash-star enforces at least 59% lines, 46% branches, 52% functions,
  and 58% regions. The fresh 2026-09-15 full all-feature report is respectively
  95,473/161,451, 14,912/32,258, 4,859/9,244, and 140,609/241,503.
  No coverage sources are excluded to meet these floors, and `make coverage-complete` retains 100%.
- Pillow-rs retains its complete maintained Python/Node/browser parity lanes
  and source-bound Rust coverage collection. Incomplete coverage is reported;
  failing comparisons or invalid collection receipts still fail.
  The fresh 0.1.3 receipt at `fd78eb80402a0d6d99e6b696d0a1ebf9ba11d5fb` records
  24 plans, 11,328 target-only coverage executions, and zero failures. All 25
  measured changed Rust lines are covered; seven comment/attribute/blank lines
  have no line records. Version 0.1.3 does not change Rust implementation;
  the collection still ran again against the released commit. See the
  [coverage receipt](COVERAGE.md#013-release-verification) for retained paths
  and the LCOV digest.

A release does not convert planned API/codec/backend work into completed work.
See [coverage](COVERAGE.md) and [benchmark methodology](BENCHMARKING.md) for
measurement boundaries. No expected oracle results or performance budgets
were changed to claim a pass.

## Prepare, tag, verify

1. Synchronize package versions, changelog, exact dependency pins, and lockfiles.
2. Run the maintained Make checks and inspect the distributable archives.
   `make release-tools-test` validates Node action cache inputs, exercises partial
   PyPI publication, and rejects failed/skipped/unrelated recovery evidence. It
   also runs in the main CI documentation/input gate through `make docs-check`.
3. Commit and push to `main`; require successful CI for that exact commit.
4. Push a new annotated `v<version>` tag, preserving earlier immutable tags.
5. Let GitHub run the release. Finish fontdone, then image-slash-star, then
   pillow-rs. Keep the three repository workflows separate.
6. Verify the registry version and checksum/provenance, then install from the
   registry in a clean consumer. Check GitHub assets and checksums too.

Local `make release-crates`, `make release-pypi`, and `make release-npm` refuse
publication and direct the maintainer to GitHub. They do not consume locally
stored registry credentials.

## Artifact and retry rules

Pillow-rs builds ABI3 wheels for Linux x86-64 (manylinux 2.28), macOS ARM64,
and Windows x86-64, plus an sdist. Each wheel is installed in an isolated host
environment and exercises the public `PIL` namespace before it reaches PyPI.
Release hosts use Python 3.12.10, the last 3.12 patch with official macOS ARM64
and Windows builds in the GitHub Python manifest. Linux uses a pinned manylinux container digest; generic `linux_x86_64` wheels
are rejected. The source distribution requires the documented Rust build tools.

Packages are compiled/tested before OIDC authentication. Every publishing job
checks the complete bundle checksum; Cargo additionally reproduces the checked
crate. PyPI stages only absent files and rejects an existing filename with
different bytes. npm compares the exact tarball integrity. A failed lookup is
never interpreted as an absent version.

The final GitHub release requires all registry jobs. Asset recovery verifies
the original workflow, tag commit, run attempt, and successful verification and
publish jobs; a merely completed workflow is insufficient. It never republishes
a registry package. Retry transient failures on the same tag. Changed source
or changed immutable artifact bytes require a new version and tag.

## Official references

- [crates.io OIDC action](https://github.com/rust-lang/crates-io-auth-action)
- [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/)
- [PyPI trusted publishing](https://docs.pypi.org/trusted-publishers/using-a-publisher/)
- [Maturin distribution guidance](https://www.maturin.rs/distribution.html)
- [GitHub artifact attestations](https://docs.github.com/en/actions/how-tos/use-artifact-attestations)

Read exact GitHub conclusions without local registry credentials:

```bash
make release-status RELEASE_STATUS_ARGS='--repo appunni-m/fontdone --commit <full-sha>'
make release-status RELEASE_STATUS_ARGS='--repo appunni-m/image-slash-star --commit <full-sha>'
make release-status RELEASE_STATUS_ARGS='--commit <pillow-rs-full-sha>'
```

When the public API quota is exhausted, pass `--run-url` with the known GitHub
Actions run URL. A job duration alone never establishes that the job passed.
