# pillow-rs

Rust image processing with a familiar Python `PIL` interface and one WebAssembly
package for Node.js and browsers.

[Documentation](https://appunni-m.github.io/pillow-rs/) ·
[API support](https://appunni-m.github.io/pillow-rs/api-support/) ·
[Benchmarks](https://appunni-m.github.io/pillow-rs/benchmarks/) ·
[Rust guide](https://appunni-m.github.io/pillow-rs/rust/)

**Current candidate: 12.2.0-alpha.1 (unreleased).** Cargo, npm, Python, and
this documentation use the same declared version. The base version tracks the
targeted Pillow release; `alpha.1` identifies this project's first candidate
for that target. Matching version numbers do not imply complete compatibility.
Read [maturity and limitations](https://appunni-m.github.io/pillow-rs/compatibility/)
before replacing Pillow in an application.

This candidate removes deprecated Rust interfaces and the former Python import
bridge. See the [migration guide](https://appunni-m.github.io/pillow-rs/rust/#upgrading-to-1220-alpha1)
and [release checklist](https://appunni-m.github.io/pillow-rs/releasing/#next-candidate).
[Published-release evidence](https://appunni-m.github.io/pillow-rs/coverage/)
retains the version and revision actually measured.

## Choose your package

The registry commands below apply **after this candidate is published**.
To try it now, [build from source](https://appunni-m.github.io/pillow-rs/installation/#build-the-unreleased-candidate).

| Use case | Install | Entry point |
| --- | --- | --- |
| Python image processing | `python -m pip install pillow-rs==12.2.0-alpha.1` | `from PIL import Image` |
| Node.js or a browser | `npm install pillow-rs@12.2.0-alpha.1` | `import init, { Image } from 'pillow-rs'` |
| Rust application | `cargo add pillow-rs@=12.2.0-alpha.1` | `pillow_rs::Image` |

The release workflow builds Python wheels for Linux x86-64 (glibc 2.28+), macOS ARM64, and
Windows x86-64. Other platforms may build from the source distribution; they
are outside the wheel matrix. Rust source builds use Rust
1.96.1. See [installation](https://appunni-m.github.io/pillow-rs/installation/) for runtime requirements.

## Make your first image

Use a fresh Python environment. Both Pillow and pillow-rs provide `PIL`, so
install them in separate environments.

```sh
python3 -m venv .venv
.venv/bin/python -m pip install pillow-rs==12.2.0-alpha.1
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

Continue with [Python recipes](https://appunni-m.github.io/pillow-rs/python/), [JavaScript and browser
integration](https://appunni-m.github.io/pillow-rs/javascript/), or [Rust integration](https://appunni-m.github.io/pillow-rs/rust/).

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
- This alpha does not promise complete Pillow replacement, hardened decoding
  of arbitrary hostile inputs, or a stable API.

## Performance you can inspect

The [benchmark site](https://appunni-m.github.io/pillow-rs/benchmarks/) shows
workload timings, sample counts, spread, correctness gates, and actual backends.
Historical results retain their source revision. Missing measurements remain
unavailable. There is no single project-wide speedup claim.

Read the [measurement protocol](https://appunni-m.github.io/pillow-rs/benchmarking/) to reproduce a run.
Benchmark budget acceptance is separate from correctness and publication.

## Contribute

Bug reports, small reproductions, documentation improvements, and input-driven
parity fixes are welcome. Start with [CONTRIBUTING.md](https://appunni-m.github.io/pillow-rs/contributing/).

```sh
make help
make setup-venv PYTHON=python3.12
make build-parity
make migration-parity-test
```

For documentation only, use `make docs-setup`, `make docs-build`, and
`make docs-serve`. Each repository builds and publishes its own GitHub Pages
site. The [command reference](https://appunni-m.github.io/pillow-rs/commands/) explains side effects and scope.

## Project information

- [Support](https://appunni-m.github.io/pillow-rs/support/) and [private security reporting](https://appunni-m.github.io/pillow-rs/security/)
- [Code of conduct](https://appunni-m.github.io/pillow-rs/conduct/)
- [Changelog](https://appunni-m.github.io/pillow-rs/changelog/) and [release process](https://appunni-m.github.io/pillow-rs/releasing/)
- [Architecture](https://appunni-m.github.io/pillow-rs/architecture/) and [coverage evidence](https://appunni-m.github.io/pillow-rs/coverage/)

The project is distributed under the [MIT-CMU License](LICENSE). Dependencies
and fixture assets retain their own licenses and notices.

## Acknowledgements

Thank you to [Puhu](https://github.com/bgunebakan/puhu) for the Rust/Python image
processing work that informed the early exploration of this project, and to
[Pillow](https://python-pillow.org/) and its contributors for the image library,
public API, and reference behavior on which the compatibility work depends.
