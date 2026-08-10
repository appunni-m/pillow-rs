# Dynamic formats coverage batch 2

Baseline: `bc6cfbdfb994f3564bfb6803284d680c290e8a21`.

## Result

The managed CPU coverage report for
`pillow-rs/src/raster/dynamic.rs` measured `388/865` executable lines before
and after this batch (`44.86%`). The final report also measured 2/4 branches,
60/119 functions, and 741/1636 regions. The new public case does not execute
this file: the Python image facade's `putpixel` operation calls
`Image::putpixel_value`, which owns the native 16-bit storage path in
`pillow-rs/src/image.rs`.

The batch added one public corpus case:

`PIL.Image.Image.putpixel.nuanced.l16-png-putpixel`

It opens a valid 16-bit grayscale PNG and writes `0x1234` at `(0, 0)`. The
Rust path now writes the native `u16` sample directly, preserving Pillow's
modulo-65536 integer behavior and the existing mode-specific byte order at
serialization time. The focused parity result was 1/1 passed on CPU and 1/1
passed on SIMD, with no infrastructure errors.

## Remaining dynamic.rs lines

The remaining typed branches are classified rather than covered with
synthetic inputs:

- Existing public PNG cases already cover the native `L16` crop, resize,
  rotate, convert, and all five maintained transpose methods. The batch did
  not duplicate those cases.
- `image-slash-star` at the pinned revision `d7e60df` decodes 16-bit grayscale
  PNG as native `L16`. Its 16-bit RGB and RGBA PNG paths are represented as
  byte-sample RGB/RGBA images, and grayscale-plus-alpha is represented as
  byte-sample RGBA. Consequently, `DynamicImage`'s `LumaA16`, `Rgb16`,
  `Rgba16`, `Rgb32F`, and `Rgba32F` crop/transform arms are not reachable from
  the allowed typed-PNG public corpus.
- `DynamicImage::put_pixel` has typed arms for those variants, but the public
  Pillow facade does not dispatch `Image.putpixel` through that trait. Routing
  the operation through it would also lose the integer value's 16-bit
  semantics. The direct `Image` storage path is the correct public behavior.

Do not add fabricated typed buffers or excluded TIFF/GPU/crash inputs merely to
raise this file's line percentage. If these `DynamicImage` variants need
coverage later, they require a separate direct-core API test lane rather than
a public PNG parity case.

## Verification

- `make migration-parity-inputs`: deterministic generation succeeded.
- Focused CPU parity: 1 selected, 1 executed, 1 passed, 0 infrastructure
  errors.
- Focused SIMD parity: 1 selected, 1 executed, 1 passed, 0 infrastructure
  errors.
- The managed dynamic-formats worker run passed and ingested two artifacts;
  `dynamic.rs` remained 388/865.
- `make migration-parity-inputs-check` passed, including the crash quarantine
  reproduction check.

The pending TIFF, crash, GPU, and fontdone lanes were not used.
