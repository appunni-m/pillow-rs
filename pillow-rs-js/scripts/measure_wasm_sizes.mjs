import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { brotliCompressSync, constants, gzipSync } from 'node:zlib';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const rows = ['core', 'extra'].map((variant) => {
    const packageDir = join(root, 'pkg', variant);
    const path = join(packageDir, 'pillow_rs_js_bg.wasm');
    const bytes = readFileSync(path);
    const packageBytes = readdirSync(packageDir)
        .map((name) => statSync(join(packageDir, name)).size)
        .reduce((total, size) => total + size, 0);
    return {
        variant,
        features: ['image-codecs-all'],
        wasm_bytes: bytes.length,
        js_bytes: statSync(join(packageDir, 'pillow_rs_js.js')).size,
        types_bytes:
            statSync(join(packageDir, 'pillow_rs_js.d.ts')).size
            + statSync(join(packageDir, 'pillow_rs_js_bg.wasm.d.ts')).size,
        package_bytes: packageBytes,
        gzip_bytes: gzipSync(bytes, { level: 9 }).length,
        brotli_bytes: brotliCompressSync(bytes, {
            params: {
                [constants.BROTLI_PARAM_QUALITY]: 11,
                [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_GENERIC,
            },
        }).length,
    };
});

const report = {
    schema: 1,
    build: 'wasm-pack --target web --release',
    debug_hooks: false,
    variants: rows,
};
const output = `${JSON.stringify(report, null, 2)}\n`;
writeFileSync(join(root, 'pkg', 'sizes.json'), output);
process.stdout.write(output);
