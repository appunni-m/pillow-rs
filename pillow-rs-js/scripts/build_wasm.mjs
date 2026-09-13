import { spawnSync } from 'node:child_process';
import { rmSync } from 'node:fs';
import { join, resolve } from 'node:path';

const [variant, profile = 'dev'] = process.argv.slice(2);
if (variant !== 'core' || !['dev', 'release'].includes(profile)) {
    throw new Error('usage: build_wasm.mjs core <dev|release>');
}

const root = resolve(new URL('..', import.meta.url).pathname);

// The package has one WASM payload. Remove output from the retired split
// package before every build so a stale checkout can never publish a second
// variant through the broad `pkg/` files list.
rmSync(join(root, 'pkg', 'extra'), { force: true, recursive: true });

const args = [
    'build',
    '--target',
    'web',
    ...(profile === 'release' ? ['--release'] : []),
    '--out-dir',
    `pkg/${variant}`,
    '--no-default-features',
    '--features',
    'wasm-all',
];

// wasm-opt is a packaging optimization, not a correctness check. The
// managed all-backend lane sets this opt-out only when the platform cannot
// provide Binaryen; normal builds retain wasm-pack's optimized default.
if (process.env.MIGRATION_WASM_NO_OPT === '1') {
    args.splice(1, 0, '--no-opt');
}

const build = spawnSync('wasm-pack', args, {
    cwd: root,
    stdio: 'inherit',
});
if (build.error) {
    throw build.error;
}
if (build.status !== 0) {
    process.exit(build.status ?? 1);
}

const package_step = spawnSync('node', ['scripts/prepare_package.mjs', variant], {
    cwd: root,
    stdio: 'inherit',
});
if (package_step.error) {
    throw package_step.error;
}
process.exit(package_step.status ?? 1);
