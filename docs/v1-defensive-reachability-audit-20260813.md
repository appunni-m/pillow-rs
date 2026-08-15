# Pillow-RS v1 defensive and unsupported reachability audit

Status: read-only investigation complete. No runtime code, manifest, fixture,
roadmap-status, or coverage-setting file was changed.

## Provenance and denominator boundary

This audit was performed from committed revision
`d617752a3df3ff9a8da0eab65e473b8628204c45` in an isolated worktree. It covers
`pillow-rs`, `pillow-rs-py`, and `pillow-rs-js`; it excludes unit tests, parity,
benchmark execution, fixture generation, fontdone edits, and GPU execution.

The production inventory at that revision was:

| Source/artifact | Tracked files | Classification |
|---|---:|---|
| `pillow-rs/src/**/*.rs` | 72 | Core Rust production source |
| `pillow-rs-py/src/**/*.rs` | 1 | PyO3 ABI/binding source |
| `pillow-rs-py/python/**/*.py` | 14 | Thin Python facade |
| `pillow-rs-js/src/**/*.rs` | 1 | WASM binding source; no managed JS component |
| `pillow-rs/src/compute/pool_gpu/shaders/**/*.wgsl` | 85 | Embedded shader assets, not LLVM Rust source |
| `build/fontdone-src` | 0 | No pinned checkout in the audited worktree; not inspected |

The managed coverage components do not list `ops/utils.rs`, `image_utils.rs`,
`raster/dynamic.rs`, `raster/buffer.rs`, `compute/registry.rs`, or the binding
crate roots. Those files may appear in a whole-project artifact, but that is a
different denominator. No file or region is removed or silently reclassified.
There are 12 source files containing `#[cfg(test)]` modules; they were neither
run nor used as coverage evidence.

## Reachability classes

| Class | Treatment |
|---|---|
| Public-valid | Reach with a maintained public endpoint and a valid input or documented error-contract input |
| Public-Rust-only | Keep in a separate Rust API lane; do not add a PIL parity case |
| Invariant-defensive | Do not manufacture malformed internal state to reach it |
| Backend/feature boundary | Require the matching managed component and environment |
| Unsupported public contract | Keep the exact public error visible; do not turn it into a skip |
| Test-only | Outside this campaign |

## Examples of code not honestly reachable through valid public PIL inputs

| File/region | Classification | Reason and valid alternative |
|---|---|---|
| `pillow-rs/src/raster/dynamic.rs:167-184` | Defensive unsupported `ColorType` wildcard | Python/JS paths cannot construct those variants. Use named valid color types in a separate Rust API lane. |
| `pillow-rs/src/raster/dynamic.rs:1241-1245` | Deliberately unsupported `DynamicImage::get_pixel_mut` | Use checked accessors and typed mutation APIs; do not target this with a parity case. |
| `pillow-rs/src/raster/buffer.rs:158-173`, `252-268` | Broken-buffer invariant | Zero-width valid rows can be a Rust-only case; malformed short buffers are not valid inputs. |
| `pillow-rs/src/raster/buffer.rs:699-716`, `819-830` | Unchecked coordinate panic | Invalid coordinates are not a valid Pillow pixel contract; use checked accessors. |
| `pillow-rs/src/raster/buffer.rs:861-880` | Allocation-size overflow | Requires dimensions that overflow `usize`; no safe PIL input should allocate it. |
| `pillow-rs/src/draw/mod.rs:897-913` | Post-validation bitmap-mask wildcard | Valid public modes are `1`, `L`, `RGBA`, and `RGBa`; use those four arms. |
| `pillow-rs/src/ops/quantize.rs:411-421`, `982-1094` | Quantizer tree/axis invariant | The quantizer computes these values internally; do not forge an invalid node or axis. |
| `pillow-rs/src/compute/pool_cpu/ops/effects.rs:945-960`, `1086-1100` | Invalid channel-count fallback | Public transforms reach only 1–4 channels; use valid L/LA/RGB/RGBA transforms. |
| `pillow-rs/src/image_utils.rs:56-82`, `102-132` | Checked 1–4 channel reconstruction | The wildcard requires a broken internal buffer; ordinary operations already cover valid layouts. |
| `pillow-rs/src/compute/pool_simd/ops/adapters.rs:1581-1587` | SIMD merge mode invariant | Mode code is constrained by the preceding match; use valid merge modes. |
| `pillow-rs/src/image.rs:2962-2998` | Palette mutation post-validation wildcard | Invalid modes return the earlier public `ValueError`; use valid L/P/LA/PA cases. |
| `pillow-rs/src/image.rs:3682-3691` | EXIF post-load invariant | A successful `load()` cannot leave `Bytes`/`Pipeline`; do not construct that state. |
| `pillow-rs-py/src/lib.rs:684-693` | `Image.open` exhaustive invalid-format arm | Input validation rejects the enum value first; use valid format names and `None`. |

## Public unsupported and backend boundaries

These are visible behavior, not missing denominator entries:

- `compute/mod.rs:232-249,269-278`: backend unavailable/no-native errors.
- `compute/registry.rs:426-465`: GPU/SIMD capability checks and rotate
  exclusions.
- `compute/registry.rs:1346-1375`: GPU autocontrast/equalize rejection for
  unsupported LA/RGBA paths.
- `pool_simd/ops/adapters.rs:1214-1238`: valid HSV/YCbCr/I/F/P/1 conversion
  requests that deliberately delegate to CPU.
- `ops/imageops.rs:245-382`: public mode-specific ImageOps errors.
- `ops/paste.rs:219-225` and `ops/rotate.rs:31-129`: public unsupported
  destination/resampling errors.
- `format.rs:19-21`, `image.rs:1008-1025`, `2484-2501`: unknown formats,
  unsupported `frombytes` modes, and encoder mode/format errors.
- `image.rs:3313-3319,4804-4821`: intentionally unimplemented multi-frame
  behavior and nonzero `seek` errors.
- `pillow-rs-js/src/lib.rs:17-40,1731-1780`: JS/WASM ABI paths without a
  managed JS coverage component.

The valid future inputs are real public error-contract cases, supported mode
matrix cases, and a Qt-capable managed run for `toqimage`/`toqpixmap`. No fake
operation descriptor, malformed buffer, wrong channel count, unchecked
coordinate, or invented font/image stream is justified.

## Feature and generated-code treatment

- The default core feature set includes GPU, codecs, parallelism, and the
  default font feature; `cfg(feature = "gpu")` is a build boundary, not proof
  of GPU execution.
- `test-api` exports and debug hooks are not ordinary Pillow endpoints.
- Unix/non-Unix binding alternatives are platform-specific source, not same-
  platform runtime branches.
- The 85 WGSL files are embedded shader assets, not generated Rust source and
  not a reason to remove GPU coordinator code from a whole-project denominator.
- Manifest, fixture, generated-report, and documentation files are evidence,
  not production execution lines.

No fresh Coverage MCP percentage was produced by this audit. Historical
coverage classifications were treated as prior evidence only.
