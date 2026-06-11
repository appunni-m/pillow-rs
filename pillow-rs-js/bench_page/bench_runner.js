/**
 * Browser WASM Benchmark Runner
 *
 * Loads WASM (web target), fetches reference images, and runs image operation
 * benchmarks. Outputs JSON results to #results element and sets #status to "Done".
 */
import init, { Image, ImageOps } from '/pillow-rs-js/pkg/pillow_rs_js.js';

const WARMUP_RUNS = 3;
const BENCH_RUNS = 30;

function byId(id) { return document.getElementById(id); }
function setStatus(text) {
    const el = byId('status');
    el.textContent = text;
    if (text === 'Done') el.classList.add('done');
}
function showError(msg) {
    const el = byId('error');
    el.textContent = msg;
    el.style.display = 'block';
}

function meanStd(arr) {
    const n = arr.length;
    const mean = arr.reduce((s, v) => s + v, 0) / n;
    const variance = arr.reduce((s, v) => s + (v - mean) ** 2, 0) / n;
    return { mean_ms: parseFloat(mean.toFixed(3)), std_ms: parseFloat(Math.sqrt(variance).toFixed(3)), runs: n };
}

function runBench(label, fn, results) {
    const timings = [];
    // warmup
    for (let i = 0; i < WARMUP_RUNS; i++) {
        try { fn(); } catch (e) { /* ignore */ }
    }
    // measured runs
    for (let i = 0; i < BENCH_RUNS; i++) {
        try {
            const t0 = performance.now();
            fn();
            const t1 = performance.now();
            timings.push(t1 - t0);
        } catch (e) { /* skip failed */ }
    }
    if (timings.length === 0) {
        results[label] = { mean_ms: -1, std_ms: -1, runs: 0, error: 'all runs failed' };
        console.warn(`  [SKIP] ${label}`);
    } else {
        results[label] = meanStd(timings);
    }
    console.log(`  ${label}: ${results[label].mean_ms}ms`);
}

async function fetchImageBytes(url) {
    const resp = await fetch(url);
    if (!resp.ok) throw new Error(`Failed to fetch ${url}: ${resp.status}`);
    const buf = await resp.arrayBuffer();
    return new Uint8Array(buf);
}

async function main() {
    setStatus('Initializing WASM...');
    byId('results').textContent = 'Loading WASM module...';
    console.log('[bench] Browser WASM benchmark runner');
    console.log('[bench] Target WASM: ../pkg/pillow_rs_js.js');

    try {
        await init();
        console.log('[bench] WASM initialized');
    } catch (err) {
        const msg = `WASM init failed: ${err.message || err}`;
        setStatus('Failed');
        showError(msg);
        byId('results').textContent = msg;
        return;
    }

    setStatus('Loading reference images...');
    console.log('[bench] Loading reference images...');

    let ref1kBytes, ref2kBytes, grayBytes;
    try {
        ref1kBytes = await fetchImageBytes('scripts/bench_reference_images/ref_1k.png');
        ref2kBytes = await fetchImageBytes('scripts/bench_reference_images/ref_2k.jpg');
        grayBytes = await fetchImageBytes('scripts/bench_reference_images/ref_grayscale.png');
        console.log(`[bench] Images loaded: ref_1k=${ref1kBytes.length}B, ref_2k=${ref2kBytes.length}B, gray=${grayBytes.length}B`);
    } catch (err) {
        setStatus('Failed');
        showError(`Failed to load reference images: ${err.message || err}`);
        return;
    }

    setStatus('Running benchmarks...');
    console.log('\n[bench] Running benchmarks (30 runs, 3 warmup)...\n');

    const results = {};

    // ── open ──
    try {
        runBench('open', () => {
            const img = Image.open(new Uint8Array(ref1kBytes));
            img.free();
        }, results);
    } catch (e) {
        results.open = { mean_ms: -1, runs: 0, error: String(e) };
    }

    // Load reusable images
    let img, img2k, grayImg;
    try {
        img = Image.open(new Uint8Array(ref1kBytes));
        img2k = Image.open(new Uint8Array(ref2kBytes));
        grayImg = Image.open(new Uint8Array(grayBytes));
    } catch (e) {
        setStatus('Failed');
        showError('Failed to open reference images: ' + e.message);
        return;
    }

    // ── resize ──
    try {
        runBench('resize', () => {
            const r = img.resize(800, 600);
            r.free();
        }, results);
    } catch (e) { results.resize = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── crop ──
    try {
        runBench('crop', () => {
            const r = img.crop(10, 10, 100, 100);
            r.free();
        }, results);
    } catch (e) { results.crop = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── rotate ──
    try {
        runBench('rotate', () => {
            const r = img.rotate(45);
            r.free();
        }, results);
    } catch (e) { results.rotate = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── transpose ──
    try {
        runBench('transpose', () => {
            const r = img.transpose('FLIP_LEFT_RIGHT');
            r.free();
        }, results);
    } catch (e) { results.transpose = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── convert ──
    try {
        runBench('convert', () => {
            const r = img.convert('L');
            r.free();
        }, results);
    } catch (e) { results.convert = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── filter (BLUR) ──
    try {
        runBench('filter(BLUR)', () => {
            const r = img.filter('BLUR');
            r.free();
        }, results);
    } catch (e) { results.filter = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── new (constructor) ──
    try {
        runBench('new', () => {
            const r = new Image('RGB', 1920, 1080, 255, 0, 0, 255);
            r.free();
        }, results);
    } catch (e) { results.new = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getpixel ──
    try {
        runBench('getpixel', () => {
            img.getpixel(100, 100);
        }, results);
    } catch (e) { results.getpixel = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── putpixel ──
    try {
        runBench('putpixel', () => {
            const p = new Image('RGB', 100, 100, 0, 0, 0, 255);
            for (let y = 0; y < 100; y++) {
                for (let x = 0; x < 100; x++) {
                    p.putpixel(x, y, 128, 128, 128, 255);
                }
            }
            p.free();
        }, results);
    } catch (e) { results.putpixel = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── split ──
    try {
        runBench('split', () => {
            const bands = img.split();
            bands.forEach(b => b.free());
        }, results);
    } catch (e) { results.split = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getbands ──
    try {
        runBench('getbands', () => {
            img.getbands();
        }, results);
    } catch (e) { results.getbands = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getbbox ──
    try {
        runBench('getbbox', () => {
            img.getbbox();
        }, results);
    } catch (e) { results.getbbox = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── getextrema ──
    try {
        runBench('getextrema', () => {
            img.getextrema();
        }, results);
    } catch (e) { results.getextrema = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── histogram ──
    try {
        runBench('histogram', () => {
            img.histogram();
        }, results);
    } catch (e) { results.histogram = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── thumbnail ──
    try {
        runBench('thumbnail', () => {
            const t = Image.open(new Uint8Array(ref1kBytes));
            t.thumbnail(200, 200);
            t.free();
        }, results);
    } catch (e) { results.thumbnail = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── paste (image paste) ──
    try {
        runBench('paste(image)', () => {
            const dst = new Image('RGBA', 500, 500, 0, 0, 0, 0);
            const src = new Image('RGBA', 100, 100, 255, 0, 0, 255);
            dst.pasteImage(src, 50, 50);
            dst.free();
            src.free();
        }, results);
    } catch (e) { results.paste = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── pasteColor ──
    try {
        runBench('pasteColor', () => {
            const dst = new Image('RGBA', 500, 500, 0, 0, 0, 0);
            dst.pasteColor(255, 0, 0, 255, 100, 100, 200, 200);
            dst.free();
        }, results);
    } catch (e) { results.pasteColor = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── gaussianBlur ──
    try {
        runBench('gaussianBlur', () => {
            const r = img.gaussianBlur(3.0);
            r.free();
        }, results);
    } catch (e) { results.gaussianBlur = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── save (toBytes PNG) ──
    try {
        runBench('save', () => {
            const s = img.save();
            if (s.length === 0) {}
        }, results);
    } catch (e) { results.save = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── enhanceBrightness ──
    try {
        runBench('enhanceBrightness', () => {
            const r = img.enhanceBrightness(1.5);
            r.free();
        }, results);
    } catch (e) { results.enhanceBrightness = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── reduce ──
    try {
        runBench('reduce', () => {
            const r = img.reduce(2);
            r.free();
        }, results);
    } catch (e) { results.reduce = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── ImageOps.invert ──
    try {
        runBench('ImageOps.invert', () => {
            const r = ImageOps.invert(img);
            r.free();
        }, results);
    } catch (e) { results.imageops_invert = { mean_ms: -1, runs: 0, error: String(e) }; }

    // ── ImageOps.grayscale ──
    try {
        runBench('ImageOps.grayscale', () => {
            const r = ImageOps.grayscale(img);
            r.free();
        }, results);
    } catch (e) { results.imageops_grayscale = { mean_ms: -1, runs: 0, error: String(e) }; }

    // Cleanup
    img.free();
    img2k.free();
    grayImg.free();

    // Output
    const jsonStr = JSON.stringify(results, null, 2);
    byId('results').textContent = jsonStr;
    setStatus('Done');
    console.log('[bench] All benchmarks complete');
}

main().catch(err => {
    const msg = `Unhandled error: ${err.message || err}`;
    setStatus('Failed');
    showError(msg);
    byId('results').textContent = msg;
    console.error('[fatal]', err);
});
