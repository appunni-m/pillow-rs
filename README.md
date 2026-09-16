# pillow-rs

<!-- release:summary -->
**Latest release: [12.2.0-alpha.1](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.0-alpha.1).**
<!-- /release:summary -->

Rust image processing with a familiar Python `PIL` interface and one npm package
for Node.js and browsers. This alpha supports common image operations;
[check compatibility](https://appunni-m.github.io/pillow-rs/compatibility/) before replacing Pillow.

[Documentation](https://appunni-m.github.io/pillow-rs/) ·
[Supported APIs](https://appunni-m.github.io/pillow-rs/compatibility/) ·
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
.venv/bin/python -m pip install pillow-rs==12.2.0-alpha.1
```
<!-- /release:python -->

On Windows, use `.venv\Scripts\python.exe` instead of `.venv/bin/python`.
See [installation requirements](https://appunni-m.github.io/pillow-rs/installation/) for supported platforms.

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
