#!/usr/bin/env node
/**
 * WASM CPU Benchmark Harness — Node.js
 * Pipeline architecture: ops are lazy, so we force materialization via toBytes().
 * Output: target/benchmarks/wasm_cpu.json
 */
import { existsSync } from 'fs';
import { execSync } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { runBench, saveResults, hashOutput, loadInputSuite, benchDir } from '../pillow-rs-js/bench_page/bench_harness.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const PKG_DIR = resolve(ROOT, 'pillow-rs-js', 'pkg_node');
const REPO_DIR = resolve(ROOT, 'pillow-rs-js');

function buildWasm() {
    if (existsSync(PKG_DIR) && existsSync(resolve(PKG_DIR, 'pillow_rs_js.js'))) return;
    console.log('[build] Building WASM for Node.js...');
    execSync('wasm-pack build --target nodejs --out-dir pkg_node', { cwd: REPO_DIR, stdio: 'inherit' });
}

async function main() {
    buildWasm();
    console.log('[load] Importing WASM...');
    const wasmModule = await import(resolve(PKG_DIR, 'pillow_rs_js.js'));
    const { Image, ImageOps } = wasmModule;
    const { ref1k, ref2k, gray } = loadInputSuite();

    // Force mkdir
    const { mkdirSync } = await import('fs');
    mkdirSync(benchDir(), { recursive: true });

    const img = Image.open(new Uint8Array(ref1k));
    const img2k = Image.open(new Uint8Array(ref2k));
    const grayImg = Image.open(new Uint8Array(gray));
    const results = {};

    console.log('\n[bench] Running WASM CPU benchmarks (pipeline ops force toBytes)...\n');

    // Each bench: create pipeline op + toBytes() forces materialize + free

    try { results.open = runBench('open', () => { const i = Image.open(new Uint8Array(ref1k)); i.toBytes(); i.free(); }); }
    catch (e) { results.open = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.resize = runBench('resize', () => { const r = img.resize(800, 600); r.toBytes(); r.free(); }); }
    catch (e) { results.resize = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.crop = runBench('crop', () => { const r = img.crop(10, 10, 100, 100); r.toBytes(); r.free(); }); }
    catch (e) { results.crop = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.rotate = runBench('rotate', () => { const r = img.rotate(90); r.toBytes(); r.free(); }); }
    catch (e) { results.rotate = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.transpose = runBench('transpose', () => { const r = img.transpose('FLIP_LEFT_RIGHT'); r.toBytes(); r.free(); }); }
    catch (e) { results.transpose = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.convert = runBench('convert', () => { const r = img.convert('L'); r.toBytes(); r.free(); }); }
    catch (e) { results.convert = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.filter = runBench('filter', () => { const r = img.filter('BLUR'); r.toBytes(); r.free(); }); }
    catch (e) { results.filter = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.new = runBench('new', () => { const r = new Image('RGB', 1920, 1080, 255, 0, 0, 255); r.toBytes(); r.free(); }); }
    catch (e) { results.new = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.getpixel = runBench('getpixel', () => { img.getpixel(100, 100); }); }
    catch (e) { results.getpixel = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.putpixel = runBench('putpixel', () => {
        const p = new Image('RGB', 100, 100, 0, 0, 0, 255);
        for (let y = 0; y < 100; y++) for (let x = 0; x < 100; x++) p.putpixel(x, y, 128, 128, 128, 255);
        p.free();
    }); } catch (e) { results.putpixel = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.split = runBench('split', () => { const b = img.split(); b.forEach(x => x.free()); }); }
    catch (e) { results.split = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.getbands = runBench('getbands', () => { img.getbands(); }); }
    catch (e) { results.getbands = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.getbbox = runBench('getbbox', () => { img.getbbox(); }); }
    catch (e) { results.getbbox = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.getextrema = runBench('getextrema', () => { img.getextrema(); }); }
    catch (e) { results.getextrema = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.histogram = runBench('histogram', () => { img.histogram(); }); }
    catch (e) { results.histogram = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.thumbnail = runBench('thumbnail', () => { const t = Image.open(new Uint8Array(ref1k)); t.thumbnail(200, 200); t.toBytes(); t.free(); }); }
    catch (e) { results.thumbnail = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.paste = runBench('paste', () => {
        const dst = Image.open(new Uint8Array(ref2k));
        const src = new Image('RGB', 800, 600, 0, 255, 0, 255);
        dst.pasteImage(src, 100, 100); dst.toBytes(); dst.free(); src.free();
    }); } catch (e) { results.paste = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.pasteColor = runBench('pasteColor', () => {
        const dst = Image.open(new Uint8Array(ref2k));
        dst.pasteColor(255, 0, 0, 255, 100, 100, 900, 700); dst.toBytes(); dst.free();
    }); } catch (e) { results.pasteColor = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.gaussianBlur = runBench('gaussianBlur', () => { const r = img.gaussianBlur(3.0); r.toBytes(); r.free(); }); }
    catch (e) { results.gaussianBlur = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.save = runBench('save', () => { const s = img.save(); if (s.length === 0) {} }); }
    catch (e) { results.save = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.enhanceBrightness = runBench('enhanceBrightness', () => { const r = img.enhanceBrightness(1.5); r.toBytes(); r.free(); }); }
    catch (e) { results.enhanceBrightness = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.reduce = runBench('reduce', () => { const r = img.reduce(2); r.toBytes(); r.free(); }); }
    catch (e) { results.reduce = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.imageops_invert = runBench('ImageOps.invert', () => { const r = ImageOps.invert(img); r.toBytes(); r.free(); }); }
    catch (e) { results.imageops_invert = { mean_ms: -1, runs: 0, error: String(e) }; }

    try { results.imageops_grayscale = runBench('ImageOps.grayscale', () => { const r = ImageOps.grayscale(img); r.toBytes(); r.free(); }); }
    catch (e) { results.imageops_grayscale = { mean_ms: -1, runs: 0, error: String(e) }; }

    img.free(); img2k.free(); grayImg.free();
    saveResults('wasm_cpu', results);
}

main().catch(err => { console.error('[fatal]', err); process.exit(1); });
