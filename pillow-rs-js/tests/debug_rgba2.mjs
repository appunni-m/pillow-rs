import { readFileSync } from 'fs';
import { join } from 'path';
import { createHash } from 'crypto';

const pkg = join(import.meta.dirname, '..', 'pkg');
const wasmBinary = readFileSync(join(pkg, 'pillow_rs_js_bg.wasm'));
const m = await import(join(pkg, 'pillow_rs_js.js'));
m.initSync(new WebAssembly.Module(wasmBinary));

const { Image, ImageDraw, imageNew } = m;

// Test: create RGBA, draw point, verify pixels via getpixel
const w = 10, h = 10;
const img = imageNew('RGBA', w, h, 1, 2, 3, 255);
const draw = new ImageDraw(img);
draw.point([5, 5], 100, 200, 50, 255);
const result = draw.image;

console.log('Mode:', result.mode);
console.log('Size:', result.width, 'x', result.height);
const raw = result.toBytes();
console.log('toBytes len:', raw.length);
console.log('Expected len:', w * h * 4);

// Check pixels via getpixel (which converts to RGBA internally)
// Sample: first pixel and modified pixel
console.log('getpixel(0,0):', Array.from(result.getpixel(0,0)));
console.log('getpixel(5,5):', Array.from(result.getpixel(5,5)));

// Check pixel at (0,2) where R channel is 6 per input
console.log('getpixel(2,0):', Array.from(result.getpixel(2,0)));

// Also check pixel-by-pixel from toBytes for (0,0):
// First pixel should be at bytes 0-3
console.log('toBytes[0..4]:', Array.from(raw.slice(0,4)));
console.log('toBytes[4..8]:', Array.from(raw.slice(4,8)));
