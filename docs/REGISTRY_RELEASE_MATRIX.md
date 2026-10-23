# Registry release matrix

## Current release: 12.2.3

Verified 2026-10-23. See the [v12.2.3 GitHub release](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.3)
for its release record and attached artifacts.

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

The PyPI release page contains all four `pillow-rs` files and all three
`pillow-rs-parallel` wheels for 12.2.3. The `pillow-rs` page's embedded project
description still shows a 12.2.2 install example; this repository documentation
update does not change the description already attached to the published artifact.

Previous versions and their release records are available from the
[GitHub release list](https://github.com/appunni-m/pillow-rs/releases). The
maintainer workflow is documented in [`RELEASING.md`](../RELEASING.md).
