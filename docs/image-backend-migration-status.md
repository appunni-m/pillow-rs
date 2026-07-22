# image-slash-star Backend Migration Status

Status: implementation complete through the first downstream integration
slice; fixture execution and Coverage MCP acceptance are pending.

The normative design and acceptance criteria live in
`../image-slash-star/docs/image-backend-migration-spec.md`. This file records
the downstream changes and the evidence still required in `pillow-rs`.

## Dependency And Feature Boundary

- The historical `pillow-rs-image` directory is excluded from the workspace
  and retained only as a reference.
- `pillow-rs` depends on package `image-slash-star` with the local crate alias
  `pillow_rs_image` and `default-features = false`.
- `image-codecs-all` forwards JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO.
- AVIF remains explicit as `image-avif`.
- The direct `png` dependency and PNG-specific decoder are removed.
- The resulting lockfile removes the old `png`, `gif`, `tiff`, `image-webp`,
  and their compression/helper dependency graph from `pillow-rs`.

## Stored State Contract

Encoded container format and decoded pixel mode are separate facts:

- `source_format` is the detected `ImageFormat` and remains stable when lazy
  input becomes materialized.
- `decoded_mode` is the exact `ImageMode` produced by the codec.
- `explicit_mode` is reserved for Pillow operation modes that a generic
  `DynamicImage` cannot express directly.
- `ImageInfo` is cached on path/byte sources and copied to their loaded result.
- indexed storage retains indices, RGB palette bytes, and per-entry alpha.

`size`, `mode`, `format_name`, palette access, and `image_info` use cached
headers without decoding pixel payloads. `verify(&self)` performs a full decode
without changing state. `load(&mut self)` replaces lazy input with persistent
loaded or indexed storage.

## Compatibility Impact

- `Image::Loaded` now carries `LoadedData` rather than a pixel buffer plus an
  optional string.
- `Image::load` now requires `&mut self`; the Python and JavaScript bindings
  already have mutable receivers.
- unknown signatures map to `UnidentifiedImageError` at the Pillow boundary;
  malformed, unsupported, and disabled-codec failures retain structured
  `image-slash-star::ImageError` values.
- PNG save/byte output now sends `P8 + ImagePalette` to the generic encoder, so
  indexed transparency is no longer discarded by an RGB expansion.

## Fixture Acceptance

`pillow-rs/tests/fixtures/image_backend/manifest.json` is pinned to Pillow
12.2.0 and contains only file-backed cases. It covers:

- exact source format, mode, dimensions, and decoded bytes for all eight
  codecs;
- lazy metadata access without materialization;
- non-mutating verification and persistent load;
- source mode, format, and metadata stability across load;
- exact indexed PNG palette RGB and alpha;
- fixture-based unknown-signature and malformed-PNG structured errors.

No synthetic encoded error byte arrays are used.

## Palette Operation Audit

Palette preservation is allowed only when an operation has a Pillow fixture
that observes `P` mode and exact output bytes. Existing oracle fixtures cover
the currently index-domain paths individually:

| Operation family | Pillow fixture files | Retained state |
|---|---|---|
| crop, resize, rotate, transpose, transform, thumbnail, reduce | `Image.<operation>.json` | indices and palette |
| point/eval, putpixel, putdata, putalpha | `Image.<operation>.json`, `ImageModule.eval.json` | indices and palette where Pillow retains `P` |
| duplicate, constant, offset, invert | `ImageChops.<operation>.json`, `ImageOps.invert.json` | indices and palette |
| blend and composite | `ImageChops.*.json`, `ImageModule.*.json` | Pillow-observed index-domain result |
| draw primitives | `ImageDraw.<primitive>.json` | indices and palette |

Color lookup tables, filters, enhancement, and color conversion are not
palette-safe and must materialize visible colors. The listed retained paths
remain provisional until the downstream parity suite is rerun against this
migration; any mismatch moves that operation to color expansion before adding
new behavior.

## Acceptance Gate

The integration slice is accepted only after all of the following are true:

1. formatting and workspace/native feature checks pass;
2. the fixture manifest passes through Coverage MCP;
3. the full Pillow operation parity suite passes;
4. Coverage MCP reports 100% line, branch, function, and region coverage for
   the accepted scope;
5. `git diff --check` is clean and `.coverage-mcp/` remains uncommitted.

The native workspace and zero-codec core currently compile. The PNG-only WASM
consumer remains blocked by pre-existing `fontdone` wasm32 C-ABI width
assumptions; that is a separate core/extra packaging slice and is not evidence
against the image backend semantics.
