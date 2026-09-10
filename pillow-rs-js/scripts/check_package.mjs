import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const manifest = JSON.parse(readFileSync('package.json', 'utf8'));
if (manifest.license !== 'MIT-CMU') {
    throw new Error(`package license must be MIT-CMU, got ${manifest.license}`);
}

const report = JSON.parse(
    execFileSync('npm', ['pack', '--dry-run', '--json'], { encoding: 'utf8' }),
)[0];
const paths = new Set(report.files.map(file => file.path));
const required = [
    'README.md',
    'LICENSE',
    'pkg/core/pillow_rs_js.js',
    'pkg/core/pillow_rs_js.d.ts',
    'pkg/core/pillow_rs_js_bg.wasm',
    'pkg/extra/pillow_rs_js.js',
    'pkg/extra/pillow_rs_js.d.ts',
    'pkg/extra/pillow_rs_js_bg.wasm',
];

for (const path of required) {
    if (!paths.has(path)) {
        throw new Error(`publishable package is missing ${path}`);
    }
}

process.stdout.write(
    `npm package: ${report.size} compressed bytes, ${report.unpackedSize} unpacked bytes, ` +
    `${report.entryCount} entries\n`,
);
