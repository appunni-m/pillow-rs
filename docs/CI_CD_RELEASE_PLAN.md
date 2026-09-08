# CI, CD, and release plan

This plan makes the current build reproducible and defines the gates for future
package publication and GitHub Pages sites. It does not publish credentials or
turn a benchmark into a release gate before the benchmark budget policy passes.

## Fixed inputs

| Input | Pinned location/value | Use |
| --- | --- | --- |
| Rust toolchain | `rust-toolchain.toml`, `1.96.1` | all workspace builds and checks |
| Cargo resolution | `Cargo.lock` with `--locked` CI commands | exact Rust dependency graph |
| Pillow oracle | manifest and CI, `12.2.0` | live parity and reverse coverage |
| Python parity runners | CI `3.10`, `3.12` | Pillow 12.2.0 supports these versions |
| Python wheel ABI | `abi3-py38`, `requires-python >=3.8` | package compatibility floor; parity oracle still runs on 3.10+ |
| Node.js | CI/release `22.14.0` | WASM package and browser runner |
| npm | CI/release `11.5.1` | locked package installation and publication |
| wasm-pack | CI/Make, `0.15.0` | WASM build |
| maturin | `requirements-ci.txt`, `1.14.1` | Python extension build |
| coverage.py | `requirements-ci.txt`, `7.10.7` | managed source coverage |
| NumPy | `requirements-ci.txt`, `2.2.6` | array input generation |
| PyYAML | `requirements-ci.txt`, `6.0.3` | manifest/input parsing |
| cargo-deny | CI, `0.20.2` | advisory, ban, license, and source checks |
| cargo-audit | CI, `0.22.2` | RustSec audit |
| Puppeteer | `pillow-rs-js/package.json` and lock, `22.15.0` | browser WASM checks |
| git dependencies | exact `rev` values in `pillow-rs/Cargo.toml` | `image-slash-star` and `fontdone` |

Cargo manifests retain compatible version ranges where the project is a
published library; CI and release packaging always use the committed lockfile
and `--locked`. Python and npm development inputs are exact versions in the
checked-in constraints/lock files.

## Pull-request pipeline

The required checks are ordered so cheap contract failures happen first:

1. repository map, documentation links/checklist, formatting, and public API
   boundary;
2. workspace clippy with the documented deprecation exception;
3. locked core build and supply-chain checks;
4. deterministic manifest/input validation;
5. Python parity and managed coverage on Python 3.10 and 3.12;
6. Node/browser WASM package build and parity; and
7. a bounded GPU smoke gate with explicit fallback accounting.

The full all-backend campaign is available through `make test`. It remains a
reviewable evidence job because GPU availability and browser WebGPU support are
runner properties. Every incomplete backend is reported in the result rather
than converted into a pass.

## Scheduled benchmark pipeline

Benchmarks should run on a stable, labeled runner from a manual or scheduled
workflow, never on every pull request. The workflow will:

- build with the pinned toolchain and lockfile;
- run the quick cohort as a smoke check and the standard cohort on schedule;
- upload immutable JSON, environment, manifest/input hashes, and Markdown
  summaries;
- compare only compatible baselines with the five-percent budget checker; and
- mark timing noise or budget violations as review-needed while keeping
  correctness failures blocking.

The repository now has a scheduled/manual workflow at
[`.github/workflows/benchmark.yml`](../.github/workflows/benchmark.yml). The
release preflight and guarded publication workflow is at
[`.github/workflows/release.yml`](../.github/workflows/release.yml). The
registry order, local credential setup, and sibling project follow-up are
recorded in [`REGISTRY_RELEASE_MATRIX.md`](REGISTRY_RELEASE_MATRIX.md). The
tag path also requires a successful `ci.yml` run for the exact tagged commit
before any registry job can publish. The Pages renderer remains the next
delivery step.
Pages renderer remains the next delivery step.

## Release sequence

1. Update the workspace version in `Cargo.toml`, then verify the Python and npm
   package versions match it.
2. Regenerate manifest-driven inputs and generated evidence.
3. Run `make release-check`, which builds all packages, checks package contents,
   and runs the locked metadata/build validations without publishing. Crate
   packaging remains explicitly deferred until the pinned `image-slash-star`
   and `fontdone` git dependencies have published registry versions; enable it
   only with `RELEASE_CRATES_READY=1` after that prerequisite is met.
4. Review parity, coverage, benchmark, security, and changelog evidence.
5. For the bootstrap, run the guarded manual release dispatch after the
   dependency crates are visible, then push an annotated `v<version>` tag.
   That tag creates the GitHub release. Every later release starts from a
   reviewed commit and uses the same pushed-tag path.
6. Publish each registry from the release workflow with protected environments:
   crates.io, PyPI, then npm. The root crate remains disabled until the
   pinned `image-slash-star` and `fontdone` versions are visible. Registry
   steps are idempotent for an already published bootstrap version; record
   artifact checksums and provenance.
7. Publish documentation and benchmark sites from the same tag; the site build
   consumes validated generated artifacts only.

Publishing targets require an explicit `RELEASE_CONFIRM=1` in local Make
invocations. CI publication should use short-lived OIDC credentials and a
protected environment; no token belongs in the repository.

## Dependency update policy

Update one tool or dependency family at a time, regenerate the corresponding
lock/constraints file, run the complete applicable Make lane, and record the
new version and evidence in the release notes. Workflow action references are
full commit SHAs with version comments; update both together and review the
release notes before changing them. GitHub's secure-use guidance recommends
full-length SHAs for immutable action versions; see
[GitHub's secure use reference](https://docs.github.com/en/actions/reference/security/secure-use).

## GitHub Pages layout

The planned Pages repository layout is:

```text
site/
├── docs/       # generated contract, API, and contributor pages
└── benchmark/  # validated parity, backend, timing, and budget views
```

The site is a presentation layer. Manifest/input JSON and run artifacts remain
the auditable source records in the release artifacts.
