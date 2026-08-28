import * as wasm from '/pkg/core/pillow_rs_js.js';
import { runWorkflow } from '/scripts/parity_workflow.mjs';

async function withTimeout(promise, milliseconds) {
    let timer;
    try {
        return await Promise.race([
            promise,
            new Promise((_, reject) => {
                timer = setTimeout(() => reject(new Error(`browser capability probe timed out after ${milliseconds}ms`)), milliseconds);
            }),
        ]);
    } finally {
        if (timer !== undefined) clearTimeout(timer);
    }
}

async function probeWebGpu() {
    const apiAvailable = typeof navigator !== 'undefined' && navigator.gpu !== undefined;
    if (!apiAvailable) {
        return {
            api: 'unavailable',
            adapter: 'not_requested',
            device: 'not_requested',
            shader_dispatch: 'not_measured',
            reason: 'navigator.gpu is not exposed by this browser context',
        };
    }
    let adapter;
    try {
        adapter = await withTimeout(
            navigator.gpu.requestAdapter({ powerPreference: 'high-performance' }),
            10000,
        );
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
    let device;
    try {
        device = await withTimeout(adapter.requestDevice(), 10000);
    } catch (error) {
        return {
            api: 'available',
            adapter: 'available',
            device: 'error',
            shader_dispatch: 'not_measured',
            reason: String(error?.message ?? error),
        };
    }
    const info = adapter.info ?? {};
    device.destroy();
    return {
        api: 'available',
        adapter: 'available',
        device: 'available',
        adapter_info: {
            vendor: info.vendor ?? null,
            architecture: info.architecture ?? null,
            device: info.device ?? null,
            description: info.description ?? null,
        },
        shader_dispatch: 'not_measured',
        reason: 'WebGPU is available, but this package variant is WASM CPU/fallback-only',
    };
}

async function main() {
    try {
        const response = await fetch('/__pillow_rs_parity_input__');
        if (!response.ok) throw new Error(`parity input request failed: HTTP ${response.status}`);
        const input = await response.json();
        await wasm.default(new URL('/pkg/core/pillow_rs_js_bg.wasm', document.baseURI));
        const result = runWorkflow(wasm, input, 'browser-wasm');
        window.__PILLOW_RS_BROWSER_PARITY__ = {
            ...result,
            capabilities: { webgpu: await probeWebGpu() },
        };
    } catch (error) {
        window.__PILLOW_RS_BROWSER_PARITY__ = {
            error: {
                class: error?.name ?? 'Error',
                message: String(error?.message ?? error),
                stack: String(error?.stack ?? ''),
            },
        };
    }
}

void main();
