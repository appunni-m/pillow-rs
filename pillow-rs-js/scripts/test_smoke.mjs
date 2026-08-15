import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
    Image,
    ImageChops,
    ImageOps,
    ImageSequence,
    initSync,
    resolveArrayLayout,
} from '../pkg/core/pillow_rs_js.js';

// wasm-pack's web target exposes initSync, which keeps this smoke lane
// independent of a browser and still executes the generated JS/WASM API.
initSync({
    module: readFileSync(new URL('../pkg/core/pillow_rs_js_bg.wasm', import.meta.url)),
});

const image = new Image('RGB', 4, 3, 10, 20, 30, 255);
assert.deepEqual(Array.from(image.size()), [4, 3]);
assert.equal(image.width, 4);
assert.equal(image.height, 3);
assert.equal(image.mode, 'RGB');

image.putpixel(1, 1, 200, 100, 50, 255);
assert.deepEqual(Array.from(image.getpixel(1, 1)), [200, 100, 50, 255]);
assert.equal(image.getbands().length, 3);
assert.equal(image.getdata().length, 4 * 3 * 3);

const copied = image.copy();
const gray = image.convert('L');
const resized = image.resize(2, 2, 'BILINEAR');
const cropped = image.crop(0, 0, 2, 2);
const rotated = image.rotate(15);
const transposed = image.transpose('FLIP_LEFT_RIGHT');
const inverted = ImageOps.invert(image);
const difference = ImageChops.difference(image, copied);

assert.equal(gray.mode, 'L');
assert.deepEqual(Array.from(resized.size()), [2, 2]);
assert.deepEqual(Array.from(cropped.size()), [2, 2]);
assert.deepEqual(Array.from(rotated.size()), [4, 3]);
assert.deepEqual(Array.from(transposed.size()), [4, 3]);
assert.equal(inverted.mode, 'RGB');
assert.equal(difference.mode, 'RGB');

const layout = resolveArrayLayout(new Uint32Array([2, 2, 3]), '|u1', null);
assert.equal(layout.mode, 'RGB');
assert.equal(layout.rawMode, 'RGB');
assert.equal(layout.width, 2);
assert.equal(layout.height, 2);
assert.equal(layout.dimensions, 3);
assert.equal(layout.modeReinterpretsDtype, false);

const sequence = new ImageSequence(image);
assert.equal(sequence.next(), undefined);

for (const value of [
    copied,
    gray,
    resized,
    cropped,
    rotated,
    transposed,
    inverted,
    difference,
    image,
]) {
    value.free();
}
layout.free();
sequence.free();

process.stdout.write('WASM API smoke: 1 scenario passed\n');
