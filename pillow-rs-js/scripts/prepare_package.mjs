import { rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const variant = process.argv[2];
if (!['core', 'extra'].includes(variant)) {
    throw new Error('usage: node scripts/prepare_package.mjs <core|extra>');
}

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

// wasm-pack writes this file to keep generated bindings out of source control.
// The repository already ignores pkg/, while leaving the nested file in place
// makes npm exclude the actual bindings even though package.json whitelists pkg/.
rmSync(join(root, 'pkg', variant, '.gitignore'), { force: true });
