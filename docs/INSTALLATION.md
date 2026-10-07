# Installation

<!-- release:summary -->
**Latest release: [12.2.1](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.1).**
<!-- /release:summary -->

Install **pillow-rs** from PyPI, npm, or crates.io. Python imports `PIL`,
JavaScript imports `pillow-rs`, and Rust imports `pillow_rs`.

## Python

Use a fresh environment. Pillow and pillow-rs provide the same `PIL` package;
installing both in one environment can overwrite each other's files.

<!-- release:python -->
```sh
python3 -m venv .venv
.venv/bin/python -m pip install pillow-rs==12.2.1
```
<!-- /release:python -->

On Windows, replace `.venv/bin/python` with `.venv\Scripts\python.exe`.
Save the [first-image example](../README.md#make-your-first-image) as `first_image.py`
and run it with that environment's Python.

The normal wheel keeps Rayon disabled. Supported operations can still use the
compiled SIMD or GPU route through automatic backend selection. To install the
separately built Rayon-backed **Parallel CPU** extension from the 12.2.1
release, use the optional `parallel` extra:

```sh
.venv/bin/python -m pip install 'pillow-rs[parallel]==12.2.1'
```

The extra installs `pillow-rs-parallel`, which contains the distinct native
extension selected by the Python package. The extra pins the companion wheel to
the same version. It does not change the normal wheel, and it does not route
work to SIMD or GPU. Source builds can continue to opt in with Maturin's
`parallel` Cargo feature.

| Platform with a prebuilt wheel | Requirement |
| --- | --- |
| Linux x86-64 | glibc 2.28 or newer |
| macOS ARM64 | macOS 11 or newer |
| Windows x86-64 | 64-bit Python |

The Parallel CPU companion wheel uses the same prebuilt-platform matrix. For a
source checkout on another target, build the binding directly with the opt-in
Cargo feature:

```sh
python -m maturin develop --manifest-path pillow-rs-py/Cargo.toml --features parallel
```

The package accepts Python 3.8 or newer. Python 3.10 and 3.12 are covered by
full comparison runs; the release wheels are installed and exercised on Python 3.12.
Other Python versions are permitted by metadata but have less verification.
If pip cannot find a compatible wheel, a source install requires Rust 1.96.1
and a native linker. See [contributor setup](../CONTRIBUTING.md) for source development.

## Node.js and browsers

<!-- release:npm -->
```sh
npm install pillow-rs@12.2.1
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
pillow-rs = "=12.2.1"
```
<!-- /release:cargo -->

Requires Rust 1.96.1 or newer. Continue with the [Rust quickstart](RUST.md).

## Greenfield integration

Treat pillow-rs as a new integration. Pin an exact package version and validate
your application's images, modes, fonts, and errors before adopting it.

To compare with Pillow, create another environment and install Pillow there.
Run the two implementations in separate processes so their shared `PIL` name
cannot collide. See [Python recipes](PYTHON.md#evaluate-an-existing-pillow-application).

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Imports load upstream Pillow | Check `python -m pip show Pillow pillow-rs` using the application's interpreter |
| pip tries to compile Rust | Check the wheel's platform, architecture, and glibc requirements |
| Browser cannot load WASM | Verify the asset URL returns WASM rather than an HTML fallback |
| An operation is rejected | Check its mode, arguments, format, and [compatibility](COMPATIBILITY.md) |

For unresolved problems, see [support](../SUPPORT.md).
