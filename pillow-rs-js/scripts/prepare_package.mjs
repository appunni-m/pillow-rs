import { rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const variant = process.argv[2];
if (variant !== 'core') {
    throw new Error('usage: node scripts/prepare_package.mjs core');
}

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

// wasm-pack writes this metadata for a package published directly from its
// output directory. The repository publishes the package root instead, so a
// nested package manifest, README, or license would advertise a second npm
// package and duplicate the root metadata. Remove those generated markers
// while retaining only the bindings and adjacent Wasm asset.
for (const name of ['.gitignore', 'LICENSE', 'README.md', 'package.json']) {
    rmSync(join(root, 'pkg', variant, name), { force: true });
}
rmSync(join(root, 'pkg', 'extra'), { force: true, recursive: true });
