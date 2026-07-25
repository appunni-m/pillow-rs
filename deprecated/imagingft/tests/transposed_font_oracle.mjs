import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const manifest = JSON.parse(readFileSync(join(
    repoRoot,
    'pillow-rs',
    'tests',
    'fixtures',
    'imagefont',
    'transposed_font.json',
), 'utf8'));

const packageDir = join(here, '..', 'pkg', 'extra');
const wasm = readFileSync(join(packageDir, 'pillow_rs_js_bg.wasm'));
const bindings = await import(pathToFileURL(join(packageDir, 'pillow_rs_js.js')));
bindings.initSync({ module: new WebAssembly.Module(wasm) });

const font = bindings.ImageFont.loadDefault();
for (const testCase of manifest.cases) {
    const orientation = testCase.orientation ?? undefined;
    const bbox = Array.from(font.getTransposedBbox(testCase.text, orientation));
    if (JSON.stringify(bbox) !== JSON.stringify(testCase.expected.bbox)) {
        throw new Error(`${testCase.id}: exact bbox differs`);
    }

    if (testCase.expected.length_error) {
        try {
            font.getTransposedLength(testCase.text, orientation);
            throw new Error(`${testCase.id}: expected rotated-length error`);
        } catch (error) {
            if (
                error.name !== testCase.expected.length_error.type
                || error.message !== testCase.expected.length_error.message
            ) {
                throw new Error(`${testCase.id}: exact length error differs`);
            }
        }
    } else if (
        font.getTransposedLength(testCase.text, orientation)
        !== testCase.expected.length
    ) {
        throw new Error(`${testCase.id}: exact length differs`);
    }

    const actual = font.getTransposedMask(testCase.text, orientation);
    const expected = testCase.expected.mask;
    if (actual.width !== expected.size[0] || actual.height !== expected.size[1]) {
        throw new Error(`${testCase.id}: exact dimensions differ`);
    }
    if (Buffer.from(actual.pixels).toString('hex') !== expected.pixels_hex) {
        throw new Error(`${testCase.id}: exact mask bytes differ`);
    }
}

console.log(
    `core: ${manifest.cases.length} independent TransposedFont mask paths passed`,
);
