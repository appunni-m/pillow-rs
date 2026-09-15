# Registry release matrix

All subsequent publication uses GitHub OIDC. The local Cargo and npm
bootstraps are complete; local login errors do not diagnose the GitHub
publisher. The owner reports configuring trusted publishing in each registry.
Acceptance requires a successful publishing job and the exact registry artifact.

## Release order and package boundaries

| Order | Repository | Candidate | Registry artifacts | GitHub assets |
|---|---|---|---|---|
| 1 | `appunni-m/fontdone` | `2.14.3-alpha.9` | Cargo `fontdone`; npm `fontdone` under `next` | Native C SDK, crate, npm archive, checksums |
| 2 | `appunni-m/image-slash-star` | `0.1.1` | Cargo `image-slash-star` | Crate, checksums |
| 3 | `appunni-m/pillow-rs` | `0.1.2` | Cargo, PyPI, npm, all named `pillow-rs` | Crate, Python wheels/sdist, npm archive, checksums |

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

The 2026-09-15 audit found these repository defects before OIDC authentication:

| Repository | Failure before this release | Correction and acceptance |
|---|---|---|
| fontdone | Native C-width compilation, unspecified Linux cache fields, cross-linker/proc-macro lookup, Windows header paths, and static/DLL symbol parsing blocked successive candidates | Alpha.9 at `acb3e920` preserves native C widths, uses the canonical macOS oracle, repairs platform audit tooling, and requires all five native/QEMU contracts on main. Fresh local parity is 20,357/20,357 with three named undefined-C cases pending; native shared/static exports each match 218 declarations. Windows regressions distinguish mangled type names, public DLL names, and alias annotations while retaining detection of real undocumented exports. |
| image-slash-star | Every coverage test passed, then Python 3.14 rejected an unescaped percent sign in the verifier's CLI help | Candidate `c30253c9` fixes the CLI, adds a regression, pins Python 3.12.10 (available on all required runners), and retains failed CI evidence. Fresh local coverage passes all four existing alpha floors; [exact main CI](https://github.com/appunni-m/image-slash-star/actions/runs/34974261464) is green for quality, coverage, and dependencies. |
| pillow-rs | Unreleased dependencies, Windows FreeType integer-width compilation, self-including checksums, generic Linux wheels, and recovery without successful registry jobs blocked publication | Candidate 0.1.2 pins the corrected dependencies, converts native font integers at the Rust boundary, adds three generator-owned parity cases, and checks Windows compilation on main. It builds portable wheels and requires successful registry evidence before recovery. Actual OIDC acceptance is established only by the new tag run. |

Historical Cargo uploads without GitHub `trustpub_data` do not prove the newly
configured OIDC publisher is wrong. A skipped publish job has not attempted
authentication. Report a configuration blocker only when the actual OIDC job
rejects the claimed repository/workflow/environment.

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

A release does not convert planned API/codec/backend work into completed work.
See [coverage](COVERAGE.md) and [benchmark methodology](BENCHMARKING.md) for
measurement boundaries. No expected oracle results or performance budgets
were changed to claim a pass.

## Prepare, tag, verify

1. Synchronize package versions, changelog, exact dependency pins, and lockfiles.
2. Run the maintained Make checks and inspect the distributable archives.
   `make release-tools-test` exercises partial PyPI publication and rejects
   failed/skipped/unrelated recovery evidence.
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
