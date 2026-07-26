# pillow-rs public API boundary audit

Date: 2026-07-26

## Scope

Probe every public item under `pillow-rs/src/**` except `pillow-rs/src/lib.rs`.
The intent is to make the binding crates depend on the `pillow-rs` root API,
not deep implementation paths such as `pillow_rs::ops::*`,
`pillow_rs::font::imagingft`, or backend internals.

## Key finding

A blind `pub` to `pub(crate)` rewrite outside `lib.rs` is not valid Rust API
design.

Types re-exported from `lib.rs` must remain `pub` at their definition site.
This applies at least to:

- `CheckedDims`
- `Image`
- `Draw`
- `PilError`
- `Font`
- `InfallibleExt` while it is still re-exported
- `PixelFormat`

The real rule should be:

1. `lib.rs` owns the public crate boundary.
2. Root-facing types and their intentionally supported methods remain `pub`.
3. Implementation modules and helper functions remain private or `pub(crate)`.
4. Binding crates call only the root/facade API and never deep implementation
   modules.

## Probe result

After making non-root public items crate-visible, `cargo check` reached a
useful failure mode: `-D dead_code` exposed items that were only alive because
they were external public endpoints.

Failure buckets from the probe:

| Bucket | Error count | Meaning |
| --- | ---: | --- |
| `font/pilfont.rs` | 24 | Legacy bitmap font surface is not used by core once hidden. Needs either a root `Font`/bitmap-font facade or deletion/deprecation decision. |
| `ops/quantize.rs` | 23 | Quantize internals and method are externally surfaced but not reached by root `Image` once hidden. |
| `ops/chops.rs` | 19 | `ImageChops`-style module functions need a root facade or should not be public. |
| `ops/imageops.rs` | 19 | `ImageOps`-style module functions need a root facade or should not be public. |
| `color.rs` | 16 | Color parsing/conversion helpers are currently deep public endpoints. Need root color API or private use through image operations. |
| `ops/filter.rs` | 15 | Filter kernels and filter methods need root/image facade classification. |
| `compute/registry.rs` | 12 | Registry/GPU shader descriptors are implementation internals and should stay private/crate-visible. |
| `ops/module_fns.rs` | 12 | Pillow module-level APIs need an explicit root facade. |
| `compute/op_def.rs` | 7 | Declarative registry helpers appear internal/dead from public API perspective. |
| `ops/convert.rs` | 6 | Conversion helpers and `Image::convert` need root-facing method classification. |
| `ops/array.rs` | 5 | Array/protocol layout helpers are binding-facing today; should be moved behind root API if still required. |
| `pipeline.rs` | 5 | Pipeline enums are implementation descriptors and should not be external API. |
| `ops/paste.rs` | 4 | `PasteSource` is a binding leak; root API should expose paste inputs without exposing the internal enum path. |
| `compute/backend_op.rs` | 3 | Backend operation traits/params are internals. |
| `ops/param_filters.rs` | 3 | Parameterized filter operations need root/image facade classification. |
| `image_utils.rs` | 2 | Raw buffer helpers are internals unless explicitly exposed by root. |
| `ops/resize.rs` | 2 | `parse_resample` is a binding leak; resize/thumbnail should be root/image methods. |
| `ops/utils.rs` | 2 | Utility helpers are internals. |
| Single-error operation files | 6 | `analysis`, `crop`, `enhance`, `rotate`, `split`, `transform` expose `Image` methods that need root-facing classification. |

## Current known deep binding leaks

The binding crates currently call these non-root paths directly:

- `pillow_rs::ops::paste::PasteSource`
- `pillow_rs::ops::resize::parse_resample`
- `pillow_rs::ops::{imageops,chops,module_fns,array,...}`
- `pillow_rs::font::imagingft`
- `pillow_rs::font::pilfont`
- `pillow_rs::compute`
- `pillow_rs::color`
- `pillow_rs::draw::outline_curve_points`

## Recommended execution plan

1. Keep `lib.rs` as the only public entry point, but replace broad `pub mod`
   exposure with either private modules plus root re-exports, or a dedicated
   `api` facade re-exported from root.
2. Keep root-facing public types `pub` at their definition site, but make their
   fields private unless a field is intentionally part of the API.
3. Add root/facade wrappers for current module-level Pillow APIs:
   `ImageOps`, `ImageChops`, image module functions, color parsing, backend
   selection, paste input construction, array layout, and font APIs.
4. Update Python and JS bindings to call only root/facade APIs.
5. Make implementation modules private/crate-visible after the facade exists.
6. Rerun:
   - `cargo check --manifest-path pillow-rs/Cargo.toml --locked`
   - `cargo check --manifest-path pillow-rs-py/Cargo.toml --locked`
   - `cargo check --manifest-path pillow-rs-js/Cargo.toml --locked`
   - maintained `make` targets after the boundary compiles.

## Do not commit

Do not commit the blind visibility probe. It intentionally breaks compilation
and is useful only as an audit mechanism.
