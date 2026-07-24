import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { loadOracleCorpus, PILLOW_VERSION } from './oracle_corpus.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const packageDir = join(here, '..', 'pkg', 'extra');
const wasm = readFileSync(join(packageDir, 'pillow_rs_js_bg.wasm'));
const bindings = await import(pathToFileURL(join(packageDir, 'pillow_rs_js.js')));
bindings.initSync({ module: new WebAssembly.Module(wasm) });

const inputManifest = JSON.parse(
    readFileSync(join(repoRoot, 'tests', 'oracles', 'image_open_inputs.json'), 'utf8'),
);
if (inputManifest.pillow_version !== PILLOW_VERSION) {
    throw new Error(
        `image-open input Pillow ${inputManifest.pillow_version} != ${PILLOW_VERSION}`,
    );
}

const fixtureModes = Object.keys(inputManifest.inputs);
const expectedCaseIds = new Set(fixtureModes.map(
    (mode) => (mode === 'RGB' ? 'rgb_10x10' : `ImageModule.open_${mode}`),
));
const cases = loadOracleCorpus().cases.filter(
    (item) => item.suite === 'fixtures'
        && item.operation.module === 'ImageModule'
        && item.operation.target === 'open'
        && expectedCaseIds.has(item.input.id),
);

if (cases.length !== expectedCaseIds.size) {
    throw new Error(
        `Image.open corpus selected ${cases.length} cases; expected ${expectedCaseIds.size}`,
    );
}

for (const testCase of cases) {
    const mode = testCase.input.mode;
    const input = inputManifest.inputs[mode];
    if (!input) throw new Error(`${testCase.id}: missing canonical encoded input`);

    const assertion = testCase.assertion;
    if (
        assertion.method !== 'image'
        || assertion.raw_kind !== 'image'
        || typeof assertion.mode !== 'string'
        || !Array.isArray(assertion.size)
    ) {
        throw new Error(`${testCase.id}: Image.open oracle must be exact raw image bytes`);
    }

    const actual = bindings.Image.open(Buffer.from(input.base64, 'base64'));
    if (actual.mode !== assertion.mode) {
        throw new Error(`${testCase.id}: mode ${actual.mode} != ${assertion.mode}`);
    }
    if (actual.width !== assertion.size[0] || actual.height !== assertion.size[1]) {
        throw new Error(`${testCase.id}: exact dimensions differ`);
    }

    const expectedBytes = readFileSync(join(testCase.outputRoot, assertion.reference));
    const actualBytes = Buffer.from(actual.toBytes());
    if (!actualBytes.equals(expectedBytes)) {
        const first = actualBytes.findIndex(
            (value, index) => value !== expectedBytes[index],
        );
        throw new Error(
            `${testCase.id}: exact public bytes differ at ${first}`
            + ` (${actualBytes[first]} != ${expectedBytes[first]})`,
        );
    }

    if (Object.hasOwn(assertion, 'palette')) {
        const actualPalette = Array.from(actual.getpalette());
        if (JSON.stringify(actualPalette) !== JSON.stringify(assertion.palette)) {
            throw new Error(`${testCase.id}: exact palette differs`);
        }
    }
}

process.stdout.write(
    `extra: ${cases.length} exact Image.open Pillow oracle cases passed\n`,
);
