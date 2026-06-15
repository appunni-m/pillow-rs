/**
 * List every skipped test with its reason.
 */
import { readFileSync, readdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { execute } from './execution_engine.mjs';
import { WasmBackend } from './wasm_backend.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(__dirname, '..', '..', 'tests', 'fixtures');
const PKG = join(__dirname, '..', 'pkg');

const wasmBinary = readFileSync(join(PKG, 'pillow_rs_js_bg.wasm'));
const wasmModule = new WebAssembly.Module(wasmBinary);
const jsGlue = await import(join(PKG, 'pillow_rs_js.js'));
jsGlue.initSync(wasmModule);
const backend = new WasmBackend(jsGlue);

function hexToBytes(hex) {
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < hex.length; i += 2) {
        bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
    }
    return bytes;
}

const files = readdirSync(FIXTURES).filter(f => f.endsWith('.json') && f !== 'index.json');
const skips = {};

for (const file of files.sort()) {
    const fixtureName = file.replace('.json', '');
    const fixture = JSON.parse(readFileSync(join(FIXTURES, file), 'utf8'));
    const { operation: opDef, input, expected } = fixture;
    if (!opDef || !expected) continue;

    try {
        const inputBytes = hexToBytes(input.bytes);
        const img = backend.make_image(input.mode, input.size, inputBytes);
        if (!img) {
            skips[fixtureName] = 'make_image returned null';
            continue;
        }

        let img2 = null;
        if (fixture.input2) {
            const input2Bytes = hexToBytes(fixture.input2.bytes);
            img2 = backend.make_image(fixture.input2.mode, fixture.input2.size, input2Bytes);
        }

        if (expected.result_type === 'error') {
            continue; // expected errors are handled separately
        }

        try {
            const result = await execute(backend, opDef, img, img2);
            // If it succeeded, it's not skipped
        } catch (e) {
            const msg = e.message || String(e);
            if (msg.includes('not implemented') || msg.includes('not supported') || msg.includes('not yet implemented') || msg.includes('unreachable')) {
                const reason = msg.replace(/^not implemented:\s*/, '').replace(/^not yet implemented:\s*/, '').trim();
                skips[fixtureName] = reason;
            }
        }
    } catch (e) {
        const msg = e.message || String(e);
        skips[fixtureName] = `outer: ${msg}`;
    }
}

// Group by reason
const byReason = {};
for (const [name, reason] of Object.entries(skips)) {
    const key = reason.slice(0, 100);
    if (!byReason[key]) byReason[key] = [];
    byReason[key].push(name);
}

console.log(`Total skipped: ${Object.keys(skips).length}\n`);
for (const [reason, names] of Object.entries(byReason).sort((a,b) => b[1].length - a[1].length)) {
    console.log(`[${names.length}x] ${reason}`);
    // Only print first few example names per reason
    for (const n of names.slice(0, 3)) {
        console.log(`       ${n}`);
    }
    if (names.length > 3) console.log(`       ... and ${names.length - 3} more`);
    console.log();
}
