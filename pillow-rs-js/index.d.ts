/**
 * Public declarations for the single pillow-rs npm package. Browser
 * bundlers resolve the generated web module; Node.js resolves node.js, whose
 * no-argument initializer reads the bundled WASM file from disk.
 */
export * from './pkg/core/pillow_rs_js';

import type {
    InitInput,
    InitOutput,
    SyncInitInput,
} from './pkg/core/pillow_rs_js';

/** Initialize from bundled bytes or an optional synchronous WASM input. */
export declare function initSync(
    module?: { module: SyncInitInput } | SyncInitInput,
): InitOutput;

/** Initialize the browser or Node.js package entry for the current runtime. */
export default function init(
    moduleOrPath?:
        | { module_or_path: InitInput | Promise<InitInput> }
        | InitInput
        | Promise<InitInput>,
): Promise<InitOutput>;
