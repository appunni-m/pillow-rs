# Releasing pillow-rs

Version **0.1.3** is published on crates.io, PyPI, and npm. All subsequent
publication runs from this repository's pinned `release.yml` workflow using
GitHub OIDC. Local publishing targets refuse uploads.

| Artifact | Registry name | Public API |
| --- | --- | --- |
| Rust core | crates.io `pillow-rs` | `pillow_rs` |
| Python wheels and source distribution | PyPI `pillow-rs` | `PIL` |
| Shared Node/browser WASM package | npm `pillow-rs` | `pillow-rs` |

Publish new dependency versions from fontdone and image-slash-star first, then
update pillow-rs's exact dependency versions and lockfile. Fontdone has one
public Cargo crate; its C/WASM build members remain private. Image-slash-star
has no Python or npm distribution.

## Prepare a release

1. Update authoritative package versions, exact dependency pins, lockfiles,
   changelog, and the documented version.
2. Run the relevant full parity, coverage, portability, package-consumer, and
   supply-chain checks. Preserve failures and unmeasured scope.
3. From a clean checkout, run `make release-check RELEASE_CRATES_READY=1`.
   Inspect the crate, wheel/sdist, and packed npm contents.
4. Commit and push to main. Require successful CI for that exact commit.
5. Push a new annotated `v<version>` tag on the validated commit.

Tags and registry versions are immutable. Changed source or changed artifact
bytes require a new version. A retry is for the same source and artifacts,
not a way to replace a published package.

## Publication and verification

The workflow checks tag identity, builds artifacts before authentication,
validates checksums, and runs isolated consumers. Separate publishing jobs
use `id-token: write` and the configured environments:

| Registry | Workflow filename | Environment |
| --- | --- | --- |
| crates.io | `release.yml` | `crates-io` |
| PyPI | `release.yml` | `pypi` |
| npm | `release.yml` | `npm` |

No long-lived registry token is required. npm stable versions use `latest`;
prereleases use `next`. The final GitHub release requires all registry jobs.

Verify the downloaded registry artifacts against the GitHub checksum manifest.
Check registry provenance for the repository, workflow, and source identity.
Install a published wheel in a fresh environment outside the checkout. The
[release matrix](docs/REGISTRY_RELEASE_MATRIX.md) records the accepted versions
and workflow runs across the three projects.

```sh
make release-tools-test
make release-status RELEASE_STATUS_ARGS='--commit <full-source-sha>'
```

A skipped job did not authenticate. A completed workflow is not necessarily
successful. Diagnose the failing job before changing trusted-publisher settings.
The recovery helper validates the original source/run and required successful
jobs; it does not republish registry packages.

## Platform and compatibility boundaries

Release wheels cover Linux x86-64 (manylinux 2.28), macOS ARM64, and Windows
x86-64, plus an sdist for source builds. Native wheel consumers run before
publication. ABI metadata and tested Python versions are separate; see
[installation](docs/INSTALLATION.md).

Release acceptance does not complete every API, codec, GPU, or coverage goal.
The [maturity guide](docs/COMPATIBILITY.md) and
[coverage evidence](docs/COVERAGE.md) preserve those boundaries.

## Documentation deployment

The Documentation workflow publishes this repository's GitHub Pages site after
a checked main build. It is independent of package publication: documentation
changes do not require new registry versions. See the
[documentation checklist](docs/DOCUMENTATION_CHECKLIST.md).

## Registry references

- [crates.io authentication action](https://github.com/rust-lang/crates-io-auth-action)
- [PyPI trusted publishing](https://docs.pypi.org/trusted-publishers/using-a-publisher/)
- [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/)
