# pillow-rs

[![CI](https://github.com/appunni-m/pillow-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/appunni-m/pillow-rs/actions/workflows/ci.yml)
[![Documentation](https://github.com/appunni-m/pillow-rs/actions/workflows/docs.yml/badge.svg?branch=main)](https://github.com/appunni-m/pillow-rs/actions/workflows/docs.yml)
[![Benchmarks](https://github.com/appunni-m/pillow-rs/actions/workflows/benchmark.yml/badge.svg?branch=main)](https://github.com/appunni-m/pillow-rs/actions/workflows/benchmark.yml)
[![Release](https://github.com/appunni-m/pillow-rs/actions/workflows/release.yml/badge.svg)](https://github.com/appunni-m/pillow-rs/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/appunni-m/pillow-rs?include_prereleases&sort=semver)](https://github.com/appunni-m/pillow-rs/releases)
[![Sponsor](https://img.shields.io/badge/Sponsor-GitHub%20Sponsors-EA4AAA?logo=githubsponsors&logoColor=white)](https://github.com/sponsors/appunni-m)

<!-- release:summary -->
**Latest release: [12.2.3](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.3).**
<!-- /release:summary -->

Rust image processing with Python's familiar `PIL` interface and a shared
WebAssembly package for Node.js and browsers. The project supports a selected
set of Pillow-compatible APIs; check the [supported APIs and limitations](https://appunni-m.github.io/pillow-rs/compatibility/)
before replacing Pillow.

[Documentation](https://appunni-m.github.io/pillow-rs/) ·
[Supported APIs](https://appunni-m.github.io/pillow-rs/compatibility/)

## Install

| Your application | Package manager | Guide |
| --- | --- | --- |
| Python | `pip install pillow-rs`; import `PIL` | [Installation](https://appunni-m.github.io/pillow-rs/installation/) |
| Node.js or browser | `npm install pillow-rs` | [JavaScript quickstart](https://appunni-m.github.io/pillow-rs/javascript/) |
| Rust | Cargo dependency `pillow-rs`; import `pillow_rs` | [Rust quickstart](https://appunni-m.github.io/pillow-rs/rust/) |

For Python, use a fresh environment. Pillow and pillow-rs both provide the
`PIL` namespace, so install them in separate environments.

<!-- release:python -->
```sh
python3 -m venv .venv
.venv/bin/python -m pip install pillow-rs==12.2.3
```
<!-- /release:python -->

On Windows, use `.venv\Scripts\python.exe` instead of `.venv/bin/python`.
See [installation requirements](https://appunni-m.github.io/pillow-rs/installation/)
for supported platforms and the optional Parallel CPU package.

## First image

```python
from io import BytesIO
from PIL import Image, ImageOps

image = Image.new("RGB", (3, 2), (255, 12, 34))
image = ImageOps.mirror(image).resize((6, 4))
output = BytesIO()
image.save(output, format="PNG")
assert image.size == (6, 4)
assert output.getvalue().startswith(b"\x89PNG\r\n\x1a\n")
```

This creates, mirrors, resizes, and encodes an RGB image in memory. Continue
with [Python recipes](https://appunni-m.github.io/pillow-rs/python/).

## Supported behavior

The supported surface includes image creation and file I/O, geometry and mode
conversion, drawing, filters, color operations, palettes, statistics, and font
rendering. Availability depends on the API, mode, format, and platform; the
[compatibility guide](https://appunni-m.github.io/pillow-rs/compatibility/)
lists the supported combinations and known limits. The version prefix targets
Pillow 12.2.0 and does not promise complete Pillow compatibility.

## Contribute and get help

[Contributing](https://appunni-m.github.io/pillow-rs/contributing/) covers
building the project and proposing changes. For help, see
[support](https://appunni-m.github.io/pillow-rs/support/),
[security reporting](https://appunni-m.github.io/pillow-rs/security/),
[releases](https://github.com/appunni-m/pillow-rs/releases), and the
[changelog](https://appunni-m.github.io/pillow-rs/changelog/).

See [LICENSE](LICENSE) for terms.

## Acknowledgements

Thank you to [Puhu](https://github.com/bgunebakan/puhu) for the Rust/Python image
processing work that informed the early exploration of this project, and to
[Pillow](https://python-pillow.org/) and its contributors for the image library,
public API, and reference behavior on which the compatibility work depends.
