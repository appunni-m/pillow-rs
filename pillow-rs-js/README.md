# pillow-rs for Node.js and browsers

<!-- release:summary -->
**Latest release: [12.2.0-alpha.1](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.0-alpha.1).**
<!-- /release:summary -->

Install the single npm package for both environments:

<!-- release:npm -->
```sh
npm install pillow-rs@12.2.0-alpha.1
```
<!-- /release:npm -->

This is an ES module package. It declares Node.js 20+; CI exercises Node.js
22.14.0. Browser use requires WebAssembly and a bundler or module server that
serves the emitted WASM asset.

## First image

Save this as an ES module in the project where you installed the package:

```javascript
import init, { Image } from "pillow-rs";

await init();
const image = new Image("RGB", 3, 2, 255, 12, 34, 255);
try {
  const resized = image.resize(6, 4);
  try {
    console.log([...resized.size()]); // [6, 4]
    console.log(resized.toBytes().length); // 72
  } finally {
    resized.free();
  }
} finally {
  image.free();
}
```

The constructor takes mode, width, height, and RGBA color components.
`size()` returns a `Uint32Array`; `toBytes()` returns raw image bytes as a
`Uint8Array`, not an encoded image file. The generated TypeScript declarations
define the JavaScript names and argument types.

## Initialization and browser assets

Node resolves `node.js` and loads the bundled WASM bytes from disk. A browser
bundler resolves the generated web module. Call and await `init()` before
constructing images in either environment.

If your bundler relocates the WASM asset, pass the emitted URL through the
initializer's `module_or_path` option. Serve the asset with the correct URL
and a WebAssembly-compatible response; an HTML fallback page is not a WASM
module. Do not ship private `pkg/` imports as the application's package API.

The JavaScript
API is not Python syntax: for example, use `resize(width, height)` and
`toBytes()`. See [compatibility](../docs/COMPATIBILITY.md) for the covered
scope. AVIF is outside the current browser codec contract.

## Memory and errors

WASM-backed objects own allocations. Release them with `free()` when finished,
including on errors; never call methods after freeing an object. New image
results need their own cleanup. Byte conversion can allocate or copy.

Operations can throw for invalid modes, unsupported operations, or invalid
inputs. Report the smallest call sequence, browser/Node version, dimensions,
mode, and package version when opening an issue.

## Upgrading

Version 12.2.0-alpha.1 keeps the same package name and environment
selection. Its `transform(size, matrix)` helper now uses the same affine input
path as `transformWithInput`. A size must have exactly two entries. Omitting
fill uses Pillow's zeroed-pixel default; specify a fill with `transformWithInput`
when opaque border pixels are required.
