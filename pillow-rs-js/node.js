import { readFileSync } from 'node:fs';

import * as web from './pkg/core/pillow_rs_js.js';

const bundledWasm = readFileSync(
    new URL('./pkg/core/pillow_rs_js_bg.wasm', import.meta.url),
);
let initialized = false;
let wasmOutput;

function syncInput(module) {
    if (module === undefined) {
        return { module: bundledWasm };
    }
    if (
        module !== null &&
        typeof module === 'object' &&
        Object.prototype.hasOwnProperty.call(module, 'module')
    ) {
        return module;
    }
    return { module };
}

/**
 * Initialize the shared browser-target bindings from the bundled WASM bytes.
 * A caller may pass the same bytes/module accepted by wasm-bindgen's
 * generated `initSync`; omitting the argument uses this package's asset.
 */
export function initSync(module) {
    if (!initialized) {
        wasmOutput = web.initSync(syncInput(module));
        initialized = true;
    }
    return wasmOutput;
}

/**
 * Initialize the package in Node.js. The no-argument path is synchronous in
 * practice because the bundled asset is read from disk; an explicit input is
 * delegated to the generated browser initializer for URL/Response support.
 */
export default async function init(moduleOrPath) {
    if (initialized) {
        return wasmOutput;
    }
    if (moduleOrPath === undefined) {
        return initSync();
    }
    wasmOutput = await web.default(moduleOrPath);
    initialized = true;
    return wasmOutput;
}

// Keep every generated class and function available from the package root.
// The explicit initSync export above takes precedence over the generated one.
export * from './pkg/core/pillow_rs_js.js';
