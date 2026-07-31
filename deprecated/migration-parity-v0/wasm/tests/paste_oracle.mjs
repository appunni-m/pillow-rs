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
    'image_backend',
    'backend_parity.json',
), 'utf8'));
if (
    manifest.oracle?.implementation !== 'Pillow'
    || manifest.oracle?.version !== '12.2.0'
) {
    throw new Error('Image.paste requires the Pillow 12.2.0 oracle');
}

const packageDir = join(here, '..', 'pkg', 'extra');
const wasm = readFileSync(join(packageDir, 'pillow_rs_js_bg.wasm'));
const bindings = await import(pathToFileURL(join(packageDir, 'pillow_rs_js.js')));
bindings.initSync({ module: new WebAssembly.Module(wasm) });

function imageFromSpec(spec) {
    const seed = new bindings.Image('L', 1, 1, 0, 0, 0, 255);
    const image = seed.fromBytes(
        spec.mode === 'PA' ? 'LA' : spec.mode,
        spec.size[0],
        spec.size[1],
        Buffer.from(spec.pixels_hex, 'hex'),
    );
    if (spec.palette_hex !== null) {
        image.putpalette(Buffer.from(spec.palette_hex, 'hex'), 'RGB');
    }
    return image;
}

function runPaste(testCase) {
    const destination = imageFromSpec(testCase.destination);
    const source = testCase.source.kind === 'image'
        ? imageFromSpec(testCase.source.image)
        : testCase.source.value;
    const mask = testCase.mask === null ? null : imageFromSpec(testCase.mask);
    const box = testCase.box;

    if (testCase.source.kind === 'image') {
        if (box.length === 2 && mask === null) {
            destination.pasteImage(source, box[0], box[1]);
        } else if (box.length === 2) {
            destination.pasteImageMasked(source, box[0], box[1], mask);
        } else if (mask === null) {
            destination.pasteImageRegion(source, ...box);
        } else {
            destination.pasteImageRegionMasked(source, ...box, mask);
        }
    } else if (testCase.source.kind === 'scalar') {
        destination.pasteScalarRegion(source, ...box);
    } else if (source.length === 4) {
        destination.pasteColor(...source, ...box);
    } else if (source.length === 3 && box.length === 2 && mask !== null) {
        destination.pasteRgbAt(...source, ...box, mask);
    } else if (source.length === 2) {
        destination.pasteLumaAlphaRegion(...source, ...box);
    } else {
        throw new Error(`${testCase.id}: unsupported typed paste call`);
    }
    return destination;
}

function assertImage(testCase, actual) {
    const expected = testCase.expected;
    if (actual.mode !== expected.mode) {
        throw new Error(`${testCase.id}: mode ${actual.mode} != ${expected.mode}`);
    }
    if (actual.width !== expected.size[0] || actual.height !== expected.size[1]) {
        throw new Error(`${testCase.id}: exact dimensions differ`);
    }
    if (Buffer.from(actual.toBytes()).toString('hex') !== expected.pixels_hex) {
        throw new Error(`${testCase.id}: exact pixels differ`);
    }
    if (
        expected.palette_hex !== null
        && Buffer.from(actual.getpalette()).toString('hex') !== expected.palette_hex
    ) {
        throw new Error(`${testCase.id}: exact palette differs`);
    }
}

for (const testCase of manifest.paste_cases) {
    assertImage(testCase, runPaste(testCase));
}

const unsupportedErrors = new Set(['rgb_two_element_color', 'l_three_element_color']);
let errorCount = 0;
for (const testCase of manifest.paste_error_cases) {
    if (unsupportedErrors.has(testCase.id)) {
        continue;
    }
    const destination = imageFromSpec(testCase.destination);
    const source = testCase.source.kind === 'image'
        ? imageFromSpec(testCase.source.image)
        : testCase.source.value;
    const mask = testCase.mask === null ? null : imageFromSpec(testCase.mask);
    try {
        if (testCase.id === 'solid_without_sized_region') {
            destination.pasteScalarAt(source, 0, 0);
        } else if (testCase.box.length === 4) {
            destination.pasteImageRegion(source, ...testCase.box);
        } else {
            destination.pasteImageMasked(source, ...testCase.box, mask);
        }
        throw new Error(`${testCase.id}: expected paste failure`);
    } catch (error) {
        if (error.name !== testCase.expected_error.type) {
            throw new Error(
                `${testCase.id}: error type ${error.name} != ${testCase.expected_error.type}`,
            );
        }
        if (error.message !== testCase.expected_error.message) {
            throw new Error(`${testCase.id}: exact error message differs`);
        }
        errorCount += 1;
    }
}

console.log(
    `core: ${manifest.paste_cases.length} exact Image.paste Pillow oracle cases passed; `
    + `${errorCount} exact errors passed; ${unsupportedErrors.size} invalid tuple shapes `
    + 'are unrepresentable by the typed WASM ABI',
);
