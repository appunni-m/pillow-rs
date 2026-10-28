import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import {
    closeSync,
    existsSync,
    openSync,
    readFileSync,
    unlinkSync,
    writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
    DisplayViewer,
    EogViewer,
    GmDisplayViewer,
    ImageShow,
    MacViewer,
    WindowsViewer,
    XDGViewer,
    XVViewer,
} from './public.js';

import * as web from './pkg/core/pillow_rs_js.js';

const bundledWasm = readFileSync(
    new URL('./pkg/core/pillow_rs_js_bg.wasm', import.meta.url),
);
let initialized = false;
let wasmOutput;

ImageShow._setRuntime({
    writeTemp(bytes, suffix = '') {
        const path = join(tmpdir(), `pillow-rs-${randomUUID()}${suffix}`);
        const descriptor = openSync(path, 'wx', 0o600);
        try {
            writeFileSync(descriptor, Buffer.from(bytes));
        } finally {
            closeSync(descriptor);
        }
        return path;
    },
    exists: existsSync,
    readFile(path) { return new Uint8Array(readFileSync(path)); },
    remove(path) {
        try { unlinkSync(path); } catch (error) {
            if (error?.code !== 'ENOENT') throw error;
        }
    },
    Popen(args, options = {}) {
        const child = typeof args === 'string'
            ? spawn(args, { shell: options.shell ?? false, stdio: 'ignore' })
            : spawn(args[0], args.slice(1), { stdio: 'ignore', windowsHide: true });
        child.unref();
        return child;
    },
    call(args) {
        const result = spawnSync(args[0], args.slice(1), { stdio: 'ignore' });
        if (result.error) throw result.error;
        return result.status ?? 0;
    },
    system(command) {
        const result = spawnSync(command, { shell: true, stdio: 'ignore' });
        if (result.error) throw result.error;
        return result.status ?? 0;
    },
    pythonExecutable: null,
    scheduleRemove(path, delay) {
        const timer = setTimeout(() => {
            try { unlinkSync(path); } catch (error) {
                if (error?.code !== 'ENOENT') throw error;
            }
        }, delay);
        timer.unref();
    },
});

function commandAvailable(command) {
    const locator = process.platform === 'win32' ? 'where.exe' : 'which';
    return spawnSync(locator, [command], { stdio: 'ignore' }).status === 0;
}

if (process.platform === 'win32') {
    ImageShow.register(WindowsViewer);
} else if (process.platform === 'darwin') {
    ImageShow.register(MacViewer);
} else {
    for (const [command, ViewerType] of [
        ['xdg-open', XDGViewer],
        ['display', DisplayViewer],
        ['gm', GmDisplayViewer],
        ['eog', EogViewer],
        ['xv', XVViewer],
    ]) {
        if (commandAvailable(command)) ImageShow.register(ViewerType);
    }
}

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
export * from './public.js';
