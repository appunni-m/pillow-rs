# Installation

<!-- release:summary -->
**Latest release: [12.2.0-alpha.1](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.0-alpha.1).**
<!-- /release:summary -->

Install **pillow-rs** from PyPI, npm, or crates.io. Python imports `PIL`,
JavaScript imports `pillow-rs`, and Rust imports `pillow_rs`.

## Python

Use a fresh environment. Pillow and pillow-rs provide the same `PIL` package;
installing both in one environment can overwrite each other's files.

<!-- release:python -->
```sh
python3 -m venv .venv
.venv/bin/python -m pip install pillow-rs==12.2.0-alpha.1
```
<!-- /release:python -->

On Windows, replace `.venv/bin/python` with `.venv\Scripts\python.exe`.
Save the [first-image example](../README.md#make-your-first-image) as `first_image.py`
and run it with that environment's Python.

| Platform with a prebuilt wheel | Requirement |
| --- | --- |
| Linux x86-64 | glibc 2.28 or newer |
| macOS ARM64 | macOS 11 or newer |
| Windows x86-64 | 64-bit Python |

The package accepts Python 3.8 or newer. Python 3.10 and 3.12 are covered by
full comparison runs; the release wheels are installed and exercised on Python 3.12.
Other Python versions are permitted by metadata but have less verification.
If pip cannot find a compatible wheel, a source install requires Rust 1.96.1
and a native linker. See [contributor setup](../CONTRIBUTING.md) for source development.

## Node.js and browsers

<!-- release:npm -->
```sh
npm install pillow-rs@12.2.0-alpha.1
```
<!-- /release:npm -->

Use Node.js 20 or newer, or a browser with WebAssembly support. The same package
selects the appropriate entry point for its environment. Browser bundlers must
serve the packaged WASM asset. Follow the [JavaScript guide](../pillow-rs-js/README.md)
for initialization and memory cleanup.

## Rust

Add this dependency to your application's `Cargo.toml`:

<!-- release:cargo -->
```toml
[dependencies]
pillow-rs = "=12.2.0-alpha.1"
```
<!-- /release:cargo -->

Requires Rust 1.96.1 or newer. Continue with the [Rust quickstart](RUST.md).

## Upgrade or return to Pillow

Keep an exact package version and your application's lockfile. Check your own
images, modes, fonts, and errors before upgrading an alpha. See
[breaking changes](RUST.md#upgrading-to-1220-alpha1) when upgrading from 0.1.x.

To compare with or return to Pillow, create another environment and install
Pillow there. Run the two implementations in separate processes so their shared
`PIL` name cannot collide. See [Python migration](PYTHON.md#evaluate-an-existing-pillow-application).

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Imports load upstream Pillow | Check `python -m pip show Pillow pillow-rs` using the application's interpreter |
| pip tries to compile Rust | Check the wheel's platform, architecture, and glibc requirements |
| Browser cannot load WASM | Verify the asset URL returns WASM rather than an HTML fallback |
| An operation is rejected | Check its mode, arguments, format, and [compatibility](COMPATIBILITY.md) |

For unresolved problems, see [support](../SUPPORT.md).
