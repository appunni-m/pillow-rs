# JS/WASM Core And Extra Packaging

Status: implemented and measured on 2026-07-23.

## Contract

The npm package exposes two independently built wasm-bindgen artifacts:

- `pillow-rs` (the default export) loads `pkg/core` with PNG only;
- `pillow-rs/extra` loads `pkg/extra` with PNG, JPEG, GIF, BMP, TIFF, WebP,
  and ICO.

Both artifacts contain the same Pillow operation and font API. The split only
changes encoded-image codec availability. ICO intentionally brings its PNG and
BMP requirements. AVIF is in neither browser artifact because
`image-slash-star` has no AVIF-capable WASM implementation; valid AVIF input
therefore retains the structured disabled/capability boundary instead of being
misclassified as malformed.

The browser runtime and the separate `fontdone-ffi-wasm` export layer enable
`fontdone/wasm-wide-internals`. This preserves the LP64 FreeType arithmetic
used by the pinned native Pillow oracle and matches the export layer's explicit
64-bit `FT_Long`/`FT_ULong` compatibility ABI. Native targets continue to use
their platform C aliases.

## Release Sizes

Built with `wasm-pack 0.15.0`, `--target web --release`, LTO, one codegen unit,
`panic = "abort"`, wasm-opt enabled by wasm-pack, and debug hooks disabled.
Compression uses gzip level 9 and Brotli quality 11 over the `.wasm` file.

| Variant | Codec features | WASM | gzip | Brotli | JS glue | Type declarations | Full generated directory |
|---|---|---:|---:|---:|---:|---:|---:|
| core | PNG | 1,792,058 B | 589,035 B | 441,120 B | 94,211 B | 52,128 B | 1,938,771 B |
| extra | PNG, JPEG, GIF, BMP, TIFF, WebP, ICO | 2,040,187 B | 684,732 B | 513,026 B | 94,211 B | 52,128 B | 2,186,900 B |
| extra delta | six additional top-level codecs | +248,129 B | +95,697 B | +71,906 B | 0 B | 0 B | +248,129 B |

`npm run size` regenerates `pkg/sizes.json` from the current built artifacts.
Raw size is the stable feature-cost comparison; compressed bytes can move by a
small amount between otherwise equivalent linker builds. Generated packages
remain ignored; this document is the retained measurement record.

`npm run test:package` reports the combined publishable package as 1,303,985
bytes compressed and 4,128,078 bytes unpacked. Its 12-entry file manifest
contains both `.wasm` binaries, both JavaScript bindings, both declaration
sets, and the size record. The post-build packaging step removes only
wasm-pack's nested `.gitignore` markers; the repository-level `pkg/` ignore is
retained so generated artifacts are never committed accidentally.

## Acceptance Evidence

- `cargo check -p pillow-rs-js --target wasm32-unknown-unknown --locked`
  passes for the default core lane;
- the same check with `--no-default-features --features wasm-extra` passes;
- `cargo check -p fontdone-ffi-wasm --target wasm32-unknown-unknown --locked`
  passes with the export layer's explicit 64-bit compatibility ABI;
- `npm run build:release` produces both optimized packages;
- `npm run test:package` parses the npm dry-run manifest and requires both
  generated variants in the publishable artifact;
- `npm run test:codecs` runs all nine Pillow-oracle image-backend manifest rows
  against both packages. Enabled formats match exact mode, dimensions, and
  pixel bytes; unavailable formats return a feature-disabled error;
- CI builds and runs the same core and extra codec matrix.

The sibling workspace's strict `-D warnings` lane still exposes its separately
documented pre-existing arithmetic-lint migration. It is not hidden by this
packaging acceptance record.
