/**
 * Shared benchmark harness — used by both Node.js WASM and browser runners.
 * DRY: common timing, input loading, output validation across all JS targets.
 */
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { performance } from 'perf_hooks';
import { createHash } from 'crypto';

export const BENCH_RUNS = 30;
export const WARMUP_RUNS = 3;

// ── Path resolution ──

let _rootDir = null;
export function rootDir() {
    if (!_rootDir) _rootDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
    return _rootDir;
}

export function benchDir() {
    return join(rootDir(), 'target', 'benchmarks');
}

export function inputDir() {
    return join(rootDir(), 'scripts', 'bench_reference_images');
}

// ── Input loading (existing images only — no new images) ──

export function loadImageFile(name) {
    return new Uint8Array(readFileSync(join(inputDir(), name)));
}

/** Load the 3 reference images. Returns {ref1k, ref2k, gray, all}. */
export function loadInputSuite() {
    return {
        ref1k: loadImageFile('ref_1k.png'),
        ref2k: loadImageFile('ref_2k.jpg'),
        gray:  loadImageFile('ref_grayscale.png'),
        all: [
            { name: 'ref_1k',   data: loadImageFile('ref_1k.png'),        w: 1024, h: 1024, mode: 'RGBA' },
            { name: 'ref_2k',   data: loadImageFile('ref_2k.jpg'),        w: 2048, h: 1536, mode: 'RGB' },
            { name: 'ref_gray', data: loadImageFile('ref_grayscale.png'), w: 1024, h: 1024, mode: 'L' },
        ],
    };
}

// ── Timing ──

export function meanStd(times) {
    const n = times.length;
    if (n === 0) return { mean: 0, std: 0 };
    const mean = times.reduce((a, b) => a + b, 0) / n;
    const variance = times.reduce((s, t) => s + (t - mean) ** 2, 0) / n;
    return { mean, std: Math.sqrt(variance) };
}

/**
 * Run a benchmark: warmup → measured runs → statistics.
 * @param {string} label - human-readable name
 * @param {() => void} fn - benchmark function (no args, no return needed)
 * @returns {{mean_ms: number, std_ms: number, runs: number, output_hash?: string}}
 */
export function runBench(label, fn, { runs = BENCH_RUNS, warmup = WARMUP_RUNS, hashFn = null } = {}) {
    // Warmup
    for (let i = 0; i < warmup; i++) {
        try { fn(); } catch (e) { /* ignore warmup errors */ }
    }
    // Measured runs
    const times = [];
    let lastResult = null;
    for (let i = 0; i < runs; i++) {
        try {
            const t0 = performance.now();
            lastResult = fn();
            const t1 = performance.now();
            times.push(t1 - t0);
        } catch (e) {
            // skip failed runs
        }
    }
    if (times.length === 0) {
        console.warn(`  [SKIP] ${label} — all runs failed`);
        return { mean_ms: -1, std_ms: -1, runs: 0, error: 'all runs failed' };
    }
    const { mean, std } = meanStd(times);
    const result = {
        mean_ms: parseFloat(mean.toFixed(4)),
        std_ms: parseFloat(std.toFixed(4)),
        runs: times.length,
    };
    // Output hash for cross-target validation
    if (hashFn && typeof hashFn === 'function') {
        try {
            result.output_hash = hashFn();
        } catch (e) {
            result.output_hash = null;
        }
    }
    console.log(`  ${label}: ${result.mean_ms}ms ±${result.std_ms}ms (${result.runs} runs)`);
    return result;
}

// ── Output hashing for validation ──

/**
 * Compute SHA-256 hash of an Image buffer (raw RGBA bytes).
 * Call after the benchmarked operation to get output_hash.
 * @param {object} wasmModule - the WASM module containing Image
 * @param {Uint8Array} imageData - raw image data from Image.toBytes()
 * @returns {string} hex digest
 */
export function hashOutput(imageData) {
    if (!imageData) return null;
    return createHash('sha256').update(imageData).digest('hex');
}

// ── Result persistence ──

export function saveResults(targetName, results) {
    const dir = benchDir();
    mkdirSync(dir, { recursive: true });
    const path = join(dir, `${targetName}.json`);
    writeFileSync(path, JSON.stringify(results, null, 2));
    console.log(`\nSaved ${Object.keys(results).length} results → ${path}`);
    return path;
}
