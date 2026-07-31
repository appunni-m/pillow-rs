import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const fixtureRoot = join(
    repoRoot,
    'pillow-rs',
    'tests',
    'fixtures',
    'image_backend',
);
const manifest = JSON.parse(
    readFileSync(join(fixtureRoot, 'backend_parity.json'), 'utf8'),
);
if (
    manifest.oracle?.implementation !== 'Pillow'
    || manifest.oracle?.version !== '12.2.0'
) {
    throw new Error('Image.apply_transparency requires the Pillow 12.2.0 oracle');
}

const packageDir = join(here, '..', 'pkg', 'extra');
const wasm = readFileSync(join(packageDir, 'pillow_rs_js_bg.wasm'));
const bindings = await import(pathToFileURL(join(packageDir, 'pillow_rs_js.js')));
bindings.initSync({ module: new WebAssembly.Module(wasm) });

function transparencyInfo(image) {
    const index = image.pendingTransparencyIndex();
    if (index !== undefined) {
        return { transparency: { kind: 'index', value: index } };
    }
    const table = image.pendingTransparencyTable();
    if (table !== undefined) {
        return {
            transparency: {
                kind: 'table',
                value_hex: Buffer.from(table).toString('hex'),
            },
        };
    }
    return {};
}

const cases = manifest.apply_transparency_cases;
if (cases.length !== 3) {
    throw new Error(`Image.apply_transparency selected ${cases.length} cases; expected 3`);
}

for (const testCase of cases) {
    const image = bindings.Image.open(
        readFileSync(join(fixtureRoot, testCase.input)),
    );
    if (testCase.prepare_alpha !== null) {
        image.putalpha(testCase.prepare_alpha);
    }

    const expected = testCase.expected;
    const beforeInfo = transparencyInfo(image);
    if (JSON.stringify(beforeInfo) !== JSON.stringify(expected.before_info)) {
        throw new Error(
            `${testCase.id}: exact pending transparency differs before apply`
            + ` (${JSON.stringify(beforeInfo)} != ${JSON.stringify(expected.before_info)})`,
        );
    }
    if (image.paletteMode() !== expected.before_palette_mode) {
        throw new Error(`${testCase.id}: palette mode differs before apply`);
    }
    if (image.hasTransparencyData() !== expected.before_has_transparency_data) {
        throw new Error(`${testCase.id}: transparency flag differs before apply`);
    }

    image.applyTransparency();

    if (JSON.stringify(transparencyInfo(image)) !== JSON.stringify(expected.info)) {
        throw new Error(`${testCase.id}: exact pending transparency differs after apply`);
    }
    if (image.paletteMode() !== expected.palette_mode) {
        throw new Error(`${testCase.id}: palette mode differs after apply`);
    }
    if (image.hasTransparencyData() !== expected.has_transparency_data) {
        throw new Error(`${testCase.id}: transparency flag differs after apply`);
    }
    if (image.mode !== expected.mode) {
        throw new Error(`${testCase.id}: mode ${image.mode} != ${expected.mode}`);
    }
    if (image.width !== expected.size[0] || image.height !== expected.size[1]) {
        throw new Error(`${testCase.id}: exact dimensions differ`);
    }
    if (Buffer.from(image.toBytes()).toString('hex') !== expected.pixels_hex) {
        throw new Error(`${testCase.id}: exact indexed samples differ`);
    }
    if (Buffer.from(image.paletteRgba()).toString('hex') !== expected.palette_rgba_hex) {
        throw new Error(`${testCase.id}: exact RGBA palette differs`);
    }
}

console.log(`core: ${cases.length} exact Image.apply_transparency Pillow oracle cases passed`);
