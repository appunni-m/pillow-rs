# Python recipes

<!-- release:summary -->
**Latest release: [12.2.0-alpha.1](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.0-alpha.1).**
<!-- /release:summary -->

These examples use `PIL` after installing pillow-rs in its own environment.
They require no sample downloads or private files. Start with [installation](INSTALLATION.md#python)
and check [supported APIs](COMPATIBILITY.md).

## Create, encode, and reopen

```python
from io import BytesIO
from PIL import Image

image = Image.new("RGB", (3, 2), (255, 12, 34))
encoded = BytesIO()
image.save(encoded, format="PNG")
encoded.seek(0)
decoded = Image.open(encoded)
assert decoded.size == (3, 2)
assert decoded.tobytes() == image.tobytes()
```

Specify the format for an in-memory destination. Dimensions are
`(width, height)`; RGB bytes are tightly packed in row-major order.
Other modes have their own byte layouts.

## Transform an image

```python
from PIL import Image, ImageOps

original = Image.new("RGB", (3, 2), (255, 12, 34))
result = ImageOps.mirror(original).resize((6, 4))
assert original.size == (3, 2)
assert result.size == (6, 4)
```

`resize` returns an image. Other APIs may mutate their receiver according to
their Pillow contract; do not assume that every method is immutable.

## Evaluate an existing Pillow application

1. List the `PIL` modules and operations it imports.
2. Check those paths in the [maturity guide](COMPATIBILITY.md) and support inventory.
3. Create separate Pillow and pillow-rs environments with the same application inputs.
4. Run each implementation in its own process. Compare mode, dimensions, pixels,
   required metadata, and expected errors.
5. Include the fonts, palettes, sequences, and malformed inputs your application
   uses. A simple RGB resize does not establish those paths.

Importing two aliases in one interpreter does not isolate their shared package
name. Use one process per environment.

## Fonts and sequences

Font behavior depends on fontdone's supported paths. Successful masks or
measurements do not establish complete shaping, color-font, variation, or
FreeType C-ABI support. Consult the [fontdone inventory](https://appunni-m.github.io/fontdone/api-support/).

Sequence support depends on the container and operation. First-frame decoding
does not prove disposal, timing, multipage decoding, or sequence encoding.
Consult [codec capabilities](https://appunni-m.github.io/image-slash-star/capabilities/)
and compare the frame metadata your application needs.

## Errors and support

Unsupported paths can raise errors. Catch only errors your application can handle.

Reports should include package and Python versions, platform, API path, mode,
dimensions, and the smallest non-sensitive input. See [support](../SUPPORT.md).
