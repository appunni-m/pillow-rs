# pillow-rs

[![CI](https://github.com/appunni-m/pillow-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/appunni-m/pillow-rs/actions/workflows/ci.yml)
[![Documentation](https://github.com/appunni-m/pillow-rs/actions/workflows/docs.yml/badge.svg?branch=main)](https://github.com/appunni-m/pillow-rs/actions/workflows/docs.yml)
[![Benchmarks](https://github.com/appunni-m/pillow-rs/actions/workflows/benchmark.yml/badge.svg?branch=main)](https://github.com/appunni-m/pillow-rs/actions/workflows/benchmark.yml)
[![Release](https://github.com/appunni-m/pillow-rs/actions/workflows/release.yml/badge.svg)](https://github.com/appunni-m/pillow-rs/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/appunni-m/pillow-rs?include_prereleases&sort=semver)](https://github.com/appunni-m/pillow-rs/releases)

<!-- release:summary -->
**Latest release: [12.2.0](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.0).**
<!-- /release:summary -->

Rust image processing with a familiar Python `PIL` interface and one npm package
for Node.js and browsers. This alpha supports common image operations;
[check compatibility](https://appunni-m.github.io/pillow-rs/compatibility/) before replacing Pillow.

[Documentation](https://appunni-m.github.io/pillow-rs/) ·
[Supported APIs](https://appunni-m.github.io/pillow-rs/compatibility/) ·
[What pillow-rs adds beyond Pillow](https://appunni-m.github.io/pillow-rs/compatibility/#pillow-rs-additions-beyond-pillow) ·
[Benchmark results](https://appunni-m.github.io/pillow-rs/benchmarks/)

## Install

| Your application | Package manager | Guide |
| --- | --- | --- |
| Python | pip: `pillow-rs`; import `PIL` | [Python installation](https://appunni-m.github.io/pillow-rs/installation/#python) |
| Node.js or browser | npm: `pillow-rs` | [JavaScript quickstart](https://appunni-m.github.io/pillow-rs/javascript/) |
| Rust | Cargo: `pillow-rs`; import `pillow_rs` | [Rust quickstart](https://appunni-m.github.io/pillow-rs/rust/) |

For Python, create a fresh environment. Pillow and pillow-rs both provide `PIL`,
so install them in separate environments.

<!-- release:python -->
```sh
python3 -m venv .venv
.venv/bin/python -m pip install pillow-rs==12.2.0
```
<!-- /release:python -->

On Windows, use `.venv\Scripts\python.exe` instead of `.venv/bin/python`.
See [installation requirements](https://appunni-m.github.io/pillow-rs/installation/) for supported platforms.
The separately built Rayon-backed **Parallel CPU** wheel is configured in the
current source tree, but a matching package-registry release is not available
yet. The ordinary wheel remains serial CPU. See the
[release matrix](https://github.com/appunni-m/pillow-rs/blob/main/docs/REGISTRY_RELEASE_MATRIX.md)
for package availability.

## Make your first image

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

This creates, mirrors, resizes, and encodes an RGB image entirely in memory.
Continue with [Python recipes](https://appunni-m.github.io/pillow-rs/python/).

## What can I use?

- Create, read, save, crop, resize, rotate, and convert images.
- Use supported drawing, filtering, color, palette, and statistics operations.
- Load fonts and render text within the documented font limitations.
- Use the [compatibility guide](https://appunni-m.github.io/pillow-rs/compatibility/) to check modes, formats, and unsupported integrations.

The version prefix identifies the targeted Pillow version. It does not promise
complete Pillow compatibility or stable alpha APIs.

## What pillow-rs adds beyond Pillow

Pillow is the behavior reference for the selected compatibility surface.
The items below are first-party runtime and contributor-tooling additions that
upstream Pillow does not bundle as equivalents.

**For applications**

- A Rust-owned image, font, and codec runtime, available as a Rust crate and
  through the Python `PIL` facade. Runtime processing does not call back into
  Pillow, FreeType, or native codec libraries.
- Rust consumers can select image codecs with Cargo features. This changes
  build composition; the codecs and image formats are not unique to pillow-rs.
- Deferred image-operation pipelines in the Rust core, so supported chained
  operations can execute when pixels are requested rather than after every
  individual call; compatible sibling chains can reuse an already materialized
  shared prefix.
- Selectable CPU, architecture-specific SIMD, and GPU execution routes, with
  controls to inspect and enable or disable compiled routes. A compiled route
  does not guarantee that every operation or device uses it.
- [Explicit GPU batching](docs/IMAGE_BATCHING.md) schedules existing lazy image
  graphs and streams keyed CPU or resident outputs within resource budgets.
  It uses `GpuBatchExecutor` and remains separate from normal Image routing.
- An opt-in Rayon **Parallel CPU** profile, separate from ordinary CPU, SIMD,
  and GPU execution. Its matching companion wheel is not available from the
  package registries yet; check the
  [release matrix](https://github.com/appunni-m/pillow-rs/blob/main/docs/REGISTRY_RELEASE_MATRIX.md)
  before expecting the `parallel` extra to install.
- A published JavaScript package for Node.js and browser WebAssembly.
- Optional pipeline receipts that report the requested and actual backend,
  per-operation paths, fallback, timing, and available resource details. Rust
  and WebAssembly expose these hooks; Python exposes them only through its
  internal binding module.

**For contributors**

- An isolated, pinned Pillow oracle and generated manifest for selected API
  parity inputs.
- Separate individual-operation and composed-pipeline benchmark reports,
  with backend and mode details and a separately labeled Parallel CPU profile.
  Parallel CPU reuses the ordinary Pillow measurement as its comparison
  baseline; it does not time a separate threaded Pillow run.
- GPU shader-dispatch evidence for distinguishing an actual kernel run from a
  requested route that fell back.

The [feature comparison and limits](https://appunni-m.github.io/pillow-rs/compatibility/#pillow-rs-additions-beyond-pillow)
records where each capability is available and what it does not guarantee.
These system additions do not mean the image operations themselves are unique
to pillow-rs; the selected compatibility surface and unsupported cases are
listed in the [compatibility guide](https://appunni-m.github.io/pillow-rs/compatibility/).

## Performance

[View benchmark results](https://appunni-m.github.io/pillow-rs/benchmarks/) for
per-workload comparisons. Each result identifies whether it measures the current
release or an older revision; performance depends on the input and hardware.

## Contribute and get help

[Contributing](https://appunni-m.github.io/pillow-rs/contributing/) covers source
builds, tests, and benchmark development.
[Support](https://appunni-m.github.io/pillow-rs/support/) ·
[Security](https://appunni-m.github.io/pillow-rs/security/) ·
[Releases](https://github.com/appunni-m/pillow-rs/releases) ·
[Changelog](https://appunni-m.github.io/pillow-rs/changelog/) ·
[Code of conduct](https://appunni-m.github.io/pillow-rs/conduct/)

See [LICENSE](LICENSE) for terms.

## Acknowledgements

Thank you to [Puhu](https://github.com/bgunebakan/puhu) for the Rust/Python image
processing work that informed the early exploration of this project, and to
[Pillow](https://python-pillow.org/) and its contributors for the image library,
public API, and reference behavior on which the compatibility work depends.
