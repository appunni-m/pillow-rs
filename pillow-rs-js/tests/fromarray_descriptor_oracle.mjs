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
    'fromarray',
    'descriptor.json',
), 'utf8'));

const packageDir = join(here, '..', 'pkg', 'core');
const wasm = readFileSync(join(packageDir, 'pillow_rs_js_bg.wasm'));
const bindings = await import(pathToFileURL(join(packageDir, 'pillow_rs_js.js')));
bindings.initSync({ module: new WebAssembly.Module(wasm) });

for (const testCase of manifest.cases) {
    const expectedError = testCase.expected.error;
    if (expectedError) {
        try {
            bindings.resolveArrayLayout(
                testCase.shape,
                testCase.typestr,
                testCase.mode ?? undefined,
            );
            throw new Error(`${testCase.id}: expected descriptor error`);
        } catch (error) {
            if (error.name !== expectedError.type || error.message !== expectedError.message) {
                throw new Error(`${testCase.id}: exact descriptor error differs`);
            }
        }
        continue;
    }

    const layout = bindings.resolveArrayLayout(
        testCase.shape,
        testCase.typestr,
        testCase.mode ?? undefined,
    );
    if (layout.mode !== testCase.expected.mode) {
        throw new Error(`${testCase.id}: exact mode differs`);
    }
    if (
        layout.width !== testCase.expected.size[0]
        || layout.height !== testCase.expected.size[1]
    ) {
        throw new Error(`${testCase.id}: exact size differs`);
    }
}

console.log(`core: ${manifest.cases.length} exact fromarray descriptors passed`);
