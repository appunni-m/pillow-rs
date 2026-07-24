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

function color3dTable(size, channels, pattern) {
    if (pattern !== 'identity') {
        throw new Error(`unknown Color3DLUT table pattern: ${pattern}`);
    }
    const table = [];
    for (let z = 0; z < size[2]; z += 1) {
        for (let y = 0; y < size[1]; y += 1) {
            for (let x = 0; x < size[0]; x += 1) {
                const values = [
                    x / (size[0] - 1),
                    y / (size[1] - 1),
                    z / (size[2] - 1),
                    (x + 2 * y + 3 * z) / (6 * (size[0] - 1)),
                ];
                table.push(...values.slice(0, channels));
            }
        }
    }
    return table;
}

function createInput(spec, mode) {
    if (spec.source !== 'reference_rgb') {
        throw new Error(`unsupported oracle input source: ${spec.source}`);
    }
    const source = bindings.Image.open(readFileSync(join(repoRoot, 'tests', 'test_reference.png')));
    const resized = source.resize(spec.size[0], spec.size[1], 'LANCZOS');
    return resized.convert(mode);
}

function expectedImage(testCase) {
    const assertion = testCase.assertion;
    const reference = join(testCase.outputRoot, assertion.reference);
    if (reference.endsWith('.bin')) {
        return {
            mode: assertion.mode,
            size: assertion.size,
            bytes: readFileSync(reference),
        };
    }
    const image = bindings.Image.open(readFileSync(reference));
    return {
        mode: image.mode,
        size: [image.width, image.height],
        bytes: Buffer.from(image.toBytes()),
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
    if (!Buffer.from(actual.toBytes()).equals(expected.bytes)) {
        throw new Error(`${testCase.id}: exact pixel bytes differ`);
    }
}

const cases = loadOracleCorpus().cases.filter(
    (item) => item.operation.module === 'ImageFilter'
        && item.operation.target === 'Color3DLUT',
);

for (const testCase of cases) {
    const params = testCase.input.params;
    const size = Number.isInteger(params.size)
        ? [params.size, params.size, params.size]
        : params.size;
    const image = createInput(testCase.input.input, testCase.input.mode);
    const table = color3dTable(size, params.channels, params._table_pattern);
    try {
        const actual = image.color3DLUT(
            size[0],
            size[1],
            size[2],
            table,
            params.channels,
            params.target_mode,
        );
        if (testCase.assertion.method === 'error') {
            throw new Error(`${testCase.id}: expected ${testCase.assertion.exception}`);
        }
        assertImage(testCase, actual);
    } catch (caught) {
        if (testCase.assertion.method !== 'error') throw caught;
        if (caught.name !== testCase.assertion.exception) {
            throw new Error(
                `${testCase.id}: exception ${caught.name} != ${testCase.assertion.exception}`,
            );
        }
        if (caught.message !== testCase.assertion.message) {
            throw new Error(
                `${testCase.id}: message ${JSON.stringify(caught.message)}`
                + ` != ${JSON.stringify(testCase.assertion.message)}`,
            );
        }
    }
}

process.stdout.write(`core: ${cases.length} exact Color3DLUT Pillow oracle cases passed\n`);
