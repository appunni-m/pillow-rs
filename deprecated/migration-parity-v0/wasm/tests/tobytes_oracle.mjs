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

function createInput(testCase) {
    const spec = testCase.input.input;
    if (spec.source === 'constant') {
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
    const source = bindings.Image.open(
        readFileSync(join(repoRoot, 'tests', 'test_reference.png')),
    );
    return source
        .resize(spec.size[0], spec.size[1], 'LANCZOS')
        .convert(testCase.input.mode);
}

const cases = loadOracleCorpus().cases.filter(
    (item) => item.suite === 'fixtures'
        && item.operation.module === 'Image'
        && item.operation.target === 'tobytes',
);

if (cases.length !== 8) {
    throw new Error(`Image.tobytes corpus selected ${cases.length} cases; expected 8`);
}

for (const testCase of cases) {
    const assertion = testCase.assertion;
    if (assertion.method !== 'image' || assertion.raw_kind !== 'bytes') {
        throw new Error(`${testCase.id}: expected exact public byte assertion`);
    }
    const image = createInput(testCase);
    const positional = testCase.input.params._args ?? [];
    const actual = positional.length === 0
        ? image.toBytes()
        : image.toBytesEncoded(positional[0], positional.slice(1));
    const expected = readFileSync(join(testCase.outputRoot, assertion.reference));
    if (!Buffer.from(actual).equals(expected)) {
        const bytes = Buffer.from(actual);
        const first = bytes.findIndex((value, index) => value !== expected[index]);
        throw new Error(
            `${testCase.id}: exact bytes differ at ${first}`
            + ` (${bytes[first]} != ${expected[first]})`,
        );
    }
}

process.stdout.write(
    `core: ${cases.length} exact Image.tobytes Pillow oracle cases passed\n`,
);
