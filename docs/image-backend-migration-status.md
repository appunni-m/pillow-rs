# image-slash-star Backend Migration Status

Status: native backend migration, lazy-loading correctness, and JS/WASM
core-extra codec packaging are implemented. Broader binding runtime parity
remains separate work.

The normative design and acceptance criteria live in
`../image-slash-star/docs/image-backend-migration-spec.md`. This file records
the downstream changes and the evidence still required in `pillow-rs`.

The implemented correctness contract for persistent implicit loading, stable
path snapshots, shared decode/pipeline caches, and copy-on-write mutation lives
in `../image-slash-star/docs/lazy-loading-correctness-proposal.md`.

## Dependency And Feature Boundary

- The historical `pillow-rs-image` directory has been removed from this repo;
  codec ownership moved to the sibling `image-slash-star` package.
- `pillow-rs` depends directly on package `image-slash-star` with
  `default-features = false`; Rust imports use `image_slash_star`.
- `image-slash-star` exclusively owns signature detection, header inspection,
  decoding, encoding, and canonical format name/extension parsing. The unused
  downstream `FormatHandler` registry was removed so it cannot become a second
  codec or detection implementation.
- `pillow-rs` only adapts upstream errors to Pillow error categories and wraps
  decoded data in its lazy/persistent image state.
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
of the stable snapshot without changing ordinary load state. The first
pixel-dependent access initializes a shared success-or-error cache for the
source or immutable pipeline node; clones and concurrent readers observe that
same published result. `load(&mut self)` reuses it and replaces the handle with
persistent loaded or indexed storage.

`Image::open` reads a path once, so later path replacement cannot change the
object's identity. Loaded pixels use shared immutable storage, and mutation
detaches before writing. This gives ordinary reads persistent lazy behavior
without forcing callers to invoke `load()` and gives clones copy-on-write
isolation.

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
- fixture-based unknown-signature and malformed-PNG structured errors;
- cloned and concurrent implicit first access for every codec;
- stable path snapshots, explicit format-hint mismatch, and repeated malformed
  source failures;
- shared pipeline materialization and exact source bytes after every operation;
- copy-on-write mutation isolation for loaded, indexed, and pipeline-produced
  images.

No synthetic encoded error byte arrays are used.

## Explicit Backend Feature Matrix

The migration suite must name the behavior it proves; opening an image and
observing a side effect is not sufficient evidence for a separate API promise.

| Feature | Downstream evidence | Status |
|---|---|---|
| magic-byte auto-detection | canonical API and manifest assertions live in `image-slash-star`; downstream observes the result through `Image::open_bytes` | passing |
| header inspection | canonical API and manifest assertions live in `image-slash-star`; downstream compares its cached `ImageInfo` | passing |
| decoded envelope | canonical API and manifest assertions live in `image-slash-star`; downstream proves state/pixels after materialization | passing |
| wrapper detection/cache | `Image::open_bytes` caches upstream `ImageInfo` without materializing | passing |
| source format and mode stability | asserted before and after `verify` and persistent `load` | passing |
| structured encoded failures | file-backed unknown input and malformed PNG rows | passing |
| indexed palette and transparency | exact Pillow palette RGB and alpha for indexed PNG | passing |
| generic PNG encoding | every allowed palette operation compares exact Pillow PNG bytes before and after `load` | passing |
| path opening and explicit format hint | fixture test proves a one-read stable path snapshot and exact hint-mismatch error | passing |
| disabled-codec forwarding | reduced-feature manifest test proves exact `FeatureDisabled` results through downstream forwarding | passing |
| animated sequence decode/encode | implemented and proven in `image-slash-star`; `pillow-rs::Image` still models a single loaded image | outside this downstream slice |
| all-format save parity through `pillow-rs` | upstream encoders are proven; downstream currently proves generic PNG only | future downstream slice |

## Palette Operation Audit

Palette preservation is allowed only when an operation has a Pillow fixture
that observes exact `P` mode, dimensions, index bytes, palette bytes, and
transparency. `scripts/generate_image_backend_operation_fixtures.py` uses the
pinned Pillow 12.2.0 oracle to generate
`tests/fixtures/image_backend/operations.json` and its raw output files.

| Operation | Exact oracle cases | Preservation decision |
|---|---:|---|
| crop | 1 | allowed |
| nearest resize | 1 | allowed |
| nearest thumbnail | 1 | allowed |
| rotate without custom fill | 1 arbitrary-angle expanded case | allowed |
| transpose | all 7 Pillow transpose methods | allowed |
| `ImageOps.flip` / `mirror` / `crop` | 3 | allowed |
| `ImageChops.offset` / `duplicate` | 2 | allowed; duplicate is a direct copy |
| nearest affine transform with zero fill | 1 | allowed |
| indexed `putpixel` | 1 direct and 1 after crop | allowed; clone isolation proven |
| effect spread | 0 deterministic cases | not allowed; Pillow is randomized |
| mesh transform or custom transform/rotate fill | 0 | not allowed |
| filters, enhancement, conversion, drawing, LUT/point, composition | 0 in this audit | not allowed |

The Rust fixture test dispatches each manifest row through the public API and
requires exact state and exact Pillow PNG output both before and after
persistent `load()`. A mismatch must move that operation to visible-color
expansion; a new operation cannot be added to the safe matcher without a
corresponding Pillow row.

## Acceptance Evidence

The downstream repository is intentionally tested manually rather than through
Coverage MCP. The maintained evidence is:

```text
make image-backend-test
make image-backend-feature-test
make fmt
make repo-map-check
cargo check -p pillow-rs-py -p pillow-rs-js
```

Both manifest targets pass, including all eight codecs, exact structured
errors, feature forwarding, lifecycle/cache behavior, and all 19 palette
operation rows. Formatting, repository-map validation, and both binding checks
pass. `git diff --check` is clean.

A full `make pillow-rs-test` run also passes the core and image suites. Its only
remaining failures are three pre-existing FreeType scalar `getlength` parity
rows; the corresponding 7,632-case pixel matrix has zero failures. Those
font-only differences are outside this backend migration.

The upstream `image-slash-star` acceptance remains Coverage MCP run
`5b0f1ca0-0ecf-433b-a159-722387249757`, snapshot
`2a9e4148-d559-44db-8368-57df58bf21fc`, with exact 100% line, branch,
function, and region coverage.

The native workspace and zero-codec core compile. The prior `fontdone` wasm32
width blocker is resolved with a target-scoped internal arithmetic feature that
matches the standalone export layer's explicit 64-bit compatibility ABI. Both
optimized JS/WASM variants compile and pass the fixture codec matrix; exact
sizes and commands are recorded in `docs/wasm-core-extra-packaging.md`.
