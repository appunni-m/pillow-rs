# pillow-rs

Rust image processing with a familiar Python `PIL` interface and one WebAssembly
package for Node.js and browsers.

[Documentation](https://appunni-m.github.io/pillow-rs/) ·
[API support](https://appunni-m.github.io/pillow-rs/api-support/) ·
[Benchmarks](https://appunni-m.github.io/pillow-rs/benchmarks/) ·
[Rust API](https://docs.rs/pillow-rs/0.1.3/pillow_rs/)

**Current release: 0.1.3.** This is an early compatibility release. The maintained
Python, Node, and browser suites each pass 11,348 comparisons at the released
commit. That covers selected inputs, modes, and errors, not every Pillow API or
every possible image. Read [maturity and limitations](docs/COMPATIBILITY.md)
before replacing Pillow in an application.

## Choose your package

| Use case | Install | Entry point |
| --- | --- | --- |
| Python image processing | `python -m pip install pillow-rs==0.1.3` | `from PIL import Image` |
| Node.js or a browser | `npm install pillow-rs@0.1.3` | `import init, { Image } from 'pillow-rs'` |
| Rust application | `cargo add pillow-rs@=0.1.3` | `pillow_rs::Image` |

Python wheels are published for Linux x86-64 (glibc 2.28+), macOS ARM64, and
Windows x86-64. Other platforms may build from the source distribution; they
do not have a published wheel in this release. Rust source builds use Rust
1.96.1. See [installation](docs/INSTALLATION.md) for runtime requirements.

## Make your first image

Use a fresh Python environment. Both Pillow and pillow-rs provide `PIL`, so
install them in separate environments.

```sh
python3 -m venv .venv
.venv/bin/python -m pip install pillow-rs==0.1.3
```

On Windows, use `.venv\Scripts\python.exe` instead of `.venv/bin/python`.

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

Continue with [Python recipes](docs/PYTHON.md), [JavaScript and browser
integration](pillow-rs-js/README.md), or [Rust integration](docs/RUST.md).

## What to expect

- Creation, transformations, drawing, filters, color, and font operations have
  a fixed public contract. The [API inventory](https://appunni-m.github.io/pillow-rs/api-support/)
  identifies every selected path and its input cases.
- Python uses familiar `PIL` imports. JavaScript exposes an explicitly
  initialized WASM API; it does not reproduce Python syntax or objects.
- The Rust core uses [image-slash-star](https://github.com/appunni-m/image-slash-star)
  for codecs and [fontdone](https://github.com/appunni-m/fontdone) for fonts.
- CPU, SIMD, and optional GPU execution have different measured support.
  A passing comparison does not by itself establish native GPU execution.
- Release 0.1.3 does not promise complete Pillow replacement, hardened decoding
  of arbitrary hostile inputs, or a stable pre-1.0 API.

## Performance you can inspect

The [benchmark site](https://appunni-m.github.io/pillow-rs/benchmarks/) shows
workload timings, sample counts, spread, correctness gates, and actual backends.
Historical results retain their source revision. Missing measurements remain
unavailable. There is no single project-wide speedup claim.

Read the [measurement protocol](docs/BENCHMARKING.md) to reproduce a run.
Benchmark budget acceptance is separate from correctness and publication.

## Contribute

Bug reports, small reproductions, documentation improvements, and input-driven
parity fixes are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md).

```sh
make help
make setup-venv PYTHON=python3.12
make build-parity
make migration-parity-test
```

For documentation only, use `make docs-setup`, `make docs-build`, and
`make docs-serve`. Each repository builds and publishes its own GitHub Pages
site. The [command reference](docs/COMMANDS.md) explains side effects and scope.

## Project information

- [Support](SUPPORT.md) and [private security reporting](SECURITY.md)
- [Code of conduct](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md) and [release process](RELEASING.md)
- [Architecture](docs/ARCHITECTURE.md) and [coverage evidence](docs/COVERAGE.md)

The project is distributed under the [MIT-CMU License](LICENSE). Dependencies
and fixture assets retain their own licenses and notices.

## Acknowledgements

Thank you to [Puhu](https://github.com/bgunebakan/puhu) for the Rust/Python image
processing work that informed the early exploration of this project, and to
[Pillow](https://python-pillow.org/) and its contributors for the image library,
public API, and reference behavior on which the compatibility work depends.
