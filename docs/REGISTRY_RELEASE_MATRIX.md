# Registry release matrix

<!-- release:summary -->
**Latest release: [12.2.3](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.3).**
<!-- /release:summary -->

## Current registry versions

Verified 2026-10-23. See the [GitHub release](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.3)
for release notes and artifact checksums.

| Registry | Package | Version | Published artifacts |
| --- | --- | --- | --- |
| crates.io | [`pillow-rs`](https://crates.io/crates/pillow-rs/12.2.3) | 12.2.3 | Rust crate |
| PyPI | [`pillow-rs`](https://pypi.org/project/pillow-rs/12.2.3/) | 12.2.3 | Source distribution and wheels for Linux x86-64, macOS ARM64, and Windows x86-64 |
| PyPI | [`pillow-rs-parallel`](https://pypi.org/project/pillow-rs-parallel/12.2.3/) | 12.2.3 | Companion wheels for Linux x86-64, macOS ARM64, and Windows x86-64 |
| npm | [`pillow-rs`](https://www.npmjs.com/package/pillow-rs/v/12.2.3) | 12.2.3 | Shared Node.js and browser WebAssembly package |

The standard Python wheel uses serial CPU. The `parallel` extra installs the
separately built Parallel CPU companion wheel; it does not change Cargo features
in an already installed wheel. See [installation](INSTALLATION.md) for package
selection and the supported-platform requirements.

The PyPI project page for `pillow-rs` still displays the README embedded in its
12.2.3 upload, which names 12.2.2 as current. The registry version and the
[installation command](INSTALLATION.md) above are 12.2.3. PyPI reads the long
description from the uploaded package metadata; a later upload is needed to
refresh it. See PyPI's [project-description guidance](https://docs.pypi.org/project-management/project-descriptions/)
and [upload troubleshooting](https://docs.pypi.org/publishing/troubleshooting/).

Earlier releases and their records are available from the
[GitHub release list](https://github.com/appunni-m/pillow-rs/releases).
