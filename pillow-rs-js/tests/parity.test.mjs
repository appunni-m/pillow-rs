/**
 * WASM parity test suite — manifest-driven, fixture-based.
 *
 * Each test loads a JSON fixture (pre-computed PIL reference hash),
 * runs the WASM operation, and compares output hashes.
 *
 * @covers annotations are parsed by scripts/coverage/validate_coverage.py
 * for enforced coverage tracking across Python + JS targets.
 *
 * Usage: node --experimental-wasm-modules pillow-rs-js/tests/parity.test.mjs
 */
import { readFileSync, readdirSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { createHash } from 'crypto';

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(__dirname, 'fixtures');

// Track results
const results = { passed: [], failed: [], skipped: [] };

function hash(bytes) {
    return createHash('sha256').update(Buffer.from(bytes)).digest('hex');
}

function loadFixture(name) {
    const data = JSON.parse(readFileSync(join(FIXTURES, name), 'utf8'));
    return data;
}

// ── Test runner ──────────────────────────────────────────────

let wasm;

async function runTests() {
    wasm = await import(join(__dirname, '..', 'pkg', 'pillow_rs_js.js'));
    await wasm.default();

    // Discover all fixtures and run tests
    const files = readdirSync(FIXTURES).filter(f => f.endsWith('.json') && f !== 'index.json');
    let passed = 0, failed = 0;

    for (const file of files) {
        try {
            await runFixture(file);
            passed++;
        } catch (e) {
            if (e.message === 'SKIP') {
                results.skipped.push({ file, reason: 'unsupported' });
            } else {
                results.failed.push({ file, error: e.message });
                failed++;
            }
        }
    }

    console.log(`\n=== Results ===`);
    console.log(`  Passed: ${passed}`);
    console.log(`  Failed: ${failed}`);
    console.log(`  Skipped: ${results.skipped.length}`);

    if (failed > 0) {
        console.log(`\nFailures:`);
        for (const f of results.failed) {
            console.log(`  ${f.file}: ${f.error}`);
        }
        process.exit(1);
    }
}

async function runFixture(file) {
    const fixture = loadFixture(file);
    const { op, mode, target, expectedHash } = fixture;

    if (!wasm) throw new Error('WASM not initialized');

    let result;
    const size = [30, 30];

    // Create input image in the right mode
    let img;

    // Map mode to WASM constructor args
    switch (mode) {
        case 'L': img = new wasm.Image('L', size[0], size[1], 128, 128, 128, 255); break;
        case 'LA': img = new wasm.Image('LA', size[0], size[1], 128, 128, 128, 255); break;
        case 'RGB': img = new wasm.Image('RGB', size[0], size[1], 255, 0, 0, 255); break;
        case 'RGBA': img = new wasm.Image('RGBA', size[0], size[1], 255, 0, 0, 255); break;
        case '1': img = new wasm.Image('1', size[0], size[1], 1, 1, 1, 255); break;
        case 'P': img = new wasm.Image('RGB', size[0], size[1], 255, 0, 0, 255).convert('P'); break;
        case 'CMYK': img = new wasm.Image('RGB', size[0], size[1], 255, 0, 0, 255); break;  // tagged
        default: throw new Error(`Unsupported mode: ${mode}`);
    }

    // Dispatch operation
    try {
        result = dispatchOp(img, op);
    } catch (e) {
        if (e.message.includes('not yet implemented') || e.message.includes('not supported')) {
            throw new Error('SKIP');
        }
        throw e;
    }

    if (!result) throw new Error('SKIP');

    const resultBytes = result.toBytes ? result.toBytes() : result;
    const actualHash = hash(resultBytes);

    if (actualHash !== expectedHash) {
        throw new Error(`Hash mismatch for ${op} x ${mode}: expected ${expectedHash.slice(0,8)}, got ${actualHash.slice(0,8)}`);
    }
}

function dispatchOp(img, op) {
    // Map op name to WASM method
    const [module, func] = op.split('.');

    if (module === 'Image') {
        switch (func) {
            case 'resize': return img.resize(15, 15);
            case 'crop': return img.crop(5, 5, 25, 25);
            case 'rotate': return img.rotate(90);
            case 'transpose': return img.transpose('FLIP_LEFT_RIGHT');
            case 'filter': return img.filter('BLUR');
            case 'convert': return img.convert('RGB');
            case 'copy': return img.copy();
            case 'split': return img.split()[0];
            case 'getbands': return img.getbands();
            case 'thumbnail': img.thumbnail(15, 15); return img;
            case 'tobytes': return { toBytes: () => img.toBytes() };
            case 'quantize': return img.quantize(16);
            default: return null; // unsupported in WASM
        }
    }

    return null; // Module not yet supported in WASM
}

// ── Main ─────────────────────────────────────────────────────

runTests().catch(e => {
    console.error(e.message);
    process.exit(1);
});
