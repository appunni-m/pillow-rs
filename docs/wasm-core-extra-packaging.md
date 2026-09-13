# JavaScript and WASM package

Status: implemented; the single-package contract was reviewed on
2026-09-13.

## Contract

`pillow-rs-js/package.json` is the one public npm manifest and publishes one
complete `wasm-all` module. The package root has environment-aware conditional
exports:

- browser bundlers resolve `pkg/core/pillow_rs_js.js`, whose default
  initializer fetches the adjacent `pillow_rs_js_bg.wasm` asset;
- Node.js resolves `node.js`, which reads that same asset from disk and wraps
  the generated web bindings with a working no-argument `init()` and
  `initSync()`;
- TypeScript resolves the package-level `index.d.ts`, so the import surface is
  identical in both environments.

There is no public `pillow-rs/extra` subpath and no second codec bundle. The
old split output is removed before every build and rejected by the package
check if stale files are still present. AVIF remains outside the browser codec
contract because `image-slash-star` has no AVIF-capable WASM implementation;
valid AVIF input therefore retains the structured disabled/capability boundary
instead of being misclassified as malformed.

The browser runtime and the separate `fontdone` package each own their own
WASM instance. They do not share handles or linear memory.

## Build and package checks

From `pillow-rs-js/`:

```sh
npm ci
npm run build:release
npm run test:package
npm run size
```

`npm run build:release` generates one browser-target directory under `pkg/`
and removes any stale `pkg/extra` directory. `npm run test:package` parses the
`npm pack --dry-run` manifest, requires the browser bindings, Node adapter,
declarations, license, and WASM asset, rejects retired split output, and
initializes the Node entry against the bundled bytes. The maintained root
`make test-wasm` target then runs the same public corpus through Node and a
real browser; the browser lane is the check for fetch-based web initialization.

Generated packages remain ignored. A clean checkout must run the build before
the package check; no ignored `pkg/` directory is a source dependency.

## Size evidence

Run `npm run size` after `npm run build:release` to regenerate
`pkg/sizes.json`. The report records the single web module's raw WASM,
JavaScript glue, declaration, gzip, Brotli, and generated-directory sizes,
plus the browser and Node package entrypoints. Sizes depend on the pinned
Rust/wasm-pack toolchain and optimizer available on the runner, so the
generated report is release evidence rather than a copied number in this
page. The generated directory remains ignored and is not a second package.

## Acceptance evidence

- `cargo check -p pillow-rs-js --target wasm32-unknown-unknown --locked`
  passes for the `wasm-all` binding;
- `npm run build:release` creates one complete browser artifact and the
  tracked Node adapter;
- `npm run test:package` proves the publishable tarball contains one WASM
  payload and that the Node initializer can load it without `fetch(file://…)`;
- `make test-wasm-node` exercises the package's Node conditional export through
  the canonical input corpus;
- `make test-wasm-browser` exercises the generated web entry and adjacent
  browser-served WASM asset through the canonical input corpus;
- the release workflow builds and packs `pillow-rs-js/` from its package root,
  then publishes that one tarball with provenance.

The sibling `fontdone` repository follows the same one-package rule. Its
browser entry remains `index.js`; its Node conditional export uses
`index.node.js` to read the bundled `fontdone.wasm` before delegating to the
shared wrapper. See the [fontdone npm package guide](https://github.com/appunni-m/fontdone/tree/main/fontdone-wasm/npm#readme).
