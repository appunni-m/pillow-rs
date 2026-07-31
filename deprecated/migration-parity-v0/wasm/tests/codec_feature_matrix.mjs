import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const variant = process.argv[2];
if (!['core', 'extra'].includes(variant)) {
    throw new Error('usage: node tests/codec_feature_matrix.mjs <core|extra>');
}

const here = dirname(fileURLToPath(import.meta.url));
const packageDir = join(here, '..', 'pkg', variant);
const fixtureDir = join(here, '..', '..', 'pillow-rs', 'tests', 'fixtures', 'image_backend');
const manifest = JSON.parse(readFileSync(join(fixtureDir, 'manifest.json'), 'utf8'));
const wasm = readFileSync(join(packageDir, 'pillow_rs_js_bg.wasm'));
const bindings = await import(pathToFileURL(join(packageDir, 'pillow_rs_js.js')));
bindings.initSync({ module: new WebAssembly.Module(wasm) });

const enabled = new Set(variant === 'core'
    ? ['image-png']
    : ['image-png', 'image-jpeg', 'image-gif', 'image-bmp', 'image-tiff', 'image-webp', 'image-ico']);

let checked = 0;
for (const row of manifest.decode) {
    const input = readFileSync(join(fixtureDir, row.input));
    if (!enabled.has(row.feature)) {
        let error;
        try {
            bindings.Image.open(input);
        } catch (caught) {
            error = String(caught);
        }
        if (!error || !error.toLowerCase().includes('feature')) {
            throw new Error(`${variant}:${row.id} expected a structured feature-disabled error, got ${error}`);
        }
        checked += 1;
        continue;
    }

    const image = bindings.Image.open(input);
    const expected = readFileSync(join(fixtureDir, row.pixels));
    const actual = Buffer.from(image.toBytes());
    if (image.mode !== row.mode || image.width !== row.width || image.height !== row.height) {
        throw new Error(`${variant}:${row.id} metadata mismatch`);
    }
    if (!actual.equals(expected)) {
        throw new Error(`${variant}:${row.id} pixel bytes mismatch`);
    }
    checked += 1;
}

process.stdout.write(`${variant}: ${checked} manifest codec rows passed\n`);
