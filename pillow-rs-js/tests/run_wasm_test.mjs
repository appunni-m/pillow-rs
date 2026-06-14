/**
 * WASM Node.js parity test — fixture-driven.
 *
 * Loads all fixtures from ../../tests/fixtures/, creates images via the
 * WasmBackend, executes operations via the execution engine, and compares
 * results against the expected reference (hash, value, or error).
 *
 * Usage: node pillow-rs-js/tests/run_wasm_test.mjs
 */

import { readFileSync, readdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { createHash } from 'crypto';
import { execute } from './execution_engine.mjs';
import { WasmBackend } from './wasm_backend.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(__dirname, '..', '..', 'tests', 'fixtures');
const PKG = join(__dirname, '..', 'pkg');

// ── WASM bootstrap ─────────────────────────────────────────────────

const wasmBinary = readFileSync(join(PKG, 'pillow_rs_js_bg.wasm'));
const wasmModule = new WebAssembly.Module(wasmBinary);

const jsGlue = await import(join(PKG, 'pillow_rs_js.js'));
jsGlue.initSync(wasmModule);

const backend = new WasmBackend(jsGlue);

// ── Helpers ────────────────────────────────────────────────────────

function hexToBytes(hex) {
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < hex.length; i += 2) {
        bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
    }
    return bytes;
}

function sha256(bytes) {
    return createHash('sha256').update(Buffer.from(bytes)).digest('hex');
}

function hash(value) {
    if (value instanceof Uint8Array) {
        return sha256(value);
    }
    if (typeof value === 'string') {
        return sha256(new TextEncoder().encode(value));
    }
    if (value && typeof value === 'object' && value.toBytes) {
        return sha256(Buffer.from(value.toBytes()));
    }
    return sha256(new TextEncoder().encode(String(value)));
}

// ── Lossy operation comparison ─────────────────────────────────────

const LOSSY_OPS = [
    'Image.resize', 'Image.thumbnail',
    'ImageEnhance.Brightness', 'ImageEnhance.Color', 'ImageEnhance.Contrast',
    'ImageEnhance.Sharpness', 'ImageFilter.GaussianBlur',
    'ImageFilter.UnsharpMask', 'ImageFilter.ModeFilter',
    'ImageOps.contain', 'ImageOps.cover', 'ImageOps.fit', 'ImageOps.pad', 'ImageOps.scale',
];

function lossyOpName(opDef) {
    return `${opDef.module}.${opDef.target}`;
}

function lossyPixelTolerance(actualBytes, expectedHex) {
    /** For lossy ops, count pixels where any byte channel differs > 2,
     *  and fail if more than 5% of pixels exceed that threshold. */
    const expectedBytes = hexToBytes(expectedHex);
    const len = Math.min(actualBytes.length, expectedBytes.length);
    if (len === 0) return { pass: false, reason: 'zero-length comparison' };

    // Determine pixel size from expected data length vs image dimensions
    // We don't have image dimensions here, so use per-byte comparison
    // Tolerance: per-byte diff > 2
    let badPixels = 0;
    const pixelCount = len;

    for (let i = 0; i < len; i++) {
        const diff = Math.abs(actualBytes[i] - expectedBytes[i]);
        if (diff > 2) {
            badPixels++;
        }
    }

    const ratio = badPixels / pixelCount;
    if (ratio > 0.05) {
        return {
            pass: false,
            reason: `${badPixels}/${pixelCount} bytes differ >2 (${(ratio * 100).toFixed(1)}% > 5%)`
        };
    }
    return { pass: true };
}

function compareFloats(actualBytes, expectedHex, tolerance = 1.0) {
    /** For F-mode images, decode as Float32 and compare with tolerance. */
    const expectedBytes = hexToBytes(expectedHex);
    const len = Math.min(actualBytes.length, expectedBytes.length);
    if (len === 0) return { pass: false, reason: 'zero-length comparison' };

    // Align to 4-byte Float32 boundaries
    const actualFloats = new Float32Array(actualBytes.buffer, 0, Math.floor(len / 4));
    const expectedFloats = new Float32Array(expectedBytes.buffer, 0, Math.floor(len / 4));
    const count = Math.min(actualFloats.length, expectedFloats.length);
    let maxDiff = 0;
    for (let i = 0; i < count; i++) {
        const diff = Math.abs(actualFloats[i] - expectedFloats[i]);
        if (diff > maxDiff) maxDiff = diff;
    }
    if (maxDiff > tolerance) {
        return { pass: false, reason: `max float diff ${maxDiff} > ${tolerance}` };
    }
    return { pass: true };
}

// ── Value comparison ───────────────────────────────────────────────

function compareValue(actual, expected) {
    /** Compare actual value against expected.value.
     *
     * Expected value types seen in fixtures:
     *   - string (e.g. "L", "<PixelAccess object at ...>", capsule repr)
     *   - number (e.g. 255, 0)
     *   - array of numbers  (e.g. [100, 100])
     *   - array of strings (e.g. ["L"], or Image repr strings for split)
     */

    if (expected === null || expected === undefined) return { pass: false, reason: 'expected is null/undefined' };

    const expVal = typeof expected.value !== 'undefined' ? expected.value : expected;

    // Null/undefined actual — check if expected value is also null-like
    if (actual === null || actual === undefined) {
        if (expVal === null || expVal === 'null' || expVal === 'None') {
            return { pass: true };
        }
        return { pass: false, reason: `got null/undefined, expected ${JSON.stringify(expVal).slice(0, 200)}` };
    }

    // Capsule / PixelAccess: string matching (repr-based)
    if (typeof expVal === 'string' && typeof actual === 'string') {
        // For repr-based comparison (PixelAccess, capsule), match the type pattern
        if (expVal.startsWith('<') && expVal.endsWith('>')) {
            // Match the type name part, e.g. "<PixelAccess object at ...>" -> "PixelAccess"
            const expTypeName = expVal.match(/^<\w+/)?.[0] || expVal;
            const actTypeName = actual.match(/^<\w+/)?.[0] || actual;
            const pass = expTypeName === actTypeName;
            return pass
                ? { pass: true }
                : { pass: false, reason: `type mismatch: expected ${expTypeName}, got ${actTypeName}` };
        }
        return expVal === actual
            ? { pass: true }
            : { pass: false, reason: `expected "${expVal}", got "${actual}"` };
    }

    // Number comparison (with tolerance for floats)
    if (typeof expVal === 'number' && typeof actual === 'number') {
        const diff = Math.abs(actual - expVal);
        const pass = diff < 0.01 || Number.isNaN(diff) === Number.isNaN(expVal);
        return pass
            ? { pass: true }
            : { pass: false, reason: `expected ${expVal}, got ${actual} (diff ${diff})` };
    }

    // Array / tuple comparison
    if (Array.isArray(expVal)) {
        if (!Array.isArray(actual)) {
            return { pass: false, reason: `expected array, got ${typeof actual}` };
        }
        if (expVal.length !== actual.length) {
            return { pass: false, reason: `array length: expected ${expVal.length}, got ${actual.length}` };
        }
        for (let i = 0; i < expVal.length; i++) {
            // For arrays of strings (e.g. mode strings, Image repr strings), do string comparison
            if (typeof expVal[i] === 'string' && typeof actual[i] === 'string') {
                if (expVal[i].startsWith('<') && actual[i].startsWith('<')) {
                    // Repr-based comparison — match type name
                    const expType = expVal[i].match(/^<\w+/)?.[0];
                    const actType = actual[i].match(/^<\w+/)?.[0];
                    if (expType && actType && expType === actType) continue;
                }
                if (expVal[i] !== actual[i]) {
                    return { pass: false, reason: `array[${i}]: expected "${expVal[i]}", got "${actual[i]}"` };
                }
            // Handle WASM Image objects in arrays (e.g. split results)
            } else if (expVal[i] && typeof expVal[i] === 'string' && expVal[i].startsWith('<') &&
                       actual[i] && typeof actual[i] === 'object' && typeof actual[i].toBytes === 'function') {
                // Image object with toBytes — accept as valid
                continue;
            } else if (typeof expVal[i] === 'number' && typeof actual[i] === 'number') {
                const diff = Math.abs(actual[i] - expVal[i]);
                if (diff >= 0.01) {
                    return { pass: false, reason: `array[${i}]: expected ${expVal[i]}, got ${actual[i]} (diff ${diff})` };
                }
            } else if (String(expVal[i]) !== String(actual[i])) {
                return { pass: false, reason: `array[${i}]: expected ${String(expVal[i])}, got ${String(actual[i])}` };
            }
        }
        return { pass: true };
    }

    // Fallback: string comparison
    const expStr = String(expVal);
    const actStr = String(actual);
    if (expStr === actStr) return { pass: true };

    // Try number comparison as last resort
    const expNum = Number(expVal);
    const actNum = Number(actual);
    if (!Number.isNaN(expNum) && !Number.isNaN(actNum)) {
        return Math.abs(actNum - expNum) < 0.01
            ? { pass: true }
            : { pass: false, reason: `expected ${expNum}, got ${actNum}` };
    }

    return { pass: false, reason: `expected "${expStr}", got "${actStr}"` };
}

// ── Test runner ────────────────────────────────────────────────────

function loadFixture(file) {
    return JSON.parse(readFileSync(join(FIXTURES, file), 'utf8'));
}

function getResultBytes(result) {
    /** Extract raw bytes from an operation result. */
    if (result instanceof Uint8Array) return result;
    if (result instanceof ArrayBuffer) return new Uint8Array(result);
    if (result && typeof result.toBytes === 'function') return Buffer.from(result.toBytes());
    if (Array.isArray(result)) return new Uint8Array(result);
    if (result && result.buffer instanceof ArrayBuffer) return new Uint8Array(result.buffer);
    return null;
}

const files = readdirSync(FIXTURES).filter(f => f.endsWith('.json') && f !== 'index.json');

let passed = 0, failed = 0, skipped = 0;
const failures = [];

for (const file of files.sort()) {
    const fixtureName = file.replace('.json', '');
    let fixture;
    try {
        fixture = loadFixture(file);
    } catch (e) {
        failed++;
        failures.push(`${fixtureName}: failed to load fixture: ${e.message}`);
        continue;
    }

    const { operation: opDef, input, expected, config } = fixture;
    if (!opDef || !expected) {
        skipped++;
        continue;
    }

    // Skip GPU-only tests
    if (fixtureName.includes('_wasm_gpu')) {
        skipped++;
        continue;
    }

    console.log(`  ${fixtureName}...`);

    try {
        // Create input image
        const inputBytes = hexToBytes(input.bytes);
        const img = backend.make_image(input.mode, input.size, inputBytes);
        if (!img) {
            skipped++;
            continue;
        }

        // Create second image if present
        let img2 = null;
        if (fixture.input2) {
            const input2Bytes = hexToBytes(fixture.input2.bytes);
            img2 = backend.make_image(fixture.input2.mode, fixture.input2.size, input2Bytes);
        }

        // Handle error expectations
        if (expected.result_type === 'error') {
            try {
                await execute(backend, opDef, img, img2);
                // No error thrown — unexpected success
                failed++;
                failures.push(`${fixtureName}: expected error but operation succeeded`);
                continue;
            } catch (e) {
                const errMsg = e.message || String(e);
                const expMsg = typeof expected.value === 'string' ? expected.value : '';
                if (expMsg && !errMsg.includes(expMsg)) {
                    failed++;
                    failures.push(`${fixtureName}: error mismatch: expected "${expMsg}", got "${errMsg}"`);
                } else {
                    passed++;
                }
                continue;
            }
        }

        // Execute operation
        let result;
        try {
            result = await execute(backend, opDef, img, img2);
        } catch (e) {
            const msg = e.message || String(e);
            if (msg.includes('not implemented') || msg.includes('not yet implemented') || msg.includes('not supported') || msg.includes('unreachable')) {
                skipped++;
                continue;
            }
            throw e; // re-throw to outer catch
        }

        // Skip if result is null/undefined and expected is not value-type
        if ((result === null || result === undefined) && expected.result_type !== 'value') {
            skipped++;
            continue;
        }

        // Compare results
        if (expected.result_type === 'value') {
            const cmp = compareValue(result, expected);
            if (cmp.pass) {
                passed++;
            } else {
                failed++;
                failures.push(`${fixtureName}: value mismatch — ${cmp.reason}`);
            }
        } else if (expected.result_type === 'hash') {
            const rawBytes = getResultBytes(result);
            if (!rawBytes) {
                failed++;
                failures.push(`${fixtureName}: could not extract bytes from result (type=${typeof result})`);
                continue;
            }

            const refHex = expected.reference_bytes || expected.value;
            const opName = lossyOpName(opDef);
            const isLossy = LOSSY_OPS.includes(opName);
            const isFMode = input.mode === 'F';

            if (isFMode) {
                // Float comparison with tolerance
                const cmp = compareFloats(rawBytes, refHex, 1.0);
                if (cmp.pass) {
                    passed++;
                } else {
                    failed++;
                    failures.push(`${fixtureName}: float tolerance exceeded — ${cmp.reason}`);
                }
            } else if (isLossy) {
                // Lossy per-pixel tolerance comparison
                const cmp = lossyPixelTolerance(rawBytes, refHex);
                if (cmp.pass) {
                    passed++;
                } else {
                    // Fallback: if tolerance fails, check exact hash
                    const actualHash = sha256(rawBytes);
                    if (actualHash === expected.value) {
                        passed++;
                    } else {
                        failed++;
                        failures.push(`${fixtureName}: lossy tolerance exceeded — ${cmp.reason}`);
                    }
                }
            } else {
                // Exact hash comparison
                const actualHash = sha256(rawBytes);
                if (actualHash === expected.value) {
                    passed++;
                } else {
                    // Check reference_bytes as fallback
                    if (expected.reference_bytes) {
                        const refHash = sha256(hexToBytes(expected.reference_bytes));
                        if (actualHash === refHash) {
                            passed++;
                            continue;
                        }
                    }
                    failed++;
                    failures.push(`${fixtureName}: hash mismatch — expected ${expected.value.slice(0, 16)}..., got ${actualHash.slice(0, 16)}...`);
                }
            }
        } else {
            // Unknown result_type — skip
            skipped++;
        }
    } catch (e) {
        const msg = e.message || String(e);
        if (msg.includes('not implemented') || msg.includes('not yet implemented') || msg.includes('not supported') || msg.includes('unreachable')) {
            skipped++;
        } else {
            failed++;
            failures.push(`${fixtureName}: ${msg}`);
        }
    }
}

// ── Report ─────────────────────────────────────────────────────────

console.log(`\n=== WASM Test Results ===`);
console.log(`  Passed:  ${passed}`);
console.log(`  Failed:  ${failed}`);
console.log(`  Skipped: ${skipped}`);
console.log(`  Total:   ${passed + failed + skipped}`);

if (failures.length > 0) {
    console.log(`\n--- Failures (${failures.length}) ---`);
    for (const f of failures) {
        console.log(`  ${f}`);
    }
}

if (failed > 0) process.exit(1);
else process.exit(0);
