#!/usr/bin/env node
/**
 * WASM CPU Benchmark Harness
 *
 * Builds WASM for Node.js, loads reference images, benchmarks image operations,
 * and outputs results to target/benchmarks/wasm_cpu.json.
 *
 * Usage: node scripts/bench_wasm_cpu.mjs
 */

import { performance } from 'perf_hooks';
import { readFileSync, existsSync, mkdirSync, writeFileSync } from 'fs';
import { execSync } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const PKG_DIR = resolve(ROOT, 'pillow-rs-js', 'pkg_node');
const REPO_DIR = resolve(ROOT, 'pillow-rs-js');
const OUTPUT_FILE = resolve(ROOT, 'target', 'benchmarks', 'wasm_cpu.json');
const REF_IMAGES_DIR = resolve(ROOT, 'scripts', 'bench_reference_images');

const WARMUP_RUNS = 3;
const BENCH_RUNS = 30;

// ── Helpers ───────────────────────────────────────────────────────

function buildWasm() {
    if (existsSync(PKG_DIR) && existsSync(resolve(PKG_DIR, 'pillow_rs_js.js'))) {
        console.log('[build] pkg_node already exists, skipping build');
        return;
    }
    console.log('[build] Building WASM for Node.js...');
    execSync('wasm-pack build --target nodejs --out-dir pkg_node', {
        cwd: REPO_DIR,
        stdio: 'inherit',
    });
}

function loadImageBytes(name) {
    const path = resolve(REF_IMAGES_DIR, name);
    return readFileSync(path);
}

function meanStd(arr) {
    const n = arr.length;
    const mean = arr.reduce((s, v) => s + v, 0) / n;
    const variance = arr.reduce((s, v) => s + (v - mean) ** 2, 0) / n;
    return { mean_ms: parseFloat(mean.toFixed(3)), std_ms: parseFloat(Math.sqrt(variance).toFixed(3)), runs: n };
}

function runBench(label, fn) {
    const timings = [];
    // warmup
    for (let i = 0; i < WARMUP_RUNS; i++) {
        try { fn(); } catch (e) { /* ignore warmup errors */ }
    }
    // measured runs
    for (let i = 0; i < BENCH_RUNS; i++) {
        try {
            const t0 = performance.now();
            fn();
            const t1 = performance.now();
            timings.push(t1 - t0);
        } catch (e) {
            // skip failed runs silently
        }
    }
    if (timings.length === 0) {
        console.warn(`  [SKIP] ${label} — all runs failed`);
        return { mean_ms: -1, std_ms: -1, runs: 0, error: 'all runs failed' };
    }
    const result = meanStd(timings);
    console.log(`  ${label}: ${result.mean_ms}ms ±${result.std_ms}ms (${result.runs} runs)`);
    return result;
}

// ── Main ──────────────────────────────────────────────────────────

async function main() {
    buildWasm();

    console.log('[load] Importing WASM module...');
    const wasmModule = await import(resolve(PKG_DIR, 'pillow_rs_js.js'));
    const { Image, ImageOps } = wasmModule;

    console.log('[load] Loading reference images...');
    const ref1kBytes = loadImageBytes('ref_1k.png');
    const ref2kBytes = loadImageBytes('ref_2k.jpg');
    const grayBytes = loadImageBytes('ref_grayscale.png');

    const results = {};

    console.log('\n[bench] Running benchmarks (30 runs, 3 warmup)...\n');

    // ── open ──
    try {
        results.open = runBench('open', () => {
            const img = Image.open(new Uint8Array(ref1kBytes));
            img.free();
        });
    } catch (e) {
        results.open = { mean_ms: -1, std_ms: -1, runs: 0, error: String(e) };
    }

    // Load a reusable image for the remaining benchmarks
    let img = Image.open(new Uint8Array(ref1kBytes));
    let img2k = Image.open(new Uint8Array(ref2kBytes));
    let grayImg = Image.open(new Uint8Array(grayBytes));

    // ── resize ──
    try {
        results.resize = runBench('resize', () => {
            const r = img.resize(800, 600);
            r.free();
        });
    } catch (e) { results.resize = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── crop ──
    try {
        results.crop = runBench('crop', () => {
            const r = img.crop(10, 10, 100, 100);
            r.free();
        });
    } catch (e) { results.crop = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── rotate ──
    try {
        results.rotate = runBench('rotate', () => {
            const r = img.rotate(45);
            r.free();
        });
    } catch (e) { results.rotate = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── transpose ──
    try {
        results.transpose = runBench('transpose', () => {
            const r = img.transpose('FLIP_LEFT_RIGHT');
            r.free();
        });
    } catch (e) { results.transpose = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── convert ──
    try {
        results.convert = runBench('convert', () => {
            const r = img.convert('L');
            r.free();
        });
    } catch (e) { results.convert = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── filter (BLUR) ──
    try {
        results.filter = runBench('filter(BLUR)', () => {
            const r = img.filter('BLUR');
            r.free();
        });
    } catch (e) { results.filter = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── new (constructor) ──
    try {
        results.new = runBench('new', () => {
            const r = new Image('RGB', 1920, 1080, 255, 0, 0, 255);
            r.free();
        });
    } catch (e) { results.new = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getpixel ──
    try {
        results.getpixel = runBench('getpixel', () => {
            img.getpixel(100, 100);
        });
    } catch (e) { results.getpixel = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── putpixel ──
    try {
        results.putpixel = runBench('putpixel', () => {
            const p = new Image('RGB', 100, 100, 0, 0, 0, 255);
            for (let y = 0; y < 100; y++) {
                for (let x = 0; x < 100; x++) {
                    p.putpixel(x, y, 128, 128, 128, 255);
                }
            }
            p.free();
        });
    } catch (e) { results.putpixel = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── split ──
    try {
        results.split = runBench('split', () => {
            const bands = img.split();
            bands.forEach(b => b.free());
        });
    } catch (e) { results.split = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getbands ──
    try {
        results.getbands = runBench('getbands', () => {
            img.getbands();
        });
    } catch (e) { results.getbands = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getbbox ──
    try {
        results.getbbox = runBench('getbbox', () => {
            img.getbbox();
        });
    } catch (e) { results.getbbox = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getextrema ──
    try {
        results.getextrema = runBench('getextrema', () => {
            img.getextrema();
        });
    } catch (e) { results.getextrema = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── histogram ──
    try {
        results.histogram = runBench('histogram', () => {
            img.histogram();
        });
    } catch (e) { results.histogram = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── thumbnail ──
    try {
        results.thumbnail = runBench('thumbnail', () => {
            const t = Image.open(new Uint8Array(ref1kBytes));
            t.thumbnail(200, 200);
            t.free();
        });
    } catch (e) { results.thumbnail = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── paste (image paste) ──
    try {
        results.paste = runBench('paste(image)', () => {
            const dst = new Image('RGBA', 500, 500, 0, 0, 0, 0);
            const src = new Image('RGBA', 100, 100, 255, 0, 0, 255);
            dst.pasteImage(src, 50, 50);
            dst.free();
            src.free();
        });
    } catch (e) { results.paste = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── pasteColor ──
    try {
        results.pasteColor = runBench('pasteColor', () => {
            const dst = new Image('RGBA', 500, 500, 0, 0, 0, 0);
            dst.pasteColor(255, 0, 0, 255, 100, 100, 200, 200);
            dst.free();
        });
    } catch (e) { results.pasteColor = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── gaussianBlur ──
    try {
        results.gaussianBlur = runBench('gaussianBlur', () => {
            const r = img.gaussianBlur(3.0);
            r.free();
        });
    } catch (e) { results.gaussianBlur = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── save (toBytes PNG) ──
    try {
        results.save = runBench('save', () => {
            const s = img.save();
            // s is a Uint8Array, avoid dead code elimination
            if (s.length === 0) {}
        });
    } catch (e) { results.save = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── enhanceBrightness ──
    try {
        results.enhanceBrightness = runBench('enhanceBrightness', () => {
            const r = img.enhanceBrightness(1.5);
            r.free();
        });
    } catch (e) { results.enhanceBrightness = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── reduce ──
    try {
        results.reduce = runBench('reduce', () => {
            const r = img.reduce(2);
            r.free();
        });
    } catch (e) { results.reduce = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── ImageOps.invert ──
    try {
        results.imageops_invert = runBench('ImageOps.invert', () => {
            const r = ImageOps.invert(img);
            r.free();
        });
    } catch (e) { results.imageops_invert = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── ImageOps.grayscale ──
    try {
        results.imageops_grayscale = runBench('ImageOps.grayscale', () => {
            const r = ImageOps.grayscale(img);
            r.free();
        });
    } catch (e) { results.imageops_grayscale = { mean_ms: -1, runs: 0, error: String(e) }; }

    // Cleanup main images
    img.free();
    img2k.free();
    grayImg.free();

    // ── Output ──────────────────────────────────────────────
    const outputDir = dirname(OUTPUT_FILE);
    if (!existsSync(outputDir)) {
        mkdirSync(outputDir, { recursive: true });
    }
    writeFileSync(OUTPUT_FILE, JSON.stringify(results, null, 2));
    console.log(`\n[output] Results written to ${OUTPUT_FILE}`);
}

main().catch(err => {
    console.error('[fatal]', err);
    process.exit(1);
});
