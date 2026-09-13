# pillow-rs

`pillow-rs` provides the Rust image API through one npm package. Its package
exports select the browser/WASM entry for browser bundlers and a Node entry
for Node.js. Both entries use the same complete codec surface and the same
WASM module.

```js
import init, { Image } from 'pillow-rs';
await init();

const image = new Image('RGB', 2, 2, 0, 0, 0, 255);
```

In a browser, `init()` fetches the package's `pillow_rs_js_bg.wasm` asset
relative to the generated module. In Node.js, the conditional `node` export
reads that same bundled asset from disk, so the no-argument initializer works
without a `file://` fetch. `initSync()` is also available in Node.js for code
that needs synchronous setup. Pass an explicit URL, `Response`, bytes, or
compiled module to `init()` when an application owns the browser asset path.

The former `pillow-rs/extra` subpath is not part of the release contract. A
fresh build produces one publishable WASM payload and one package entrypoint
per environment.

The package is generated from the Rust workspace. `pkg/` is build output and
is intentionally absent from source control. The supported local checks are:

```sh
npm ci
npm run build:release
npm run test:package
```

The package does not include filesystem APIs or native dependencies. AVIF is
not part of the browser codec contract at this time. See the project
documentation and release notes for the current capability and parity scope:
<https://github.com/appunni-m/pillow-rs>.

This package is licensed under the MIT-CMU License. The complete terms are in
[`LICENSE`](LICENSE).
