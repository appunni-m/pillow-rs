# Maturity and compatibility

**12.2.0-alpha.1 is an unreleased compatibility candidate.** It offers useful image
operations and a tested `PIL` facade. It does not implement all of Pillow,
every image-format specification, or the full FreeType API.

## Read status correctly

| Label | What it establishes | What it does not establish |
| --- | --- | --- |
| Tested case | A named comparison passed on a recorded runner | All arguments, modes, platforms, or backends |
| Declared support | A manifest or runtime table exposes a path | Complete executed evidence |
| Partial | The documented subset is available | The remainder of the API or format |
| Unsupported | No supported application path is claimed | Success inferred from a stub |
| Unmeasured | No qualifying evidence is available | Success, failure, or zero cost |

The [generated inventory](https://appunni-m.github.io/pillow-rs/api-support/)
lists all 209 selected operations across 24 surfaces. Its source is the
[manifest](../pillow-rs/tests/fixtures/manifest.yaml); case counts come from
indexed inputs. Declared support, input count, and executed outcomes are
separate facts.

## API paths

| Family | Current scope |
| --- | --- |
| `PIL.Image`, `PIL.Image.Image` | Selected creation, codec, pixel, geometry, conversion, and composition operations |
| `PIL.ImageOps`, `PIL.ImageChops`, `PIL.ImageColor` | Selected operations, modes, and argument combinations |
| `PIL.ImageDraw`, `PIL.ImageEnhance`, `PIL.ImageFilter` | Selected drawing, enhancement, and filter paths |
| `PIL.ImageFont` and font classes | Selected loading, masks, metrics, and geometry; fontdone limitations apply |
| `PIL.ImagePalette`, `PIL.ImageSequence`, `PIL.ImageStat` | Selected palette, iterator, and statistics paths |
| Paths outside the inventory | Unmeasured by this contract; no drop-in support claim |
| GUI integration, platform capture, external color management | Outside the selected public contract |
| JavaScript | Its own initialized WASM API; selected parity workflows execute through it |

Presence of an API does not establish every mode. The inventory links to
inputs for covered modes, sizes, keywords, and failures.

## Backend support

| Backend | Evidence and limits |
| --- | --- |
| Python CPU | 11,348 maintained cases passed on both release-commit Python CI lanes |
| Node WASM | 11,348 cases passed on the release-commit Node runner |
| Browser WASM | 11,348 cases passed in the CI headless browser; other browser/device combinations are not implied |
| SIMD | Separate backend campaigns exist; routing may include explicit host controls |
| GPU | Requires an available adapter and native dispatch receipt; unsupported paths and host controls remain visible |
| WGSL line coverage | Unmeasured; Rust LCOV does not instrument shader lines |

Broader floating-point GPU arithmetic proofs and zero-violation benchmark
budget acceptance remain open. Requesting a GPU backend is not proof of native
execution.

## Codecs and fonts

Codec support comes from enabled image-slash-star features. Still images,
sequences, metadata, and encoding have separate capabilities. AVIF is incomplete
and outside the current browser codec contract. Consult [codec support](https://appunni-m.github.io/image-slash-star/capabilities/).

Fontdone maintains a conservative per-function adoption map. Passing fixtures
or exporting a C symbol does not make all FreeType behavior available. Text
shaping, full color-font rendering, and broad C replacement need separate
review. Consult [fontdone maturity](https://appunni-m.github.io/fontdone/maturity/).

## Published-release evidence

[Main CI](https://github.com/appunni-m/pillow-rs/actions/runs/35014896711)
tested commit `fd78eb80402a0d6d99e6b696d0a1ebf9ba11d5fb`. Python 3.10,
Python 3.12, Node, and browser each passed 11,348/11,348 comparisons.
[Release CI](https://github.com/appunni-m/pillow-rs/actions/runs/35017008075)
also installed native wheels on Linux, macOS, and Windows before publication.
[Coverage](COVERAGE.md) records the source-bound collection.

These are versioned observations. A documentation build does not turn an old
measurement into evidence for the newest source revision.
