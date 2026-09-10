# pillow-rs

`pillow-rs` provides the `pillow-rs` Rust image API in browsers through
wasm-bindgen. The package contains two import paths:

```js
import init, { Image } from 'pillow-rs';
import initExtra, { Image as ExtraImage } from 'pillow-rs/extra';
```

Both paths expose the same image and font operations and the same complete
codec surface. The `extra` path is retained as a compatibility alias for
applications that used the earlier split bundle. Each generated module must
be initialized before calling its exported operations.

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
