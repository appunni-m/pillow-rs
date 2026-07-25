import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const manifest = JSON.parse(readFileSync(join(
    repoRoot,
    'pillow-rs',
    'tests',
    'fixtures',
    'image_backend',
    'backend_parity.json',
), 'utf8'));
if (
    manifest.oracle?.implementation !== 'Pillow'
    || manifest.oracle?.version !== '12.2.0'
) {
    throw new Error('drawing requires the Pillow 12.2.0 oracle');
}

const packageDir = join(here, '..', 'pkg', 'extra');
const wasm = readFileSync(join(packageDir, 'pillow_rs_js_bg.wasm'));
const bindings = await import(pathToFileURL(join(packageDir, 'pillow_rs_js.js')));
bindings.initSync({ module: new WebAssembly.Module(wasm) });

function imageFromSpec(spec) {
    const seed = new bindings.Image('L', 1, 1, 0, 0, 0, 255);
    const image = seed.fromBytes(
        spec.mode === 'PA' ? 'LA' : spec.mode,
        spec.size[0],
        spec.size[1],
        Buffer.from(spec.pixels_hex, 'hex'),
    );
    if (spec.palette_hex !== null) {
        image.putpalette(Buffer.from(spec.palette_hex, 'hex'), 'RGB');
    }
    return image;
}

function color(value, mode) {
    if (typeof value === 'number') {
        if (mode === 'RGB') return [value, 0, 0, 255];
        if (mode === 'RGBA' || mode === 'CMYK') return [value, 0, 0, 0];
        if (mode === 'LA') return [value, value, value, 0];
        return [value, value, value, 255];
    }
    if (value.length === 2) return [value[0], value[0], value[0], value[1]];
    if (value.length === 3) return [...value, 255];
    return value;
}

function optionalColor(parameters, name, mode) {
    return parameters[name] === undefined
        ? [undefined, undefined, undefined, undefined]
        : color(parameters[name], mode);
}

function drawCase(testCase) {
    const image = imageFromSpec(testCase.source);
    const draw = new bindings.ImageDraw(image);
    const p = testCase.parameters;
    const fill = optionalColor(p, 'fill', testCase.source.mode);
    const outline = optionalColor(p, 'outline', testCase.source.mode);
    const width = p.width ?? 1;
    const xy = p.xy.flat();

    switch (testCase.operation) {
    case 'line':
        draw.line(...xy, ...fill, width);
        break;
    case 'rectangle':
        draw.rectangle(...xy, ...fill, ...outline, width);
        break;
    case 'ellipse':
        draw.ellipse(...xy, ...fill, ...outline, width);
        break;
    case 'polygon':
        draw.polygon(xy, ...fill, ...outline, width);
        break;
    case 'point':
        draw.point(xy, ...fill);
        break;
    case 'arc':
        draw.arc(...xy, p.start, p.end, ...fill, width);
        break;
    case 'chord':
        draw.chord(...xy, p.start, p.end, ...fill, ...outline, width);
        break;
    case 'pieslice':
        draw.pieslice(...xy, p.start, p.end, ...fill, ...outline, width);
        break;
    case 'circle':
        draw.circle(...xy, p.radius, ...fill, ...outline, width);
        break;
    case 'rounded_rectangle':
        draw.roundedRectangle(...xy, p.radius, ...fill, ...outline, width);
        break;
    default:
        throw new Error(`${testCase.id}: unsupported drawing operation`);
    }
    return draw.image;
}

for (const testCase of manifest.draw_cases) {
    if (
        JSON.stringify(testCase.backends) !== JSON.stringify(['cpu'])
        || JSON.stringify(testCase.unsupported_backends) !== JSON.stringify(['simd', 'gpu'])
    ) {
        throw new Error(`${testCase.id}: drawing backend declaration changed`);
    }
    const actual = drawCase(testCase);
    const expected = testCase.expected;
    if (actual.mode !== expected.mode) {
        throw new Error(`${testCase.id}: mode ${actual.mode} != ${expected.mode}`);
    }
    if (actual.width !== expected.size[0] || actual.height !== expected.size[1]) {
        throw new Error(`${testCase.id}: exact dimensions differ`);
    }
    if (Buffer.from(actual.toBytes()).toString('hex') !== expected.pixels_hex) {
        throw new Error(`${testCase.id}: exact pixels differ`);
    }
    if (
        expected.palette_hex !== null
        && Buffer.from(actual.getpalette()).toString('hex') !== expected.palette_hex
    ) {
        throw new Error(`${testCase.id}: exact palette differs`);
    }
}

console.log(
    `core: ${manifest.draw_cases.length} exact CPU drawing Pillow oracle cases passed; `
    + 'SIMD/GPU remain explicitly unsupported',
);
