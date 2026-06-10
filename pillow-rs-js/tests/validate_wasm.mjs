// WASM validation script — run via Node.js
// Usage: node --experimental-wasm-modules validate_wasm.mjs
// Reads test operations from stdin (JSON), runs them via WASM, outputs results (JSON)

import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));

async function main() {
    const wasm = await import(join(__dirname, '..', 'pkg', 'pillow_rs_js.js'));
    await wasm.default();

    const results = {};

    // ── Identity test: new + toBytes ───────────────────────
    const img = new wasm.Image("RGB", 10, 10, 255, 128, 0, 255);
    results["new_rgb_tobytes"] = Array.from(img.toBytes());

    // ── resize ────────────────────────────────────────────
    const small = img.resize(5, 5, "BILINEAR");
    results["resize"] = Array.from(small.toBytes());

    // ── crop ──────────────────────────────────────────────
    const cropped = img.crop(2, 2, 8, 8);
    results["crop"] = Array.from(cropped.toBytes());

    // ── rotate 90 ─────────────────────────────────────────
    const r90 = img.rotate(90);
    results["rotate_90"] = Array.from(r90.toBytes());

    // ── transpose ─────────────────────────────────────────
    const flipped = img.transpose("FLIP_LEFT_RIGHT");
    results["transpose_flip_lr"] = Array.from(flipped.toBytes());

    // ── convert to L ──────────────────────────────────────
    const gray = img.convert("L");
    results["convert_L"] = Array.from(gray.toBytes());

    // ── convert to RGBA ───────────────────────────────────
    const rgba = img.convert("RGBA");
    results["convert_RGBA"] = Array.from(rgba.toBytes());

    // ── filter ────────────────────────────────────────────
    const blurred = img.filter("BLUR");
    results["filter_blur"] = Array.from(blurred.toBytes());

    // ── split + getbands ──────────────────────────────────
    const bands = img.split();
    results["split_count"] = bands.length;
    results["getbands"] = img.getbands();

    // ── getpixel ──────────────────────────────────────────
    results["getpixel"] = img.getpixel(5, 5);

    // ── enhance ───────────────────────────────────────────
    const bright = img.enhanceBrightness(1.5);
    results["enhance_brightness"] = Array.from(bright.toBytes());

    // ── copy ──────────────────────────────────────────────
    const copy = img.copy();
    results["copy_size"] = copy.size();

    // ── meta ──────────────────────────────────────────────
    results["size"] = img.size();
    results["mode"] = img.mode;
    results["width"] = img.width;
    results["height"] = img.height;

    console.log(JSON.stringify(results));
}

main().catch(e => { console.error(e.message); process.exit(1); });
