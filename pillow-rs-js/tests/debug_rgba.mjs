import { readFileSync } from 'fs';
import { join } from 'path';

const pkg = join(import.meta.dirname, '..', 'pkg');
const wasmBinary = readFileSync(join(pkg, 'pillow_rs_js_bg.wasm'));
const m = await import(join(pkg, 'pillow_rs_js.js'));
m.initSync(new WebAssembly.Module(wasmBinary));

const { Image, ImageDraw, imageNew } = m;

// Test point draw on RGBA
const img = imageNew('RGBA', 10, 10, 1, 2, 3, 255);
console.log('Initial mode:', img.mode);
console.log('Initial toBytes len:', img.toBytes().length);
console.log('Initial getpixel(0,0):', Array.from(img.getpixel(0,0)));

const draw = new ImageDraw(img);
draw.point([5, 5], 100, 200, 50, 255);

const result = draw.image;
console.log('Result mode:', result.mode);
console.log('Result toBytes len:', result.toBytes().length);
console.log('Result getpixel(0,0):', Array.from(result.getpixel(0,0)));
console.log('Result getpixel(5,5):', Array.from(result.getpixel(5,5)));

// Test arc draw on RGBA
const img2 = imageNew('RGBA', 10, 10, 1, 2, 3, 255);
const draw2 = new ImageDraw(img2);
draw2.arc(2, 2, 8, 8, 0, 180, 0, 255, 0, 255);
const result2 = draw2.image;
console.log('Arc result mode:', result2.mode);
console.log('Arc toBytes len:', result2.toBytes().length);
console.log('Arc getpixel(0,0):', Array.from(result2.getpixel(0,0)));
console.log('Arc getpixel(5,2):', Array.from(result2.getpixel(5,2)));
