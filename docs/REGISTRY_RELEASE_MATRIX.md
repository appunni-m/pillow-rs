# Historical artifacts and release evidence

The three projects publish from their own GitHub repositories through OIDC.
The observations below describe earlier releases verified on 2026-09-16.
They are retained for audit and do not identify the newest installable versions.
For current packages, use the release lists for
[pillow-rs](https://github.com/appunni-m/pillow-rs/releases),
[fontdone](https://github.com/appunni-m/fontdone/releases), and
[image-slash-star](https://github.com/appunni-m/image-slash-star/releases).

| Project | Accepted version | Registries | Release evidence |
| --- | --- | --- | --- |
| fontdone | 2.14.3-alpha.10 | One Cargo crate; one npm package; native C SDK on GitHub | [Release run](https://github.com/appunni-m/fontdone/actions/runs/35002120087), [tag CI](https://github.com/appunni-m/fontdone/actions/runs/35002119892) |
| image-slash-star | 0.1.2 | Cargo only | [Release run](https://github.com/appunni-m/image-slash-star/actions/runs/35010129246), [main CI](https://github.com/appunni-m/image-slash-star/actions/runs/35007807946) |
| pillow-rs | 0.1.3 | Cargo, PyPI, npm | [Release run](https://github.com/appunni-m/pillow-rs/actions/runs/35017008075), [main CI](https://github.com/appunni-m/pillow-rs/actions/runs/35014896711) |

Fontdone's C and raw WASM workspace packages are private Cargo build members.
Pillow-rs's npm package serves both Node and browsers; its Python import is
`from PIL import Image`. Compare it with Pillow in separate environments.

## Artifact identity

All six pillow-rs registry artifacts were downloaded and matched to the
[GitHub release checksum manifest](https://github.com/appunni-m/pillow-rs/releases/tag/v0.1.3).
Cargo and npm provenance identify commit
`fd78eb80402a0d6d99e6b696d0a1ebf9ba11d5fb` and the accepted release run.
[PyPI provenance](https://pypi.org/integrity/pillow-rs/0.1.3/pillow_rs-0.1.3.tar.gz/provenance)
identifies the repository's `release.yml` publisher and `pypi` environment.
This verifies published metadata and artifact digests; it is not an independent
signature-chain audit.

Fontdone Cargo/npm provenance identifies
`cb90d41a863f8569335d8ed775a4038d23c79dc5` and its release run.
Image-slash-star's registry/GitHub crate SHA-256 is
`e53037e57d0c5cae052ba94851c8cf72a80b9dfe195cd21b166506ab7bdbeb3b`.

## Acceptance limits

Pillow-rs's four runtime lanes each passed 11,348 cases at the released commit.
Its source-bound coverage has 24 plans and 11,328 target executions with zero
failures; all 25 measured changed Rust lines are covered. See
[coverage](COVERAGE.md) for the denominator and digest.

Fontdone remains alpha: runnable parity is 20,357/20,357, three undefined-C
inputs remain named pending cases, C-contract adoption is incomplete, and
benchmark thresholds are not yet accepted.

Image-slash-star's release floors are 59% lines, 46% branches, 52% functions,
and 58% regions. Its 2026-09-15 all-feature report records 95,473/161,451 lines,
14,912/32,258 branches, 4,859/9,244 functions, and 140,609/241,503 regions.
These floors do not imply complete source coverage; its complete-coverage
target retains the full denominator.

The [release process](../RELEASING.md) follows the same separation of validation,
artifact identity, and OIDC publishing used by
[coverage-mcp's reference release](https://github.com/appunni-m/coverage-mcp/actions/runs/33979588689).

## Alpha.6 partial publication, 2026-10-05

**Alpha.6 is not an accepted release.** The annotated `v12.2.0-alpha.6` tag
identifies `a0c7ffc45996ddd8a5e287fb78125d2a25fd2630`.
[Main CI](https://github.com/appunni-m/pillow-rs/actions/runs/37297480408)
passed all nine jobs on that exact commit. Python 3.10, Python 3.12, Node WASM,
and browser WASM each passed 17,105 live Pillow comparisons. The separate
[local evidence](GPU_BATCH_VALIDATION.md) records CPU, SIMD, GPU, Parallel CPU,
and explicit batch results; those source results do not prove the registry
crate's runtime behavior.

The [release run](https://github.com/appunni-m/pillow-rs/actions/runs/37301043651)
failed in the PyPI publishing step. Crates.io and npm jobs succeeded; the
GitHub release job was skipped. Registry checks found three standard Python
wheels, no Python sdist, and no same-version `pillow-rs-parallel` distribution.
The public annotation only states that attestations were being generated and
uploaded. The exact error is unavailable here: the signed-out browser cannot
expand logs, and the GitHub job-log API returned HTTP 403, requiring repository
admin rights. No publisher configuration change or retry has been justified by
that evidence.

The published Rust crate's SHA-256 is
`262a20e0a3df9734224c81d474f7bb999bf69ea88caa2841207f91f558a53b72`.
Its embedded `.cargo_vcs_info.json` matches the tag, but Cargo's normalized
manifest selects the registry `fontdone = "=2.14.3-alpha.12"`. That dependency's
downloaded archive embeds VCS revision
`fccab6837ccc959f49a7a5d13d058466fbf204a0`; this is archive metadata, not
independent provenance. The source build selects Git revision
`e2ff6ede3246b1568a087ef3bcf3dd848692d4bb`. The latter contains
later malformed-font error and cached-face teardown fixes. The charmap
registration fix is present in both revisions.

Public `FreeTypeFont::from_bytes` probes using `0xff` bytes returned:

| Input length | Live Pillow 12.2.0 / pinned source | Published Rust crate |
| ---: | --- | --- |
| 1 | `invalid stream operation` | `invalid stream operation` |
| 16 | `invalid stream operation` | `unknown file format` |
| 17 | `broken file` | `unknown file format` |
| 32 | `broken file` | `unknown file format` |

Loading the embedded default font on a Rust thread then exiting aborted the
published-crate process during TLS destruction (shell exit 133 on macOS).
The same public probe passed against the pinned source. Do not recommend the
alpha.6 Rust crate or install its unavailable Parallel CPU extra.

Recovery requires a new fontdone registry version containing the fixes already
on its remote main, followed by consistent exact Git/registry dependency pins
and a new pillow-rs version. The fontdone release workflow requires successful
tag CI, including coverage. No new fontdone tag or release was started, and no
existing gate was bypassed. Diagnose the PyPI failure with an authenticated
job log before another publication. Preserve the alpha.6 tag and distributed
artifacts; changed source or artifact bytes require a new version.

The recovery consumer in `scripts/check_release_crate.py` now exercises the
normalized archive before packaging acceptance and before upload authentication.
Its failure blocks another push/publication until the dependency is corrected.
This guard was added after the alpha.6 tag; it was not executed by that release.

Recovery checks ran locally after the failure:

| Command / scope | Result |
| --- | --- |
| `make docs-lint docs-test docs-build release-tools-test PYTHON=.venv/bin/python` | Passed; 44 documentation tests, 13 release-tool tests, strict site build |
| Open Source `audit_documentation.py --strict` | Zero errors; ten review prompts in existing skills, external environment files, and qualified compatibility/performance wording |
| `make repo-map-update repo-map-check PYTHON=.venv/bin/python` | Passed after adding the consumer file |
| `RUSTC_WRAPPER= .venv/bin/python scripts/check_release_crate.py --offline --archive /private/tmp/pillow-alpha6-published-crate/pillow-rs.crate` | Failed both public probes; three wrong font errors and TLS teardown abort |
| Same Rust probe source against the workspace Git dependency, separately invoked with `errors` and `thread` | Both passed |

The consumer disables core default features to isolate font behavior. These
recovery checks do not rerun the six image backend campaigns or establish
registry-crate GPU, SIMD, or Parallel CPU parity. No coverage was collected.
