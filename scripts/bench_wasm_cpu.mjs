#!/usr/bin/env node
/**
 * WASM CPU Benchmark Harness — Node.js
 *
 * Uses shared bench_harness.mjs for timing, input loading, output hashing.
 * Auto-builds WASM for Node.js if needed.
 * Output: target/benchmarks/wasm_cpu.json
 */
import { readFileSync, existsSync } from 'fs';
import { execSync } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const PKG_DIR = resolve(ROOT, 'pillow-rs-js', 'pkg_node');
const REPO_DIR = resolve(ROOT, 'pillow-rs-js');

// ── Shared harness imports ──
import {
    runBench, saveResults, hashOutput,
    loadInputSuite, inputDir, benchDir,
} from '../pillow-rs-js/bench_page/bench_harness.mjs';

// ── Build WASM if needed ──

function buildWasm() {
    if (existsSync(PKG_DIR) && existsSync(resolve(PKG_DIR, 'pillow_rs_js.js'))) {
        console.log('[build] pkg_node exists, skipping');
        return;
    }
    console.log('[build] Building WASM for Node.js...');
    execSync('wasm-pack build --target nodejs --out-dir pkg_node', {
        cwd: REPO_DIR, stdio: 'inherit',
    });
}

// ── Main ──

async function main() {
    buildWasm();

    console.log('[load] Importing WASM...');
    const wasmModule = await import(resolve(PKG_DIR, 'pillow_rs_js.js'));
    const { Image, ImageOps } = wasmModule;

    console.log('[load] Loading reference images...');
    const { ref1k, ref2k, gray } = loadInputSuite();
    const mkdirSync = (await import('fs')).mkdirSync;
    mkdirSync(benchDir(), { recursive: true });

    // Reusable loaded images
    const img = Image.open(new Uint8Array(ref1k));
    const img2k = Image.open(new Uint8Array(ref2k));
    const grayImg = Image.open(new Uint8Array(gray));

    const results = {};
    const toHash = (imgObj) => { try { return hashOutput(imgObj.save?.() ?? imgObj.toBytes?.()); } catch(e) { return null; } };

    console.log('\n[bench] Running benchmarks (30 runs, 3 warmup)...\n');

    // ── open ──
    try { results.open = runBench('open', () => { const i = Image.open(new Uint8Array(ref1k)); i.free(); }); }
    catch (e) { results.open = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── resize ──
    try {
        let last;
        results.resize = runBench('resize', () => { last = img.resize(800, 600); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.resize = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── crop ──
    try {
        let last;
        results.crop = runBench('crop', () => { last = img.crop(10, 10, 100, 100); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.crop = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── rotate ──
    try {
        let last;
        results.rotate = runBench('rotate', () => { last = img.rotate(90); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.rotate = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── transpose ──
    try {
        let last;
        results.transpose = runBench('transpose', () => { last = img.transpose('FLIP_LEFT_RIGHT'); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.transpose = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── convert ──
    try {
        let last;
        results.convert = runBench('convert', () => { last = img.convert('L'); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.convert = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── filter BLUR ──
    try {
        let last;
        results.filter = runBench('filter', () => { last = img.filter('BLUR'); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.filter = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── new ──
    try {
        results.new = runBench('new', () => {
            const r = new Image('RGB', 1920, 1080, 255, 0, 0, 255); r.free();
        });
    } catch (e) { results.new = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getpixel ──
    try { results.getpixel = runBench('getpixel', () => { img.getpixel(100, 100); }); }
    catch (e) { results.getpixel = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── putpixel ──
    try {
        results.putpixel = runBench('putpixel', () => {
            const p = new Image('RGB', 100, 100, 0, 0, 0, 255);
            for (let y = 0; y < 100; y++)
                for (let x = 0; x < 100; x++)
                    p.putpixel(x, y, 128, 128, 128, 255);
            p.free();
        });
    } catch (e) { results.putpixel = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── split ──
    try { results.split = runBench('split', () => { const b = img.split(); b.forEach(x => x.free()); }); }
    catch (e) { results.split = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getbands ──
    try { results.getbands = runBench('getbands', () => { img.getbands(); }); }
    catch (e) { results.getbands = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getbbox ──
    try { results.getbbox = runBench('getbbox', () => { img.getbbox(); }); }
    catch (e) { results.getbbox = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getextrema ──
    try { results.getextrema = runBench('getextrema', () => { img.getextrema(); }); }
    catch (e) { results.getextrema = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── histogram ──
    try { results.histogram = runBench('histogram', () => { img.histogram(); }); }
    catch (e) { results.histogram = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── thumbnail ──
    try {
        let last;
        results.thumbnail = runBench('thumbnail', () => {
            const t = Image.open(new Uint8Array(ref1k)); t.thumbnail(200, 200); last = t; t.free();
        }, { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.thumbnail = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── paste (matches Pillow baseline: 800x600 onto 2048x1536) ──
    try {
        let last;
        results.paste = runBench('paste', () => {
            const dst = Image.open(new Uint8Array(ref2k));
            const src = new Image('RGB', 800, 600, 0, 255, 0, 255);
            dst.pasteImage(src, 100, 100); last = dst; dst.free(); src.free();
        }, { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.paste = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── pasteColor (matches Pillow baseline: 800x600 fill onto 2048x1536) ──
    try {
        let last;
        results.pasteColor = runBench('pasteColor', () => {
            const dst = Image.open(new Uint8Array(ref2k));
            dst.pasteColor(255, 0, 0, 255, 100, 100, 900, 700); last = dst; dst.free();
        }, { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.pasteColor = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── gaussianBlur ──
    try {
        let last;
        results.gaussianBlur = runBench('gaussianBlur', () => { last = img.gaussianBlur(3.0); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.gaussianBlur = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── save ──
    try { results.save = runBench('save', () => { const s = img.save(); if (s.length === 0) {} }); }
    catch (e) { results.save = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── enhanceBrightness ──
    try {
        let last;
        results.enhanceBrightness = runBench('enhanceBrightness', () => { last = img.enhanceBrightness(1.5); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.enhanceBrightness = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── reduce ──
    try {
        let last;
        results.reduce = runBench('reduce', () => { last = img.reduce(2); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.reduce = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── ImageOps.invert ──
    try {
        let last;
        results.imageops_invert = runBench('ImageOps.invert', () => { last = ImageOps.invert(img); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.imageops_invert = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── ImageOps.grayscale ──
    try {
        let last;
        results.imageops_grayscale = runBench('ImageOps.grayscale', () => { last = ImageOps.grayscale(img); last.free(); },
            { hashFn: () => hashOutput(last?.save?.()) });
    } catch (e) { results.imageops_grayscale = { mean_ms: -1, runs: 0, error: String(e) }; }

    img.free(); img2k.free(); grayImg.free();

    saveResults('wasm_cpu', results);
}

main().catch(err => { console.error('[fatal]', err); process.exit(1); });
