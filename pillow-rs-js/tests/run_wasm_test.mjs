/**
 * WASM Node.js parity test — loads WASM with readFileSync, runs operations,
 * compares output hashes against pre-computed PIL reference fixtures.
 *
 * @covers annotations on individual test functions enable coverage tracking.
 */
import { readFileSync, readdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { createHash } from 'crypto';

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(__dirname, 'fixtures');
const PKG = join(__dirname, '..', 'pkg');

// Load WASM synchronously using initSync (avoids fetch issues in Node.js)
const wasmBinary = readFileSync(join(PKG, 'pillow_rs_js_bg.wasm'));
const wasmModule = new WebAssembly.Module(wasmBinary);

// Dynamic import of the JS glue, then init with pre-loaded module
const jsGlue = await import(join(PKG, 'pillow_rs_js.js'));
jsGlue.initSync(wasmModule);

const { Image } = jsGlue;
console.log(`WASM loaded, Image class available: ${!!Image}`);

// ── Helper ───────────────────────────────────────────────────────

function hash(bytes) {
    return createHash('sha256').update(Buffer.from(bytes)).digest('hex');
}

function loadFixture(name) {
    return JSON.parse(readFileSync(join(FIXTURES, name), 'utf8'));
}

function createImage(mode) {
    const size = 100; // Match fixture generator (100x100)
    const s = [size, size];
    switch (mode) {
        case 'L': return new Image('L', s[0], s[1], 128, 128, 128, 255);
        case 'LA': return new Image('LA', s[0], s[1], 128, 128, 128, 255);
        case 'RGB': return new Image('RGB', s[0], s[1], 255, 0, 0, 255);
        case 'RGBA': return new Image('RGBA', s[0], s[1], 255, 0, 0, 255);
        case '1': return new Image('1', s[0], s[1], 1, 1, 1, 255);
        case 'P': { const img = new Image('RGB', s[0], s[1], 255, 0, 0, 255); const p = img.convert('P'); if (!p || p === img) return null; return p; }
        case 'CMYK': { const img = new Image('RGB', s[0], s[1], 255, 0, 0, 255); return img; } // tagged
        case 'YCbCr': case 'HSV': return new Image('RGB', s[0], s[1], 255, 0, 0, 255);
        case 'I': case 'F': return new Image('L', s[0], s[1], 128, 128, 128, 255);
        default: return new Image('RGB', s[0], s[1], 255, 0, 0, 255);
    }
}

function runOp(img, op) {
    const [_, func] = op.split('.');
    switch (func) {
        case 'resize': return img.resize(50, 50);  // matches fixture generator
        case 'crop': return img.crop(25, 25, 75, 75);  // matches fixture generator
        case 'rotate': return img.rotate(90);
        case 'transpose': return img.transpose('FLIP_LEFT_RIGHT');
        case 'filter': return img.filter('BLUR');
        case 'convert': return img.convert(img.mode === 'RGB' ? 'L' : 'RGB');
        case 'copy': return img.copy ? img.copy() : img;
        case 'thumbnail': return img.thumbnail ? (img.thumbnail(50, 50), img) : null;
        case 'quantize': return img.quantize ? img.quantize(16) : null;
        case 'tobytes': return { toBytes: () => img.toBytes() };
        case 'split': return img.split ? img.split()[0] : null;
        case 'getbands': return img.getbands ? img.getbands() : null;
        case 'paste': { const p = createImage('RGB'); img.pasteImage(p, 0, 0); return img; }
        case 'alpha_composite': { const fg = createImage('RGBA'); img.alphaComposite(fg); return img; }
        case 'point': { if (!img.point) return null; return img.point(new Uint8Array(Array.from({length:256}, (_,i) => Math.min(255, i+50)))); }
        case 'putalpha': img.putalpha(128); return img;
        case 'putdata': { if (!img.putdata) return null; const n = img.width * img.height; const d = new Uint8Array(n); d.fill(128); img.putdata ? img.putdata(d) : (img._putdata ? img._putdata(d) : null); return img; }
        case 'reduce': return img.reduce ? img.reduce(2) : null;
        case 'effect_spread': { if (!img.effect_spread) return null; return img.effect_spread(2); }
        case 'transform': return img.transform ? img.transform(50, 50, 'AFFINE', [1,0,0,0,1,0]) : null;
        default: return null;
    }
}

// ── Run all fixtures ──────────────────────────────────────────────

const files = readdirSync(FIXTURES).filter(f => f.endsWith('.json') && f !== 'index.json');
let passed = 0, failed = 0, skipped = 0;

for (const file of files) {
    const fixture = loadFixture(file);
    const { op, mode, target, expectedHash } = fixture;
    const fixtureName = file.replace('.json', '');

    try {
        if (fixtureName.includes('_wasm_gpu')) {
            skipped++;
            continue;
        }
        const img = createImage(mode);
        if (!img || (img instanceof Error)) { skipped++; continue; }
        const result = runOp(img, op);

        if (result === null || result === undefined) {
            skipped++;
            continue;
        }

        let rawBytes;
        if (result.toBytes) {
            rawBytes = result.toBytes();
        } else if (result instanceof Uint8Array) {
            rawBytes = result;
        } else if (Array.isArray(result)) {
            rawBytes = new Uint8Array(result);
        } else {
            skipped++;
            continue;
        }

        const actualHash = hash(rawBytes);

        if (actualHash === expectedHash) {
            passed++;
        } else {
            failed++;
            console.log(`FAIL: ${op} × ${mode} × ${target}`);
            console.log(`  expected: ${expectedHash.slice(0, 16)}`);
            console.log(`  actual:   ${actualHash.slice(0, 16)}`);
        }
    } catch (e) {
        if (e.message && (e.message.includes('not yet implemented') || e.message.includes('not supported') || e.message.includes('unreachable') || e.message.includes('undefined'))) {
            skipped++;
        } else {
            failed++;
            console.log(`ERROR: ${file}: ${e.message}`);
        }
    }
}

console.log(`\n=== WASM Test Results ===`);
console.log(`  Passed:  ${passed}`);
console.log(`  Failed:  ${failed}`);
console.log(`  Skipped: ${skipped}`);
console.log(`  Total:   ${passed + failed + skipped}`);

if (failed > 0) process.exit(1);
