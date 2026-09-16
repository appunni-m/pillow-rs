# Installation

The package is **pillow-rs** on crates.io, PyPI, and npm. The current version is
`0.1.3`. Python imports `PIL`; Rust imports `pillow_rs`; JavaScript imports
`pillow-rs`.

## Python

Create a separate environment. Upstream Pillow and pillow-rs own the same
`PIL` package and must not be installed together.

```sh
python3 -m venv .venv
.venv/bin/python -m pip install pillow-rs==0.1.3
.venv/bin/python -c "from PIL import Image; print(Image.new('RGB', (3, 2)).size)"
```

The final command prints `(3, 2)`. On Windows, use
`.venv\Scripts\python.exe` instead of `.venv/bin/python`.

| Distribution | Published platform | Verification |
| --- | --- | --- |
| ABI3 wheel | Linux x86-64, glibc 2.28+ | Installed and exercised in release CI |
| ABI3 wheel | macOS ARM64, macOS 11+ | Installed and exercised in release CI |
| ABI3 wheel | Windows x86-64 | Installed and exercised in release CI |
| Source distribution | Other compatible Rust/Python hosts | Source build required; not a wheel-support claim |

Metadata permits Python 3.8+ and uses `abi3-py38`. Full Pillow comparisons use
Python 3.10 and 3.12; release wheels were installed on Python 3.12.10. The
metadata floor is not an executed test matrix for every Python version.

Source builds require Rust 1.96.1 and a native linker. Pip uses the declared
Maturin build backend. Prefer a wheel on a published platform.

## Node.js and browsers

```sh
npm install pillow-rs@0.1.3
```

The package declares Node.js 20+; CI uses 22.14.0. One WASM module serves both
environments through conditional exports. The [JavaScript guide](../pillow-rs-js/README.md)
covers initialization, browser asset paths, and cleanup. `pillow-rs-js` is
only the source folder name.

## Rust

```toml
[dependencies]
pillow-rs = "=0.1.3"
```

Rust 1.96.1 is the declared minimum and tested workspace toolchain. Cargo uses
the published fontdone and image-slash-star dependencies. See [Rust integration](RUST.md).

## Upgrade and rollback

Pin the version and keep your lockfile. Run the application's image corpus
before upgrading: pre-1.0 API and backend behavior are still evolving.

To return to upstream Pillow, create a fresh environment and install Pillow
there. Reusing an environment after one distribution overwrites the other's
files can leave mixed installations. Compare the implementations in separate
processes and environments.

## Troubleshooting

| Symptom | Check | Next step |
| --- | --- | --- |
| Imports load upstream Pillow | `python -m pip show Pillow pillow-rs` | Recreate an environment with only the intended distribution |
| Pip compiles Rust | Wheel platform, architecture, and glibc requirements | Use a wheel platform or install source-build tools |
| Browser cannot load WASM | Network response and bundler asset URL | Pass the emitted asset URL to `init()` |
| Operation or codec is rejected | Exact API, mode, format, and features | Check [compatibility](COMPATIBILITY.md) |

For unresolved problems, follow [SUPPORT.md](../SUPPORT.md).
