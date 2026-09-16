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
