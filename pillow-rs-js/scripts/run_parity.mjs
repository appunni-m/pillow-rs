import { readFileSync } from 'node:fs';
import * as wasm from '../pkg/core/pillow_rs_js.js';
import { runWorkflow } from './parity_workflow.mjs';

// The Python side sends the same workflow documents used by the Pillow
// oracle. This host adapter only initializes Node's WASM package; workflow
// dispatch is shared with the browser host in parity_workflow.mjs.
wasm.initSync({
    module: readFileSync(new URL('../pkg/core/pillow_rs_js_bg.wasm', import.meta.url)),
});

async function readStdin() {
    const chunks = [];
    for await (const chunk of process.stdin) chunks.push(chunk);
    return Buffer.concat(chunks).toString('utf8');
}

const input = JSON.parse(await readStdin());

async function probeWebGpu() {
    const gpu = globalThis.navigator?.gpu;
    if (gpu === undefined) {
        return {
            api: 'unavailable',
            adapter: 'not_requested',
            device: 'not_requested',
            shader_dispatch: 'not_measured',
            reason: 'Node does not expose navigator.gpu in this runtime',
        };
    }
    let adapter;
    try {
        adapter = await gpu.requestAdapter({ powerPreference: 'high-performance' });
    } catch (error) {
        return {
            api: 'available',
            adapter: 'error',
            device: 'not_requested',
            shader_dispatch: 'not_measured',
            reason: String(error?.message ?? error),
        };
    }
    if (!adapter) {
        return {
            api: 'available',
            adapter: 'unavailable',
            device: 'not_requested',
            shader_dispatch: 'not_measured',
            reason: 'navigator.gpu.requestAdapter returned no adapter',
        };
    }
    const info = adapter.info ?? {};
    return {
        api: 'available',
        adapter: 'available',
        device: 'not_requested',
        adapter_info: {
            vendor: info.vendor ?? null,
            architecture: info.architecture ?? null,
            device: info.device ?? null,
            description: info.description ?? null,
        },
        shader_dispatch: 'not_measured',
        reason: 'Node workflow uses the CPU/fallback-only WASM package variant',
    };
}

const result = runWorkflow(wasm, input, 'node-wasm');
process.stdout.write(JSON.stringify({
    ...result,
    capabilities: { webgpu: await probeWebGpu() },
}));
