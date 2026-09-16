import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const manifest = JSON.parse(readFileSync('package.json', 'utf8'));
if (manifest.license !== 'MIT-CMU') {
    throw new Error(`package license must be MIT-CMU, got ${manifest.license}`);
}
if (manifest.name !== 'pillow-rs') {
    throw new Error(`package name must be pillow-rs, got ${manifest.name}`);
}
if (manifest.exports?.['.']?.node !== './node.js') {
    throw new Error('package root must select node.js under the node condition');
}
if (manifest.exports?.['.']?.browser !== './pkg/core/pillow_rs_js.js') {
    throw new Error('package root must select the generated web module in browsers');
}
if (manifest.browser !== './pkg/core/pillow_rs_js.js') {
    throw new Error('legacy browser package resolution must select the generated web module');
}

const report = JSON.parse(
    execFileSync('npm', ['pack', '--dry-run', '--json'], { encoding: 'utf8' }),
)[0];
const paths = new Set(report.files.map(file => file.path));
const required = [
    'README.md',
    'LICENSE',
    'node.js',
    'index.d.ts',
    'pkg/core/pillow_rs_js.js',
    'pkg/core/pillow_rs_js.d.ts',
    'pkg/core/pillow_rs_js_bg.wasm',
];

for (const path of required) {
    if (!paths.has(path)) {
        throw new Error(`publishable package is missing ${path}`);
    }
}

const retiredFiles = [...paths].filter((path) => path.startsWith('pkg/extra/'));
if (retiredFiles.length > 0) {
    throw new Error(`retired extra WASM output is still publishable: ${retiredFiles.join(', ')}`);
}
if (paths.has('pkg/core/package.json')) {
    throw new Error('generated nested package metadata must not be publishable');
}

// Resolve the package by name from its own package root so this check exercises
// Node's conditional export rather than bypassing the published entrypoint.
const nodeEntry = await import('pillow-rs');
const nodeOutput = await nodeEntry.default();
if (!(nodeOutput.memory instanceof WebAssembly.Memory)) {
    throw new Error('Node package entry did not initialize a WebAssembly memory export');
}
if (typeof nodeEntry.Image !== 'function' || typeof nodeEntry.initSync !== 'function') {
    throw new Error('Node package entry is missing the generated API or initSync wrapper');
}

const examples = [...readFileSync('README.md', 'utf8').matchAll(/^```javascript\s*\n([\s\S]*?)^```\s*$/gm)];
if (examples.length === 0) throw new Error('README has no executable JavaScript example');
for (const [, example] of examples) {
    execFileSync(process.execPath, ['--input-type=module', '-e', example], { stdio: 'inherit' });
}

process.stdout.write(
    `npm package: ${report.size} compressed bytes, ${report.unpackedSize} unpacked bytes, ` +
    `${report.entryCount} entries\n`,
);
