# Supported APIs and limitations

<!-- release:summary -->
**Latest release: [12.2.3](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.3).**
<!-- /release:summary -->

pillow-rs implements a selected subset of Pillow. A listed API can still
reject a mode, option, or input format that is outside its supported contract.
Check the exact path and declared requirements in the
[generated operation inventory](https://appunni-m.github.io/pillow-rs/api-support/)
before relying on it.

## Supported API families

| Task | APIs to start with |
| --- | --- |
| Create, open, save, and inspect images | `PIL.Image` and image properties |
| Resize, crop, rotate, or convert | `PIL.Image.Image` |
| Flip, mirror, fit, or combine images | `PIL.ImageOps` and `PIL.ImageChops` |
| Draw shapes and text | `PIL.ImageDraw` |
| Adjust color or apply filters | `PIL.ImageColor`, `PIL.ImageEnhance`, and `PIL.ImageFilter` |
| Load fonts and measure text | Selected `PIL.ImageFont` APIs |
| Work with palettes, sequences, or statistics | `PIL.ImagePalette`, `PIL.ImageSequence`, and `PIL.ImageStat` |

Start with the [Python recipes](PYTHON.md), [Rust guide](RUST.md), or
[JavaScript guide](../pillow-rs-js/README.md). JavaScript has its own API names
and initialization.

## Execution profiles

- **CPU:** Serial CPU is separate from Parallel CPU, SIMD, and GPU. Automatic
  routing may select another compiled profile for a supported call.
- **SIMD:** Architecture-specific vector paths are available for selected
  operations and modes; the profile does not imply that every call uses SIMD.
- **GPU:** Device, build, operation, and mode support vary. A GPU request can
  fall back to CPU; inspect backend receipts before treating a result as GPU
  execution.
- **Parallel CPU:** The optional `parallel` extra installs the separately built
  `pillow-rs-parallel` companion wheel. It is distinct from SIMD and GPU and
  cannot change features in an already installed standard wheel. See
  [installation](INSTALLATION.md#python) and the
  [registry release matrix](REGISTRY_RELEASE_MATRIX.md).

## Limitations

| Area | Current limit |
| --- | --- |
| Pillow replacement | APIs outside the selected operation inventory are not promised. |
| Modes and options | Support varies by operation; check the inventory for the combinations you use. |
| Image formats | Supported decoding, encoding, metadata, and frame behavior vary by format. |
| Fonts | Selected font loading, masks, and metrics are supported; complete shaping and color-font behavior are not promised. |
| GUI and color management | GUI integrations, screen capture, and external color-management behavior are outside the supported contract. |
| GPU | A compiled route requires a usable device and a supported operation; a request alone does not prove kernel execution. |
| WebAssembly | Available operations and codecs depend on the package build and host environment. |

The compatibility target is Pillow 12.2.0. The version prefix does not promise
complete Pillow API or mode compatibility.

## Before replacing Pillow

Try the operations, images, modes, fonts, and expected errors your application
uses in separate Pillow and pillow-rs environments. Follow the
[migration steps](PYTHON.md#evaluate-an-existing-pillow-application) and
[installation guide](INSTALLATION.md).
