import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { loadOracleCorpus } from './oracle_corpus.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const packageDir = join(here, '..', 'pkg', 'core');
const wasm = readFileSync(join(packageDir, 'pillow_rs_js_bg.wasm'));
const bindings = await import(pathToFileURL(join(packageDir, 'pillow_rs_js.js')));
bindings.initSync({ module: new WebAssembly.Module(wasm) });

function referencePath(testCase) {
    const reference = testCase.input.input.reference;
    if (reference) {
        return join(
            repoRoot,
            'tests',
            'fixtures_2',
            'input',
            'images',
            `${reference}.png`,
        );
    }
    return join(repoRoot, 'tests', 'test_reference.png');
}

function createInput(testCase) {
    const spec = testCase.input.input;
    if (spec.source === 'constant') {
        if (testCase.input.mode === 'P' && !Array.isArray(spec.color)) {
            return bindings.imageNewPaletteIndex(
                spec.size[0],
                spec.size[1],
                spec.color,
            );
        }
        const values = Array.isArray(spec.color)
            ? [...spec.color]
            : [spec.color, 0, 0, 0];
        while (values.length < 4) values.push(values.length === 3 ? 255 : 0);
        return new bindings.Image(
            testCase.input.mode,
            spec.size[0],
            spec.size[1],
            values[0],
            values[1],
            values[2],
            values[3],
        );
    }
    if (spec.source !== 'reference_rgb') {
        throw new Error(`${testCase.id}: unsupported input source ${spec.source}`);
    }
    const source = bindings.Image.open(readFileSync(referencePath(testCase)));
    const resized = source.width === spec.size[0] && source.height === spec.size[1]
        ? source
        : source.resize(spec.size[0], spec.size[1], 'LANCZOS');
    return resized.convert(testCase.input.mode);
}

function expectedImage(testCase) {
    const assertion = testCase.assertion;
    const reference = join(testCase.outputRoot, assertion.reference);
    if (reference.endsWith('.bin')) {
        return {
            mode: assertion.mode,
            size: assertion.size,
            bytes: readFileSync(reference),
            raw: true,
            palette: assertion.palette,
        };
    }
    const image = bindings.Image.open(readFileSync(reference));
    return {
        mode: image.mode,
        size: [image.width, image.height],
        bytes: Buffer.from(image.getdata()),
        raw: false,
        palette: undefined,
    };
}

function assertImage(testCase, actual) {
    const expected = expectedImage(testCase);
    if (actual.mode !== expected.mode) {
        throw new Error(`${testCase.id}: mode ${actual.mode} != ${expected.mode}`);
    }
    if (actual.width !== expected.size[0] || actual.height !== expected.size[1]) {
        throw new Error(`${testCase.id}: dimensions differ`);
    }
    const actualBytes = Buffer.from(expected.raw ? actual.toBytes() : actual.getdata());
    if (!actualBytes.equals(expected.bytes)) {
        const first = actualBytes.findIndex((value, index) => value !== expected.bytes[index]);
        throw new Error(
            `${testCase.id}: exact pixel bytes differ`
            + ` (${actualBytes.length} != ${expected.bytes.length}) at ${first}:`
            + ` ${actualBytes[first]} != ${expected.bytes[first]}`,
        );
    }
    if (expected.palette !== undefined) {
        const actualPalette = Array.from(actual.getpalette());
        if (JSON.stringify(actualPalette) !== JSON.stringify(expected.palette)) {
            throw new Error(
                `${testCase.id}: exact palette differs`
                + ` (${actualPalette.length} != ${expected.palette.length})`,
            );
        }
    }
}

const cases = loadOracleCorpus().cases.filter(
    (item) => item.operation.module === 'ImageModule'
        && item.operation.target === 'eval',
);

for (const testCase of cases) {
    if (testCase.input.params.function !== 'add_10') {
        throw new Error(`${testCase.id}: unsupported callable input`);
    }
    const lut = Uint8Array.from(
        { length: 256 },
        (_, value) => Math.min(255, value + 10),
    );
    const actual = createInput(testCase).eval(lut);
    assertImage(testCase, actual);
}

process.stdout.write(`core: ${cases.length} exact Image.eval Pillow oracle cases passed\n`);
