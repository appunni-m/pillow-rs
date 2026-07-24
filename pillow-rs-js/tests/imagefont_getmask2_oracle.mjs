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
    'getmask2.json',
), 'utf8'));

if (
    manifest.oracle?.implementation !== 'Pillow'
    || manifest.oracle?.version !== '12.2.0'
    || manifest.oracle?.freetype_version !== '2.14.3'
) {
    throw new Error('ImageFont.getmask2 requires the pinned Pillow/FreeType oracle');
}

const packageDir = join(here, '..', 'pkg', 'extra');
const wasm = readFileSync(join(packageDir, 'pillow_rs_js_bg.wasm'));
const bindings = await import(pathToFileURL(join(packageDir, 'pillow_rs_js.js')));
bindings.initSync({ module: new WebAssembly.Module(wasm) });

const font = bindings.ImageFont.loadDefault();
for (const testCase of manifest.cases) {
    const start = testCase.start ?? [undefined, undefined];
    const actual = font.getmask2(testCase.text, start[0], start[1]);
    const expected = testCase.expected;
    if (actual.width !== expected.size[0] || actual.height !== expected.size[1]) {
        throw new Error(`${testCase.id}: exact dimensions differ`);
    }
    if (actual.offsetX !== expected.offset[0] || actual.offsetY !== expected.offset[1]) {
        throw new Error(`${testCase.id}: exact offset differs`);
    }
    if (Buffer.from(actual.pixels).toString('hex') !== expected.pixels_hex) {
        throw new Error(`${testCase.id}: exact mask bytes differ`);
    }
}

console.log(
    `core: ${manifest.cases.length} exact ImageFont.getmask2 Pillow oracle cases passed`,
);
