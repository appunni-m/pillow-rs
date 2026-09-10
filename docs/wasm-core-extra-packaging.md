# JS/WASM Core And Extra Packaging

Status: implemented; the current package contract was reviewed on
2026-09-11.

## Contract

The npm package exposes two independently built wasm-bindgen artifacts:

- `pillow-rs` (the default export) loads `pkg/core`;
- `pillow-rs/extra` loads `pkg/extra`.

Both artifacts currently use the `wasm-all` Cargo feature and therefore expose
the same Pillow operation, font, and complete image-codec surface. The
`extra` path is retained as a compatibility alias for consumers of the former
split bundle; it is not a reduced or expanded codec build. AVIF is in neither
browser artifact because `image-slash-star` has no AVIF-capable WASM
implementation; valid AVIF input therefore retains the structured
disabled/capability boundary instead of being misclassified as malformed.

The browser runtime and the separate `fontdone-wasm` export layer enable
`fontdone/wasm-wide-internals`. This preserves the LP64 FreeType arithmetic
used by the pinned native Pillow oracle and matches the export layer's explicit
64-bit `FT_Long`/`FT_ULong` compatibility ABI. Native targets continue to use
their platform C aliases.

## Release sizes

Run `npm run size` after `npm run build:release` to regenerate
`pkg/sizes.json`. The report records raw WASM, JavaScript glue, declaration,
gzip, Brotli, and generated-directory sizes for both variants. The two rows
should currently have the same codec feature list and therefore the same
sizes; any difference is a build regression that needs investigation. Sizes
depend on the Rust/wasm-pack toolchain and optimizer available on the runner,
so the generated report is the release evidence rather than a copied number in
this page. Generated packages remain ignored.

`npm run test:package` parses the current dry-run manifest and requires the
package README, MIT-CMU license, both `.wasm` binaries, both JavaScript
bindings, both declaration sets, and the size record. The post-build packaging
step removes only wasm-pack's nested `.gitignore` markers; the repository-level
`pkg/` ignore is retained so generated artifacts are never committed
accidentally.

## Acceptance Evidence

- `cargo check -p pillow-rs-js --target wasm32-unknown-unknown --locked`
  passes for the default core lane;
- the same check with `--no-default-features --features wasm-extra` passes;
- `cargo check -p fontdone-wasm --target wasm32-unknown-unknown --locked`
  passes with the export layer's explicit 64-bit compatibility ABI;
- `npm run build:release` produces both optimized packages;
- `npm run test:package` parses the npm dry-run manifest and requires both
  generated variants in the publishable artifact;
- the maintained root `make test-wasm` target runs the canonical Node and
  browser parity corpus against both generated packages. Enabled formats
  match exact mode, dimensions, and pixel bytes; unavailable formats return a
  feature-disabled error;
- CI builds and runs the same core and extra codec matrix.

The sibling workspace's strict `-D warnings` lane still exposes its separately
documented pre-existing arithmetic-lint migration. It is not hidden by this
packaging acceptance record.
