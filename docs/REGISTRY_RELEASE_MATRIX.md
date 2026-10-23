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

The [PyPI 12.2.3 page](https://pypi.org/project/pillow-rs/12.2.3/) still shows
the long description uploaded with 12.2.2, including its old install command.
The published release files and the installation guide above are 12.2.3; PyPI's
description will refresh with a future package upload.

Earlier releases and their records are available from the
[GitHub release list](https://github.com/appunni-m/pillow-rs/releases).
